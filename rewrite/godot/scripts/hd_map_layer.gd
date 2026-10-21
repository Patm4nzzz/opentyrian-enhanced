## Scenery layer of the HD (AI) mode, ADR 0100: the layer's whole map,
## upscaled ×4 offline in a single block (scripts/hd_upscale.py render), then
## cut into strips of `chunk_rows` tile rows (WebP, with `margin`
## pixels of the neighboring strips above and below, not displayed: they
## avoid a compression seam between two strips). No tiles:
## no grid. Child of the TileMapLayer it replaces, scaled down to 1/4
## to keep the original geometry. Every frame, it draws the pieces
## of the upscaled map where the C places the tiles
## (TyrianSim.hd_layer_cells, set by set_cells): the C addresses map and screen
## flat, a row of tiles may overflow onto the next map line
## and a tile cut at the right edge continues on the left, one line lower.
## Only the strips near the screen are in memory; the following ones are
## decoded ahead of time in a WorkerThreadPool thread.
class_name HdMapLayer
extends Node2D

const TILE_W := 24
const TILE_H := 28
## Height of the play area in original pixels.
const VIEW_HEIGHT := 200
## Strips decoded ahead on each side (one strip = 112 original pixels);
## 2 measured slower (more texture uploads per frame).
const PREFETCH := 1

var _dir := ""
var _chunk_h := 0  # height of a strip in original pixels
var _margin := 0  # ×4 pixels of the neighbors encoded around each strip
var _available := {}  # numbers of the strips present in layer.json
var _shown := {}  # number -> displayable ImageTexture
var _tasks := {}  # number -> WorkerThreadPool task id
var _decoded := {}  # number -> ImageTexture (filled by the tasks)
var _mutex := Mutex.new()
var _chunk_rows := 1
var _k := 4.0  # image upscale factor
## Pieces to draw (TyrianSim.hd_layer_cells): column, row, x, y
## (game_screen), first pixel of the tile, width.
var _cells := PackedInt32Array()


## Folder name of a layer: first 16 hexadecimal digits of the SHA-256
## of "tileset width height\n" then of the tile numbers as little-endian
## int32, empty tiles (negative or blank_tiles) as 0. Same computation
## as key() in scripts/hd_upscale.py.
static func layer_key(tileset_name: String, layer_json: Dictionary, blank: Dictionary) -> String:
	var tiles := PackedInt32Array()
	for v in layer_json["tiles"]:
		var n := int(v)
		tiles.append(0 if n < 0 or blank.has(n) else n)
	var ctx := HashingContext.new()
	ctx.start(HashingContext.HASH_SHA256)
	ctx.update(("%s %d %d\n" % [tileset_name, int(layer_json["width"]), int(layer_json["height"])]).to_utf8_buffer())
	ctx.update(tiles.to_byte_array())
	return ctx.finish().hex_encode().substr(0, 16)


## HD layer of `layer_json`, or null if its images are missing (the layer
## then keeps its original tiles).
static func open(maps_dir: String, tileset_name: String, layer_json: Dictionary, blank: Dictionary) -> HdMapLayer:
	var dir := maps_dir.path_join(layer_key(tileset_name, layer_json, blank))
	var text := FileAccess.get_file_as_string(dir.path_join("layer.json"))
	if text.is_empty():
		return null
	var meta: Variant = JSON.parse_string(text)
	if not meta is Dictionary:
		return null
	var hd := HdMapLayer.new()
	hd.name = "HdMap"
	hd._dir = dir
	hd._chunk_rows = int(meta["chunk_rows"])
	hd._chunk_h = hd._chunk_rows * TILE_H
	hd._margin = int(meta.get("margin", 0))
	hd._k = float(meta["scale"])
	hd.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	for c in meta["chunks"]:
		hd._available[int(c)] = true
	hd.scale = Vector2.ONE / float(meta["scale"])
	return hd


func _process(_delta: float) -> void:
	update_view()


