## HD (AI) mode: what the C does on the play area's palette indices
## and that the Godot nodes, which draw in colors, do not have.
##   - player shots (shadows included) and enemy shots, from the list of
##     what the C drew (TyrianSim.hd_shot_sprites);
##   - enemies: the list of what JE_drawEnemy drew at this iteration
##     (TyrianSim.hd_enemy_sprites, darkened trails and tinted enemies
##     included), not the state after the tick;
##   - player ships (player 2's Dragonwing included), darkened shadows,
##     charge icon and sidekicks, same pieces as the 8-bit rendering
##     (TyrianSim.hd_player_sprites), upscaled sheets;
##   - Zinglon beam (TyrianSim.hd_zinglon, vga256d.c:JE_barBright) after the
##     ship;
##   - explosions blended with what they cover (blit_sprite2_blend);
##   - level filter (event 44, backgrnd.c:JE_filterScreen), lamp and
##     flipped image (enemy_render.c:JE_starShowVGA) over the whole play area
##     (TyrianSim.hd_screen_effects).
## C operations: shaders/hd_index_effects.gdshader. Each node that reads
## the screen is preceded by a copy of the screen (BackBufferCopy): it sees what
## is drawn under it at that moment, like the C.
class_name HdPlayfieldEffects
extends RefCounted

const SHADER := preload("res://shaders/hd_index_effects.gdshader")
const PLAY_SIZE := Vector2(264, 184)
const NORMAL := 0
const BLEND := 1
const DARKEN := 2
const SCREEN := 3
const BRIGHT := 5
const SMOOTHIE := 6
## Sheets of TyrianSim.hd_player_sprites ("main:N" number).
const SHEETS := {
	5: "main_05_options",
	7: "main_07_player_shots",
	8: "main_08_player_ships",
	9: "main_09_powerups",
	11: "main_11_player_shots2",
	12: "main_12_ships_t2000"
}

## Color → index table (whole palette) per palette.
static var _lut_cache := {}

var _level: Node2D
var _sim: Node
var _x_offset: int
var _ship_nodes := {}
var _blits := PackedInt32Array()
var _zinglon := PackedInt32Array()
var _zinglon_node: Node2D
var _enemy_blits := PackedInt32Array()
## Enemy layer (0..3) -> HD drawing node.
var _enemy_nodes := {}
var _enemy_copies := {}
var _shot_blits := PackedInt32Array()
## 0 player shots, 1 enemy shots -> HD drawing node.
var _shot_nodes := {}
## Variable drawing order of tyrian2.c:JE_main (level events):
## layers 2 and 3 and the sky and top enemies change rank.
## Ranks (z) set in `_apply_order` from TyrianSim.hd_screen_effects.
var _map: Node2D
var _z := {}
var _screen_rect: Sprite2D
## Image filters: [BackBufferCopy, rectangle] per tyrian2.c step.
var _smoothie_stages: Array = []
var _materials: Array[ShaderMaterial] = []
var _palette := PackedByteArray()
var _explosion_material: ShaderMaterial


