# HD (AI) mode scenery, ADR 0100: the key of a layer (name of its folder
# in upscaled/maps/) must be identical in GDScript and in
# scripts/hd_upscale.py, otherwise the game does not find the images and keeps
# the tiles. Reference values: `hd_upscale.py key godot/assets/imported`.
# Toggle during a level (F3 / F10): HD scenery in place of the tiles, and
# back to the original tiles.
extends GdUnitTestSuite

var _saved: PackedByteArray
var _had_file := false


func before_test() -> void:
	_had_file = FileAccess.file_exists(DisplaySettings.PATH)
	if _had_file:
		_saved = FileAccess.get_file_as_bytes(DisplaySettings.PATH)


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	if _had_file:
		var f := FileAccess.open(DisplaySettings.PATH, FileAccess.WRITE)
		f.store_buffer(_saved)
		f.close()
	DisplaySettings._loaded = false
	Effects.apply_display_settings()
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _blank(tileset_name: String) -> Dictionary:
	var blank := {}
	for n in TyrianAssets.read_json("tiles/%s.json" % tileset_name)["blank_tiles"]:
		blank[int(n)] = true
	return blank


func test_layer_keys_match_the_upscale_script() -> void:
	var level: Dictionary = TyrianAssets.read_json_uncached("levels/ep1_09.json")
	var blank := _blank(level["tileset"])
	var expected := ["5652d9f225b85edb", "feaed82f5e0312dd", "e387d43cb8498a7f"]
	for i in 3:
		assert_str(HdMapLayer.layer_key(level["tileset"], level["layers"][i], blank)).is_equal(expected[i])


func test_missing_images_keep_the_tiles() -> void:
	var level: Dictionary = TyrianAssets.read_json_uncached("levels/ep1_09.json")
	var hd := HdMapLayer.open("user://no-such-dir", level["tileset"], level["layers"][0], _blank(level["tileset"]))
	assert_object(hd).is_null()


func test_switching_art_mode_in_a_level_swaps_the_scenery() -> void:
	var key := HdMapLayer.layer_key(
		"shapesz", TyrianAssets.read_json_uncached("levels/ep1_09.json")["layers"][0], _blank("shapesz")
	)
	if not FileAccess.file_exists(DisplaySettings.UPSCALED.path_join("maps/%s/layer.json" % key)):
		return  # HD images missing (hd_assets.sh fetch not run): nothing to toggle
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.ORIGINAL)
	Effects.apply_display_settings()
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	var layer: TileMapLayer = level.map.layer(0)
	assert_object(layer.get_node_or_null("HdMap")).is_null()
	assert_float(layer.self_modulate.a).is_equal(1.0)
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	Effects.apply_display_settings()
	var hd := layer.get_node_or_null("HdMap") as HdMapLayer
	assert_object(hd).is_not_null()
	assert_bool(hd.visible).is_true()
	assert_float(layer.self_modulate.a).is_equal(0.0)
	assert_array(hd.loaded_chunks()).is_not_empty()
	assert_object(level._playfield_bg).is_null()
	# Starfield: pixel-exact overlay above HD layer 1.
	level.step_sim(0)
	var stars: Sprite2D = level.get_node_or_null("HdStars")
	assert_object(stars).is_not_null()
	assert_bool(stars.visible).is_true()
	assert_int(stars.texture_filter).is_equal(CanvasItem.TEXTURE_FILTER_NEAREST)
	assert_vector(stars.texture.get_size()).is_equal(Vector2(264, 184))
	# Ships, enemies and shots: lists of what the C drew, enlarged sheets,
	# per-index operations (HdPlayfieldEffects).
	for n in ["HdShips0", "HdEnemies0", "HdShots0"]:
		var node: CanvasItem = level.get_node_or_null(n)
		assert_object(node).is_not_null()
		assert_bool(node.visible).is_true()
	assert_bool(level._player_node.visible).is_false()
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.ORIGINAL)
	Effects.apply_display_settings()
	assert_bool(hd.visible).is_false()
	assert_float(layer.self_modulate.a).is_equal(1.0)
	level.step_sim(0)
	assert_bool(stars.visible).is_false()
	assert_bool(level.get_node("HdShips0").visible).is_false()


# Crash 1 time in 14 at the end of an HD level (signal 11, Mutex.lock on a
# null object in a WorkerThreadPool thread): Effects.show_scene removes the
# level, then bullet_time_reset emits effects_changed; the removed
# level relaunched update_view, which started decoding the neighboring strips,
# and the level was freed during this decoding. A layer outside the tree must not
# start any task.
func test_leaving_the_level_starts_no_decoding_task() -> void:
	var key := HdMapLayer.layer_key(
		"shapesz", TyrianAssets.read_json_uncached("levels/ep1_09.json")["layers"][0], _blank("shapesz")
	)
	if not FileAccess.file_exists(DisplaySettings.UPSCALED.path_join("maps/%s/layer.json" % key)):
		return  # HD images missing
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	Effects.apply_display_settings()
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	var layers: Array[HdMapLayer] = []
	for i in 3:
		var l: TileMapLayer = level.map.layer(i)
		if l != null and l.get_node_or_null("HdMap") != null:
			layers.append(l.get_node("HdMap"))
	assert_array(layers).is_not_empty()
	for hd in layers:
		hd._shown.clear()  # strips to reload: preloading would want to decode some
	Effects.show_scene(Game.SCENE_DEBRIEF)
	for hd in layers:
		assert_dict(hd._tasks).is_empty()
