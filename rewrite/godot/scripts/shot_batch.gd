## Batch of projectiles drawn by a MultiMeshInstance2D (one Sprite2
## sheet = one texture). Each instance carries in INSTANCE_CUSTOM the normalized
## UV rectangle of its image (see shaders/multimesh_atlas.gdshader).
##
## Main path: `update_from_sim` — the full MultiMesh buffer is
## built by the Rust extension (TyrianSim.shot_instances) and set in a single
## call; the capacity doubles on demand, with no ceiling (docs/performance.md).
## `update_from` (GDScript loop, one instance at a time) remains for
## calls without a simulation (tools).
class_name ShotBatch
extends MultiMeshInstance2D

## Initial capacity; it doubles on demand.
const INITIAL_INSTANCES := 512
## Floats per instance in MultiMesh.buffer: Transform2D (8) + custom (4).
const INSTANCE_FLOATS := 12

var sheet: SpriteSheet
## Offset subtracted from the image index (500 for spriteSheet12, shots.c).
var index_base := 0
var _tex_size := Vector2.ONE
## Image table for the extension: 8 floats per index (normalized u, v, w, h,
## width and height in pixels, x, y offset); width 0 = absent.
var _frames := PackedFloat32Array()


static func _quad_mesh() -> ArrayMesh:
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = PackedVector2Array([Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)])
	arrays[Mesh.ARRAY_TEX_UV] = PackedVector2Array([Vector2(0, 0), Vector2(1, 0), Vector2(1, 1), Vector2(0, 1)])
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0, 1, 2, 0, 2, 3])
	var m := ArrayMesh.new()
	m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return m


func setup(sheet_name: String, base: int) -> void:
	sheet = TyrianAssets.sheet(sheet_name)
	index_base = base
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_2D
	mm.use_custom_data = true
	mm.mesh = _quad_mesh()
	mm.instance_count = INITIAL_INSTANCES
	mm.visible_instance_count = 0
	multimesh = mm
	if sheet != null:
		texture = sheet.texture
		# Normalized UVs: size of the original atlas (identical in HD mode,
		# where the texture is 4× larger; regions stays in original pixels).
		_tex_size = Vector2(sheet.texture.get_size()) / sheet.scale
		_build_frame_table()
	var mat := ShaderMaterial.new()
	mat.shader = preload("res://shaders/multimesh_atlas.gdshader")
	material = mat
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST


func _build_frame_table() -> void:
	var top := 0
	for idx in sheet.regions:
		top = maxi(top, int(idx))
	_frames.resize((top + 1) * 8)
	_frames.fill(0.0)
	for idx in sheet.regions:
		var r: Rect2 = sheet.regions[idx]
		var o: Vector2 = sheet.offsets[idx]
		var k := int(idx) * 8
		_frames[k] = r.position.x / _tex_size.x
		_frames[k + 1] = r.position.y / _tex_size.y
		_frames[k + 2] = r.size.x / _tex_size.x
		_frames[k + 3] = r.size.y / _tex_size.y
		_frames[k + 4] = r.size.x
		_frames[k + 5] = r.size.y
		_frames[k + 6] = o.x
		_frames[k + 7] = o.y


## Fills the batch from the simulation: enemy shots (`enemy`) or the player's
## whose image is in [sprite_min, sprite_max], x shifted by `x_offset`.
func update_from_sim(sim: Node, enemy: bool, sprite_min: int, sprite_max: int, x_offset: int) -> void:
	if sheet == null or sim == null:
		return
	var data: PackedFloat32Array = sim.shot_instances(enemy, sprite_min, sprite_max, index_base, x_offset, _frames)
	var n := data.size() / INSTANCE_FLOATS
	if n == 0:
		multimesh.visible_instance_count = 0
		return
	var cap := multimesh.instance_count
	if n > cap:
		while cap < n:
			cap *= 2
		multimesh.instance_count = cap
	# MultiMesh.buffer must cover the whole capacity; only the first n
	# instances are drawn (visible_instance_count).
	data.resize(cap * INSTANCE_FLOATS)
	multimesh.buffer = data
	multimesh.visible_instance_count = n


## `items`: flattened array (x, y, sprite); `accept` filters the sprites of this sheet.
func update_from(items: PackedInt32Array, stride: int, accept: Callable) -> void:
	if sheet == null:
		return
	var n := 0
	var i := 0
	while i + 2 < items.size():
		var sprite := items[i + 2]
		if accept.call(sprite):
			var idx := sprite - index_base
			if sheet.regions.has(idx):
				if n >= multimesh.instance_count:
					multimesh.instance_count = multimesh.instance_count * 2
				var r: Rect2 = sheet.regions[idx]
				var pos: Vector2 = Vector2(items[i], items[i + 1]) + sheet.offsets[idx]
				multimesh.set_instance_transform_2d(n, Transform2D(Vector2(r.size.x, 0), Vector2(0, r.size.y), pos))
				multimesh.set_instance_custom_data(
					n,
					Color(
						r.position.x / _tex_size.x,
						r.position.y / _tex_size.y,
						r.size.x / _tex_size.x,
						r.size.y / _tex_size.y
					)
				)
				n += 1
		i += stride
	multimesh.visible_instance_count = n
