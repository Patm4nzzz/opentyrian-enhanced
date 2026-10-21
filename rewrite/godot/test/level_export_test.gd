# Export from the level editor's "Tyrian" dock ("Export to the mod"):
# the events of a level scene, plus an enemy added by name,
# become a levels/epN_NN.json with NAMED events (ADR 0128), which the
# mod loader reads back without error. Same call as the plugin
# (addons/tyrian_level_editor/plugin.gd:export_to_mod).
extends GdUnitTestSuite


func test_editor_export_writes_named_events() -> void:
	assert_bool(ClassDB.class_exists("TyrianMods")).override_failure_message("GDExtension missing").is_true()
	var api: RefCounted = ClassDB.instantiate("TyrianMods")
	api.load(Game.data_dir, PackedStringArray())
	var res: LevelEventResource = load("res://levels/ep1_09_events.tres").duplicate(true)
	var events: Array[Dictionary] = res.events
	var original_count := events.size()
	# "Add an enemy here": type 15, enemy referred to by name.
	events.insert(
		0,
		{
			"eventtime": 0,
			"eventtype": 15,
			"eventdat": "tech_corridor_32",
			"eventdat2": 130,
			"eventdat3": 0,
			"eventdat4": 0,
			"eventdat5": 0,
			"eventdat6": 0
		}
	)
	var out: Dictionary = api.export_level(1, 9, events, res.random_enemies)
	assert_str(String(out["error"])).is_empty()
	var text := String(out["text"])
	assert_str(text).contains('"spawn": "tech_corridor_32", "x": 130')
	assert_str(text).not_contains("eventtype")
	assert_str(text).not_contains("eventdat")
	var parsed: Variant = JSON.parse_string(text)
	assert_object(parsed).is_not_null()
	assert_int((parsed["events"] as Array).size()).is_equal(original_count + 1)
	# Read back by the mod loader (mod written to a temporary folder).
	var root := OS.get_user_data_dir().path_join("export_test_mods")
	var dir := root.path_join("exported")
	DirAccess.make_dir_recursive_absolute(dir.path_join("levels"))
	var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
	f.store_string('{"format_version": 1, "name": "Exported"}')
	f.close()
	var lf := FileAccess.open(dir.path_join("levels/ep1_09.json"), FileAccess.WRITE)
	lf.store_string(text)
	lf.close()
	var reader: RefCounted = ClassDB.instantiate("TyrianMods")
	reader.load(Game.data_dir, PackedStringArray([root]))
	var errors := []
	for d in reader.diagnostics(1):
		if d["severity"] == "error":
			errors.append(d["message"])
	assert_array(errors).is_empty()
	assert_str(String(reader.level_override_source(1, 9))).is_equal("exported")
	# Cleanup.
	DirAccess.remove_absolute(dir.path_join("levels/ep1_09.json"))
	DirAccess.remove_absolute(dir.path_join("levels"))
	DirAccess.remove_absolute(dir.path_join("mod.json"))
	DirAccess.remove_absolute(dir)
	DirAccess.remove_absolute(root)


func test_editor_export_refuses_unnamed_enemy() -> void:
	var api: RefCounted = ClassDB.instantiate("TyrianMods")
	api.load(Game.data_dir, PackedStringArray())
	var events: Array[Dictionary] = [
		{
			"eventtime": 0,
			"eventtype": 15,
			"eventdat": 900,
			"eventdat2": 130,
			"eventdat3": 0,
			"eventdat4": 0,
			"eventdat5": 0,
			"eventdat6": 0
		}
	]
	var out: Dictionary = api.export_level(1, 9, events, PackedInt32Array())
	assert_str(String(out["error"])).contains("has no name")