## `x_offset`: screen offset of the simulation's x (game_screen → screen).
## `z`: ranks of the level.gd elements (enum Z: LAYER1, LAYER2, ENEMY_GROUND,
## ENEMY_SKY, LAYER3, ENEMY_TOP, PLAYER_SHOTS, PLAYER, ENEMY_SHOTS, EXPLOSIONS).
func _init(level: Node2D, sim: Node, map: Node2D, explosion_node: Node2D, z: Dictionary, x_offset: int) -> void:
	_level = level
	_sim = sim
	_map = map
	_z = z
	_x_offset = x_offset
	var enemy_z := [z.ENEMY_GROUND, z.ENEMY_GROUND, z.ENEMY_SKY, z.ENEMY_TOP]
	var shot_z := [z.PLAYER_SHOTS, z.ENEMY_SHOTS]
	var z_player: int = z.PLAYER
	# Filter and lamp: after layer 2 "over 2" (EXPLOSIONS + 1).
	var z_effects: int = z.EXPLOSIONS + 2
	for layer in 4:
		_enemy_copies[layer] = _add_back_buffer_copy(enemy_z[layer])
		var e := Node2D.new()
		e.name = "HdEnemies%d" % layer
		e.z_index = enemy_z[layer]
		e.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		e.material = _new_material(-1)
		e.draw.connect(_draw_enemies.bind(e, layer))
		level.add_child(e)
		_enemy_nodes[layer] = e
	for kind in 2:
		var sz: int = shot_z[kind]
		_add_back_buffer_copy(sz)
		var n := Node2D.new()
		n.name = "HdShots%d" % kind
		n.z_index = sz
		n.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		n.material = _new_material(-1)
		n.draw.connect(_draw_shots.bind(n, kind))
		level.add_child(n)
		_shot_nodes[kind] = n
	for mode in [DARKEN, NORMAL, BLEND]:
		# Ship shadows go under the player shots (rank just
		# before them): the C drew them afterwards and darkened the shots.
		var z_node: int = z.PLAYER_SHOTS - 1 if mode == DARKEN else z_player
		var n := Node2D.new()
		n.name = "HdShips%d" % mode
		n.z_index = z_node
		n.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		n.draw.connect(_draw_ships.bind(n, mode))
		if mode != NORMAL:
			_add_back_buffer_copy(z_node)
			n.material = _new_material(mode)
		level.add_child(n)
		_ship_nodes[mode] = n
	# Zinglon beam: JE_doSpecialShot comes after the ship (player_movement.c).
	_add_back_buffer_copy(z_player)
	_zinglon_node = Node2D.new()
	_zinglon_node.name = "HdZinglon"
	_zinglon_node.z_index = z_player
	_zinglon_node.material = _new_material(-1)
	_zinglon_node.draw.connect(_draw_zinglon)
	level.add_child(_zinglon_node)
	# Explosions: copy of the screen just before, then blend by index.
	var copy := _add_back_buffer_copy(explosion_node.z_index)
	level.move_child(copy, explosion_node.get_index())
	# HD layer 2 blended by index (hd_layer_blend): it reads the screen under
	# it, which must be re-copied if it comes after other readers.
	if map.layer(1) != null:
		var l2_copy := BackBufferCopy.new()
		l2_copy.copy_mode = BackBufferCopy.COPY_MODE_VIEWPORT
		map.layer(1).add_child(l2_copy)
		map.layer(1).move_child(l2_copy, 0)
	_explosion_material = _new_material(BLEND)
	explosion_node.material = _explosion_material
	_add_back_buffer_copy(z_effects)
	# Image filters (tyrian2.c), one step per C slot (see
	# _update_smoothies).
	for k in 5:
		var stage_copy := _add_back_buffer_copy(0)
		var r := Sprite2D.new()
		r.name = "HdSmoothie%d" % k
		r.centered = false
		r.scale = PLAY_SIZE
		r.material = _new_material(SMOOTHIE)
		r.visible = false
		_level.add_child(r)
		_smoothie_stages.append([stage_copy, r])
	_screen_rect = Sprite2D.new()
	_screen_rect.name = "HdScreenEffects"
	_screen_rect.centered = false
	var white := Image.create_empty(1, 1, false, Image.FORMAT_RGBA8)
	white.fill(Color.WHITE)
	_screen_rect.texture = ImageTexture.create_from_image(white)
	for st: Array in _smoothie_stages:
		st[1].texture = _screen_rect.texture
	_screen_rect.scale = PLAY_SIZE
	_screen_rect.z_index = z_effects
	_screen_rect.material = _new_material(SCREEN)
	_screen_rect.visible = false
	level.add_child(_screen_rect)


func _add_back_buffer_copy(z: int) -> BackBufferCopy:
	var b := BackBufferCopy.new()
	b.copy_mode = BackBufferCopy.COPY_MODE_VIEWPORT
	b.z_index = z
	_level.add_child(b)
	return b


func _new_material(mode: int) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = SHADER
	m.set_shader_parameter("mode", mode)
	m.set_shader_parameter("play_size", PLAY_SIZE)
	_materials.append(m)
	return m


