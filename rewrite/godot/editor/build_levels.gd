## Headless generator for the level scenes (ADR 0005).
##
##   godot --headless --path rewrite/godot res://editor/build_levels.tscn [-- --only=ep1_09]
##
## Launched as a scene (not with `--script`) so that the autoloads (Game...)
## exist: TyrianAssets and the scripts it loads use them.
##
## For each assets/imported/levels/epN_NN.json: one shared TileSet per tile
## set (res://levels/tilesets/<shapes>.tres), the events resource
## (res://levels/epN_NN_events.tres, LevelEventResource) and the scene
## res://levels/epN_NN.tscn (LevelMap + Layer1..Layer3 as TileMapLayer).
extends Node

const OUT_DIR := "res://levels"


func _ready() -> void:
	_build.call_deferred()


func _build() -> void:
	var only := ""
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--only="):
			only = arg.trim_prefix("--only=")
	var code := _run(only)
	get_tree().quit(code)


func _run(only: String) -> int:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(OUT_DIR.path_join("tilesets")))
	var dir := DirAccess.open(TyrianAssets.IMPORTED.path_join("levels"))
	if dir == null:
		printerr("build_levels: assets/imported/levels missing - run tyrian-import first")
		return 1
	var files := Array(dir.get_files()).filter(func(f: String) -> bool: return f.ends_with(".json"))
	files.sort()
	var tilesets := {}
	var built := 0
	for f in files:
		var id: String = f.get_basename()
		if not only.is_empty() and id != only:
			continue
		var level: Dictionary = TyrianAssets.read_json_uncached("levels/" + f)
		var ts_name := String(level["tileset"])
		if not tilesets.has(ts_name):
			tilesets[ts_name] = _tileset(ts_name)
			if tilesets[ts_name] == null:
				return 1
		var map := LevelMapBuilder.build_map(level, tilesets[ts_name])
		var events := LevelMapBuilder.build_events(level)
		var events_path := OUT_DIR.path_join("%s_events.tres" % id)
		if ResourceSaver.save(events, events_path) != OK:
			printerr("build_levels: write failed", events_path)
			return 1
		map.events = load(events_path)
		for child in map.get_children():
			child.owner = map
		var packed := PackedScene.new()
		packed.pack(map)
		var scene_path := OUT_DIR.path_join("%s.tscn" % id)
		if ResourceSaver.save(packed, scene_path) != OK:
			printerr("build_levels: write failed", scene_path)
			return 1
		map.free()
		built += 1
	print("build_levels: %d level scenes in %s" % [built, OUT_DIR])
	return 0 if built > 0 else 1


## TileSet saved as .tres, with the imported atlas texture
## (external reference, no embedded image).
func _tileset(ts_name: String) -> TileSet:
	var tex_path := TyrianAssets.IMPORTED.path_join("tiles/%s.png" % ts_name)
	if not ResourceLoader.exists(tex_path):
		printerr("build_levels: %s not imported (run `godot --headless --import` first)" % tex_path)
		return null
	var ts := LevelMapBuilder.build_tileset(ts_name, load(tex_path))
	var path := OUT_DIR.path_join("tilesets/%s.tres" % ts_name)
	if ResourceSaver.save(ts, path) != OK:
		printerr("build_levels: write failed", path)
		return null
	return load(path)