## Keeps the visible strips and their immediate neighbors in memory (missing
## neighbors are loaded right away), and decodes in the background
## the next PREFETCH strips on each side; frees the others. The top
## of the screen is at local y -position.y of the parent layer.
func update_view() -> void:
	# Outside the tree (level removed by Effects.show_scene, freed at the end of
	# the frame): no task, it would outlive the layer (crash).
	var parent := get_parent() as Node2D
	if parent == null or not is_inside_tree():
		return
	var top := -parent.position.y
	var first := floori(top / _chunk_h) - 1
	var last := floori((top + VIEW_HEIGHT - 1) / _chunk_h) + 1
	for c in range(first, last + 1):
		_show(c, true)
	for k in range(1, PREFETCH + 1):
		_show(first - k, false)
		_show(last + k, false)
	for c in _shown.keys():
		if c < first or c > last:
			_shown.erase(c)
	_mutex.lock()
	for c in _decoded.keys():
		if c < first - PREFETCH or c > last + PREFETCH:
			_decoded.erase(c)
	_mutex.unlock()


func _show(c: int, now: bool) -> void:
	if not _available.has(c) or _shown.has(c):
		return
	if _tasks.has(c) and now:
		WorkerThreadPool.wait_for_task_completion(_tasks[c])
		_tasks.erase(c)
	var tex := _take(c)
	if tex == null:
		if not now:
			if not _tasks.has(c):
				_tasks[c] = WorkerThreadPool.add_task(_decode.bind(c))
			return
		tex = _load(c)
	if tex == null:
		push_error("HdMapLayer: unreadable strip %s" % _path(c))
		_available.erase(c)
		return
	_shown[c] = tex


## Map pieces placed by the C at this frame (TyrianSim.hd_layer_cells).
func set_cells(cells: PackedInt32Array) -> void:
	_cells = cells
	queue_redraw()


## Local frame: that of the parent TileMapLayer (position -layer_scroll,
## origin at screen x = -24 of game_screen), ×_k.
func _draw() -> void:
	var parent := get_parent() as Node2D
	if parent == null:
		return
	var origin := -parent.position - Vector2(24, 0)
	for i in range(0, _cells.size() - 5, 6):
		var row := _cells[i + 1]
		var c := row / _chunk_rows
		if not _shown.has(c):
			_show(c, true)
		var tex: Texture2D = _shown.get(c)
		if tex == null:
			continue
		var fx := _cells[i + 4]
		var fw := _cells[i + 5]
		var src := Rect2(
			(_cells[i] * TILE_W + fx) * _k, _margin + (row - c * _chunk_rows) * TILE_H * _k, fw * _k, TILE_H * _k
		)
		var dst := Rect2((Vector2(_cells[i + 2], _cells[i + 3]) + origin) * _k, Vector2(fw, TILE_H) * _k)
		draw_texture_rect_region(tex, dst, src)


## Decoding and upload in a background task (the rendering API is safe from
## another thread): the main frame only sets the texture.
func _decode(c: int) -> void:
	var tex := _load(c)
	_mutex.lock()
	_decoded[c] = tex
	_mutex.unlock()


func _load(c: int) -> ImageTexture:
	var img := Image.load_from_file(_path(c))
	return ImageTexture.create_from_image(img) if img != null else null


func _take(c: int) -> ImageTexture:
	_mutex.lock()
	var tex: ImageTexture = _decoded.get(c)
	_decoded.erase(c)
	_mutex.unlock()
	return tex


func _path(c: int) -> String:
	return _dir.path_join("%03d.webp" % c)


## Loaded strips (tests, memory measurement).
func loaded_chunks() -> Array:
	return _shown.keys()


func _exit_tree() -> void:
	_finish_tasks()


## A layer freed without leaving the tree also waits for its tasks.
func _notification(what: int) -> void:
	if what == NOTIFICATION_PREDELETE:
		_finish_tasks()


func _finish_tasks() -> void:
	for id in _tasks.values():
		WorkerThreadPool.wait_for_task_completion(id)
	_tasks.clear()
	_decoded.clear()