## One frame: current hardware palette (768 bytes), simulation state.
func update(palette: PackedByteArray) -> void:
	if palette != _palette:
		_palette = palette
		var lut := _lut(palette)
		var pal_tex := ImageTexture.create_from_image(Image.create_from_data(256, 1, false, Image.FORMAT_RGB8, palette))
		for m in _materials:
			m.set_shader_parameter("index_lut", lut)
			m.set_shader_parameter("palette_tex", pal_tex)
	_blits = _sim.hd_player_sprites()
	_zinglon = _sim.hd_zinglon()
	_zinglon_node.queue_redraw()
	_enemy_blits = _sim.hd_enemy_sprites()
	_shot_blits = _sim.hd_shot_sprites()
	for n in _shot_nodes.values():
		n.queue_redraw()
	for n in _ship_nodes.values():
		n.queue_redraw()
	for n in _enemy_nodes.values():
		n.queue_redraw()
	var fx: PackedInt32Array = _sim.hd_screen_effects()
	if fx.size() > 8:
		_apply_order(fx[5], fx[6], fx[7] != 0, fx[8] != 0)
	if fx.size() > 15:
		_update_smoothies(fx)
	var on := fx[0] != -99 or fx[1] != -99 or fx[2] != 0
	_screen_rect.visible = on
	if on:
		var m := _screen_rect.material as ShaderMaterial
		m.set_shader_parameter("filter_col", fx[0])
		m.set_shader_parameter("filter_int", fx[1])
		m.set_shader_parameter("star_show_code", fx[2])
		m.set_shader_parameter("light_x", fx[3])
		m.set_shader_parameter("light_y", fx[4])


## Shows or hides all the nodes (Original / HD toggle during a level).
func set_active(on: bool, explosion_node: Node2D) -> void:
	for n in _ship_nodes.values():
		n.visible = on
	for n in _enemy_nodes.values():
		n.visible = on
	for n in _shot_nodes.values():
		n.visible = on
	_zinglon_node.visible = on
	if not on:
		_screen_rect.visible = false
	explosion_node.material = _explosion_material if on else null


static func _lut(palette: PackedByteArray) -> Texture2D:
	if not _lut_cache.has(palette):
		if _lut_cache.size() >= 8:
			_lut_cache.clear()
		var img: Image = ClassDB.class_call_static("TyrianVga", "nearest_index_lut", palette, 0xffff)
		_lut_cache[palette] = ImageTexture.create_from_image(img)
	return _lut_cache[palette]


func _draw_ships(node: Node2D, mode: int) -> void:
	for k in range(0, _blits.size() - 4, 5):
		if _blits[k + 4] != mode:
			continue
		var sheet := TyrianAssets.sheet(SHEETS.get(_blits[k], ""))
		if sheet != null:
			sheet.draw(node, _blits[k + 3], _blits[k + 1] + _x_offset, _blits[k + 2])


## Two JE_barBright per beam (x ± t then x ± (t + 2), lines 0 to 184): the
## core is brightened twice, the edges once. Mode (red) and number of
## passes (green) in the drawing color (shader in mode -1).
func _draw_zinglon() -> void:
	for k in range(0, _zinglon.size() - 1, 2):
		var cx: int = _zinglon[k] + _x_offset
		var t: int = _zinglon[k + 1]
		var once := Color8(BRIGHT, 1, 0)
		_zinglon_node.draw_rect(Rect2(cx - t - 2, 0, 2, 185), once)
		_zinglon_node.draw_rect(Rect2(cx + t + 1, 0, 2, 185), once)
		_zinglon_node.draw_rect(Rect2(cx - t, 0, 2 * t + 1, 185), Color8(BRIGHT, 2, 0))


func _draw_enemies(node: Node2D, layer: int) -> void:
	for k in range(0, _enemy_blits.size() - 6, 7):
		if _enemy_blits[k] != layer:
			continue
		var sheet := TyrianAssets.sheet(TyrianAssets.enemy_sheet_name(_enemy_blits[k + 1]))
		if sheet == null:
			continue
		# Mode and filter passed to the shader through the drawing color (mode -1).
		var tag := Color8(_enemy_blits[k + 5], _enemy_blits[k + 6], 0)
		sheet.draw(node, _enemy_blits[k + 4], _enemy_blits[k + 2] + _x_offset, _enemy_blits[k + 3], tag)


