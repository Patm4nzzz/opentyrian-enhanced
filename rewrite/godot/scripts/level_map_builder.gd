## Builds the scenery layers of an imported level (levels/epN_NN.json).
##
## Used both by the headless generator (editor/build_levels.gd) and,
## if the generated scene does not exist, by the game scene at runtime.
## Geometry: level_load.c:JE_loadMap, 24x28 tiles; layer 1: 14x300,
## layer 2: 14x600, layer 3: 15x600; row 0 at the top of the map.
class_name LevelMapBuilder
extends RefCounted

const TILE_SIZE := Vector2i(24, 28)
const LAYER_NAMES := ["Layer1", "Layer2", "Layer3"]


## TileSet built from the atlas `tiles/<name>.png` (tile n at
## ((n-1) % columns, (n-1) / columns)). Empty tiles are not created.
static func build_tileset(tileset_name: String, texture: Texture2D = null) -> TileSet:
	var meta: Dictionary = TyrianAssets.read_json("tiles/%s.json" % tileset_name)
	if texture == null:
		texture = TyrianAssets.texture("tiles/%s.png" % tileset_name)
	var ts := TileSet.new()
	ts.tile_size = TILE_SIZE
	var src := TileSetAtlasSource.new()
	src.texture = texture
	src.texture_region_size = TILE_SIZE
	var blank := {}
	for n in meta["blank_tiles"]:
		blank[int(n)] = true
	var columns := int(meta["columns"])
	for n in range(1, int(meta["tile_count"]) + 1):
		if blank.has(n):
			continue
		src.create_tile(Vector2i((n - 1) % columns, (n - 1) / columns))
	ts.add_source(src, 0)
	return ts


## Fills a TileMapLayer with a JSON layer (1-based numbers, 0 = empty).
static func fill_layer(layer: TileMapLayer, layer_json: Dictionary, columns: int) -> int:
	var w := int(layer_json["width"])
	var h := int(layer_json["height"])
	var tiles: Array = layer_json["tiles"]
	var src := layer.tile_set.get_source(0) as TileSetAtlasSource
	var count := 0
	for row in h:
		for col in w:
			var n := int(tiles[row * w + col])
			if n <= 0:
				continue
			var coords := Vector2i((n - 1) % columns, (n - 1) / columns)
			if not src.has_tile(coords):
				continue
			layer.set_cell(Vector2i(col, row), 0, coords)
			count += 1
	return count


## Node2D root (LevelMap script) with the three TileMapLayers.
static func build_map(level: Dictionary, tileset: TileSet) -> Node2D:
	var root := Node2D.new()
	root.set_script(load("res://scripts/level_map.gd"))
	root.name = "ep%d_%02d" % [int(level["episode"]), int(level["lvl_file_num"])]
	root.episode = int(level["episode"])
	root.lvl_file_num = int(level["lvl_file_num"])
	root.tileset_name = String(level["tileset"])
	var columns := int(TyrianAssets.read_json("tiles/%s.json" % level["tileset"])["columns"])
	for i in 3:
		var layer := TileMapLayer.new()
		layer.name = LAYER_NAMES[i]
		layer.tile_set = tileset
		layer.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		# Locked in the editor's 2D view: no accidental dragging.
		layer.set_meta("_edit_lock_", true)
		fill_layer(layer, level["layers"][i], columns)
		root.add_child(layer)
	return root


## Events resource (LevelEventResource) from the JSON.
static func build_events(level: Dictionary) -> LevelEventResource:
	var res := LevelEventResource.new()
	res.episode = int(level["episode"])
	res.lvl_file_num = int(level["lvl_file_num"])
	res.map_x = PackedInt32Array(
		[int(level["header"]["map_x"]), int(level["header"]["map_x2"]), int(level["header"]["map_x3"])]
	)
	res.random_enemies = PackedInt32Array(level["enemies"])
	var events: Array[Dictionary] = []
	for e in level["events"]:
		var d := {}
		for k in LevelEventResource.EVENT_KEYS:
			d[k] = int(e[k])
		events.append(d)
	res.events = events
	return res
