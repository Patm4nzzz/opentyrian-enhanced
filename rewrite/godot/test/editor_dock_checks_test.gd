# Level editor: dock checks before writing (known enemy name,
# valid mod) and layer lock. Same calls as
# addons/tyrian_level_editor/plugin.gd.
extends GdUnitTestSuite

const TEMPLATE := "res://../mod_template"


func _api() -> RefCounted:
	var api: RefCounted = ClassDB.instantiate("TyrianMods")
	api.load(Game.data_dir, PackedStringArray())
	return api


func test_known_enemy_names_include_mod_and_original_enemies() -> void:
	var names: PackedStringArray = _api().enemy_names(ProjectSettings.globalize_path(TEMPLATE).simplify_path(), 1)
	assert_bool(names.has("my_drone")).is_true()
	assert_bool(names.has("tech_corridor_32")).is_true()
	assert_bool(names.has("no_such_enemy")).is_false()


func test_check_mod_accepts_template_and_refuses_unknown_enemy_in_level() -> void:
	var api := _api()
	var template := ProjectSettings.globalize_path(TEMPLATE).simplify_path()
	assert_array(Array(api.check_mod(template, 1))).is_empty()
	var root := OS.get_user_data_dir().path_join("editor_check_test")
	var dir := root.path_join("bad")
	DirAccess.make_dir_recursive_absolute(dir.path_join("levels"))
	var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
	f.store_string('{"format_version": 1, "name": "Bad"}')
	f.close()
	var events: Array[Dictionary] = [
		{
			"eventtime": 0,
			"eventtype": 15,
			"eventdat": "no_such_enemy",
			"eventdat2": 130,
			"eventdat3": 0,
			"eventdat4": 0,
			"eventdat5": 0,
			"eventdat6": 0
		}
	]
	var out: Dictionary = api.export_level(1, 9, events, PackedInt32Array())
	var lf := FileAccess.open(dir.path_join("levels/ep1_09.json"), FileAccess.WRITE)
	lf.store_string(String(out["text"]))
	lf.close()
	var errors: PackedStringArray = api.check_mod(dir, 1)
	assert_int(errors.size()).is_greater(0)
	assert_str(errors[0]).contains("no_such_enemy")
	DirAccess.remove_absolute(dir.path_join("levels/ep1_09.json"))
	DirAccess.remove_absolute(dir.path_join("levels"))
	DirAccess.remove_absolute(dir.path_join("mod.json"))
	DirAccess.remove_absolute(dir)
	DirAccess.remove_absolute(root)


func test_generated_level_layers_are_locked_in_the_2d_view() -> void:
	var level: Dictionary = TyrianAssets.read_json_uncached("levels/ep1_09.json")
	var ts: TileSet = load("res://levels/tilesets/shapesz.tres")
	var map := LevelMapBuilder.build_map(level, ts)
	for i in 3:
		assert_bool(map.get_node(String(LevelMapBuilder.LAYER_NAMES[i])).get_meta("_edit_lock_", false)).is_true()
	map.free()