func _draw_shots(node: Node2D, kind: int) -> void:
	for k in range(0, _shot_blits.size() - 5, 6):
		if _shot_blits[k] != kind:
			continue
		var sheet := TyrianAssets.sheet(SHEETS.get(_shot_blits[k + 1], ""))
		if sheet != null:
			var tag := Color8(_shot_blits[k + 5], 0, 0)
			sheet.draw(node, _shot_blits[k + 4], _shot_blits[k + 2] + _x_offset, _shot_blits[k + 3], tag)


## Image filters of tyrian2.c (fx[9..13]: smoothies 1 to 5, fx[14]:
## smoothie_data[0], fx[15]: water tint), at the C's drawing ranks:
## ice (smoothies 5) after layer 1, lava and water before the ground
## enemies (lava after them if smoothie_data[0] > 0), ice (smoothies 3) before the
## sky enemies. The blur (smoothies 4) keeps the image's tint and only
## blends it with the previous one: with no memory in HD, nothing to redo.
func _update_smoothies(fx: PackedInt32Array) -> void:
	var ground: int = _enemy_nodes[0].z_index
	var sky: int = _enemy_nodes[2].z_index
	# [active, filter, rank]
	var stages := [
		[fx[13] != 0, 3, _z.LAYER1 + 1],
		[fx[9] != 0 and fx[14] == 0, 1, ground - 2],
		[fx[10] != 0, 2, ground - 1],
		[fx[9] != 0 and fx[14] > 0, 1, ground + 1],
		[fx[11] != 0, 3, sky - 1],
	]
	for k in 5:
		var on: bool = stages[k][0]
		var copy: BackBufferCopy = _smoothie_stages[k][0]
		var r: Sprite2D = _smoothie_stages[k][1]
		r.visible = on
		copy.visible = on
		if on:
			_set_z(copy, stages[k][2])
			_set_z(r, stages[k][2])
			var m := r.material as ShaderMaterial
			m.set_shader_parameter("smoothie", stages[k][1])
			m.set_shader_parameter("water_hue", fx[15])


## Drawing ranks of tyrian2.c:JE_main according to background2over (0/3: layer 2
## before the ground enemies, 1: after, 2: after the explosions),
## background3over (0: after the sky enemies, 1: after the enemy shots,
## 2: before the sky enemies), topEnemyOver and skyEnemyOverAll (top then sky
## enemies after the enemy shots and layer 3 "over 1"). The
## ranks of level.gd (enum Z) are spaced by 4: a moved element takes an
## intermediate rank.
func _apply_order(over2: int, over3: int, top_over: bool, sky_over: bool) -> void:
	var l2: int = _z.LAYER2
	var ground: int = _z.ENEMY_GROUND
	if over2 == 1:
		ground = _z.LAYER2 - 2
	elif over2 == 2:
		l2 = _z.EXPLOSIONS + 1
	var l3: int = _z.LAYER3
	if over3 == 1:
		l3 = _z.ENEMY_SHOTS + 1
	elif over3 == 2:
		l3 = _z.ENEMY_SKY - 2
	var top: int = _z.ENEMY_SHOTS + 2 if top_over else _z.ENEMY_TOP
	var sky: int = _z.ENEMY_SHOTS + 3 if sky_over else _z.ENEMY_SKY
	_set_z(_map.layer(1), l2)
	_set_z(_map.layer(2), l3)
	for layer in [0, 1]:
		_set_z(_enemy_nodes[layer], ground)
		_set_z(_enemy_copies[layer], ground)
	_set_z(_enemy_nodes[2], sky)
	_set_z(_enemy_copies[2], sky)
	_set_z(_enemy_nodes[3], top)
	_set_z(_enemy_copies[3], top)


static func _set_z(node: CanvasItem, z: int) -> void:
	if node != null and node.z_index != z:
		node.z_index = z
