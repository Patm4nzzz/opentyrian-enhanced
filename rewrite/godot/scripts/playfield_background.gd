## Play area background in 8-bit indices: layer 1 and, if
## `background2over` is 0 or 3, layer 2 blended by index like
## backgrnd.c:blit_background_row_blend (TyrianSim.playfield_background).
## Then replaces the corresponding TileMapLayers of LevelMap. Used
## only with the 8-bit HUD (HudRenderer palette) and without HD tiles.
class_name PlayfieldBackground
extends Sprite2D

var _texture: ImageTexture
## Hardware palette of the level (TyrianVga): that of the 8-bit HUD (pic 3) in
## solo, a TyrianVga loaded with pic 6 in two-player (same palette 5,
## pcxmast.c:pcxpal).
var _vga: RefCounted


func _init() -> void:
	centered = false
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST


## True if the indexed render is available: hides the replaced layers.
func start(sim: Node, map: LevelMap, vga: RefCounted) -> bool:
	if sim == null or vga == null or not sim.has_method("playfield_background"):
		return false
	_vga = vga
	if not refresh(sim):
		return false
	_hide_layers(sim, map)
	return true


func _hide_layers(sim: Node, map: LevelMap) -> void:
	var l1 := map.layer(0)
	if l1 != null:
		l1.visible = false
	var parts: int = sim.playfield_parts()
	var l2 := map.layer(1)
	if l2 != null:
		l2.visible = parts & 1 == 0
	var l3 := map.layer(2)
	if l3 != null:
		l3.visible = parts & 2 == 0
	# Enemies (TyrianSim.enemies layers): ground always composited, sky and
	# top according to skyEnemyOverAll / topEnemyOver; player shots composited.
	var shown := {"Enemies0": false, "Enemies1": false, "Enemies2": parts & 4 == 0, "Enemies3": parts & 8 == 0}
	for n in shown:
		var root := get_parent().get_node_or_null(n)
		if root != null:
			root.visible = shown[n]
	var level := get_parent()
	for n in level.get("_player_shots"):
		n.visible = parts & 16 == 0
	for n in level.get("_enemy_shots"):
		n.visible = parts & 64 == 0
	var ex: Node = level.get("_explosion_node")
	if ex != null:
		ex.visible = parts & 64 == 0
	# Mod ship (PNG): Godot drawing kept (the composite uses the original sheet).
	var pl: Node = level.get("_player_node")
	if pl != null:
		pl.visible = parts & 32 == 0 or level.get("_mod_ship_texture") != null


## Hands control back to the Godot nodes (switch to HD (AI) mode during a level):
## everything _hide_layers hid becomes visible again, then the node disappears.
func stop(map: LevelMap) -> void:
	for i in 3:
		var l := map.layer(i)
		if l != null:
			l.visible = true
	var level := get_parent()
	for n in ["Enemies0", "Enemies1", "Enemies2", "Enemies3"]:
		var root := level.get_node_or_null(n)
		if root != null:
			root.visible = true
	for n in level.get("_player_shots"):
		n.visible = true
	for n in level.get("_enemy_shots"):
		n.visible = true
	for prop in ["_explosion_node", "_player_node"]:
		var node: Node = level.get(prop)
		if node != null:
			node.visible = true
	if _glow != null:
		_glow.queue_free()
		_glow = null
	queue_free()


func refresh(sim: Node) -> bool:
	var img: Image = sim.playfield_background(Game.data_dir, _vga.palette_bytes())
	if img == null:
		return false
	if _texture == null:
		_texture = ImageTexture.create_from_image(img)
		texture = _texture
	else:
		_texture.update(img)
	return true


func update(sim: Node, map: LevelMap) -> void:
	if refresh(sim):
		_hide_layers(sim, map)
	_refresh_glow(sim)


## RGBA layer of the C code (video_rgba.c, TyrianSim.playfield_glow) composited above
## the whole image with classic alpha blending (SDL_BLENDMODE_BLEND).
var _glow: Sprite2D
var _glow_texture: ImageTexture


func _refresh_glow(sim: Node) -> void:
	if not sim.has_method("playfield_glow"):
		return
	var img: Image = sim.playfield_glow(_vga.palette_bytes())
	if img == null:
		return
	if _glow == null:
		_glow = Sprite2D.new()
		_glow.name = "RgbaOverlay"
		_glow.centered = false
		_glow.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		# 320×200 screen frame (RGBAScreen composited 1:1; the game's x
		# are shifted there by GAME_COORD_X_OFFSET = -24 on the Rust side).
		_glow.z_index = RenderingServer.CANVAS_ITEM_Z_MAX - 1
		get_parent().add_child(_glow)
	if _glow_texture == null:
		_glow_texture = ImageTexture.create_from_image(img)
		_glow.texture = _glow_texture
	else:
		_glow_texture.update(img)
