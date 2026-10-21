## Tyrian level editor plugin (ADR 0005, authoring kit ADR 0082).
##
## Adds a "Tyrian" dock: pick the start line (layer 1) in the open level
## scene (res://levels/epN_NN.tscn), then "Play from here" launches the game
## scene at the matching scroll position
## (the game calls TyrianSim.fast_forward_to(cur_loc)).
##
## Mod kit: "Add an enemy here" adds an event at the chosen line;
## "Export to the mod" writes userdata/mods/<mod>/ (UserData) (template
## res://../mod_template + levels/epN_NN.json: the level's events,
## named by TyrianMods.export_level - same table as the Rust reader);
## "Build the .zip package" produces userdata/mods/<mod>.zip to distribute.
@tool
extends EditorPlugin

const MOD_TEMPLATE := "res://../mod_template"
static var USER_MODS: String = UserData.path("mods")

var _dock: EditorDock
var _row: SpinBox
var _info: Label
var _mod_name: LineEdit
var _enemy: LineEdit
var _x: SpinBox
var _status: Label
## Events modified since the last export ("Play from here" exports first).
var _dirty := false


func _enter_tree() -> void:
	# EditorDock (Godot 4.6+) replaces add_control_to_dock.
	_dock = EditorDock.new()
	_dock.name = "Tyrian"
	_dock.title = "Tyrian"
	_dock.default_slot = EditorDock.DOCK_SLOT_RIGHT_UL
	var box := VBoxContainer.new()
	_dock.add_child(box)
	_info = Label.new()
	_info.autowrap_mode = TextServer.AUTOWRAP_WORD
	_info.text = "Open a res://levels/epN_NN.tscn scene"
	box.add_child(_info)
	var row_box := HBoxContainer.new()
	var l := Label.new()
	l.text = "Start row (layer 1)"
	row_box.add_child(l)
	_row = SpinBox.new()
	_row.min_value = 0
	_row.max_value = LevelMap.START_ROW
	_row.value = LevelMap.START_ROW
	_row.value_changed.connect(func(_v: float) -> void: _update_info())
	_row.get_line_edit().select_all_on_focus = true
	row_box.add_child(_row)
	box.add_child(row_box)
	var from_view := Button.new()
	from_view.text = "Row at the top of the 2D view"
	from_view.pressed.connect(_row_from_viewport)
	box.add_child(from_view)
	var play := Button.new()
	play.text = "Play from here"
	play.pressed.connect(play_from_here)
	box.add_child(play)

	box.add_child(HSeparator.new())
	var kit := Label.new()
	kit.text = "Mod"
	box.add_child(kit)
	_mod_name = _labeled(box, "Folder name", LineEdit.new())
	_mod_name.text = "my_first_level"
	# Name of a new enemy from the mod ("enemies" section) or of an original
	# enemy (rewrite/mods/original-names.json).
	_enemy = _labeled(box, "Enemy (name)", LineEdit.new())
	_enemy.text = "my_drone"
	_x = _labeled(box, "Position (x)", _spin(-200, 400, 130))
	var add := Button.new()
	add.text = "Add an enemy here"
	add.pressed.connect(add_enemy_here)
	box.add_child(add)
	var export := Button.new()
	export.text = "Export to the mod"
	export.pressed.connect(export_to_mod)
	box.add_child(export)
	var pack := Button.new()
	pack.text = "Build the .zip package"
	pack.pressed.connect(package_mod)
	box.add_child(pack)
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD
	box.add_child(_status)
	add_dock(_dock)
	_dock.make_visible()
	scene_changed.connect(
		func(root: Node) -> void:
			_lock_layers(root)
			_update_info()
	)
	_update_info()
	print_verbose("tyrian_level_editor: dock added")
	# Automatic level opening (editor screenshot):
	# TYRIAN_EDITOR_OPEN=res://levels/ep1_09.tscn
	var to_open := OS.get_environment("TYRIAN_EDITOR_OPEN")
	if not to_open.is_empty():
		_open_level.call_deferred(to_open)


func _open_level(path: String) -> void:
	EditorInterface.open_scene_from_path(path)
	# The editor restores its layout after loading: wait a little.
	await get_tree().create_timer(3.0).timeout
	EditorInterface.set_main_screen_editor("2D")
	_dock.make_visible()
	print_verbose("tyrian_level_editor: %s opened in 2D" % path)
	await get_tree().process_frame
	_row.value = LevelMap.START_ROW
	# Frame the 2D view on the bottom of the map (start of the level).
	var vp := EditorInterface.get_editor_viewport_2d()
	var ct := Transform2D.IDENTITY.scaled(Vector2(2, 2))
	ct.origin = -Vector2(0, (LevelMap.START_ROW - 6) * LevelMapBuilder.TILE_SIZE.y) * 2.0
	vp.global_canvas_transform = ct
	_update_info()


func _exit_tree() -> void:
	if _dock != null:
		remove_dock(_dock)
		_dock.queue_free()
		_dock = null


## Layers 1 to 3 locked against accidental dragging in the 2D view
## (same mark as the editor's padlock; only tile painting modifies them).
func _lock_layers(root: Node) -> void:
	if not root is LevelMap:
		return
	for i in 3:
		var layer := (root as LevelMap).layer(i)
		if layer != null and not layer.get_meta("_edit_lock_", false):
			layer.set_meta("_edit_lock_", true)


func _edited_level() -> LevelMap:
	return EditorInterface.get_edited_scene_root() as LevelMap


func _update_info() -> void:
	if _info == null:
		return
	var lvl := _edited_level()
	if lvl == null:
		_info.text = "Open a res://levels/epN_NN.tscn scene"
		return
	_info.text = (
		"ep%d level %d (%s) — cur_loc %d"
		% [lvl.episode, lvl.lvl_file_num, lvl.tileset_name, LevelMap.cur_loc_for_row(int(_row.value))]
	)


## Layer 1 line at the top of the editor's 2D view.
func _row_from_viewport() -> void:
	var lvl := _edited_level()
	if lvl == null:
		return
	var vp := EditorInterface.get_editor_viewport_2d()
	var top_left := vp.global_canvas_transform.affine_inverse() * Vector2.ZERO
	var layer := lvl.layer(0)
	var local := layer.to_local(top_left) if layer != null else top_left
	_row.value = clampi(int(floor(local.y / LevelMapBuilder.TILE_SIZE.y)), 0, LevelMap.START_ROW)


func play_from_here() -> void:
	var lvl := _edited_level()
	if lvl == null:
		push_warning("Play from here: no level scene is open")
		return
	# The game reads events from the files (original level + mods):
	# events added in the editor only exist once exported.
	if _dirty:
		export_to_mod()
	# The game rereads the request at startup and passes it to Game.start_level_at
	# (same path as the Level editor screen, ADR 0126).
	LevelMap.write_start_request(lvl.episode, lvl.lvl_file_num, LevelMap.cur_loc_for_row(int(_row.value)))
	EditorInterface.play_custom_scene("res://scenes/level.tscn")


func _spin(lo: int, hi: int, value: int) -> SpinBox:
	var sb := SpinBox.new()
	sb.min_value = lo
	sb.max_value = hi
	sb.value = value
	# A click selects the value: typing replaces it instead of appending to it.
	sb.get_line_edit().select_all_on_focus = true
	return sb


func _labeled(box: Container, text: String, control: Control) -> Control:
	var h := HBoxContainer.new()
	var l := Label.new()
	l.text = text
	l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	h.add_child(l)
	control.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	h.add_child(control)
	box.add_child(h)
	return control


func _say(text: String) -> void:
	_status.text = text
	print("tyrian_level_editor: ", text)


## Adds a "sky enemy" (type 15, level_events.c; events-spec.md) at the
## scroll position of the chosen line, in eventtime order.
func add_enemy_here() -> void:
	var lvl := _edited_level()
	if lvl == null or lvl.events == null:
		_say("Open a level scene first")
		return
	var t := LevelMap.cur_loc_for_row(int(_row.value))
	var who := _enemy.text.strip_edges()
	if who.is_empty() or who.is_valid_int():
		_say("Enemy: a name is expected (an enemy of the mod or from rewrite/mods/original-names.json)")
		return
	# Same check as the game: an unknown name would make the game refuse the whole mod level.
	var known := _known_enemy_names(lvl.episode)
	if not known.is_empty() and not known.has(who):
		_say(
			(
				# gdlint: ignore=max-line-length
				"Unknown enemy '%s': use an enemy defined in the mod (mod.json, section enemies) or an original one (rewrite/mods/original-names.json)"
				% who
			)
		)
		return
	# The name stays a name in the resource: the export writes it as is.
	var ev := {
		"eventtime": t,
		"eventtype": 15,
		"eventdat": who,
		"eventdat2": int(_x.value),
		"eventdat3": 0,
		"eventdat4": 0,
		"eventdat5": 0,
		"eventdat6": 0
	}
	# Undoable (Ctrl+Z / Ctrl+Y): the action is attached to the level scene (undoing marks it unsaved).
	var undo := get_undo_redo()
	undo.create_action("Add an enemy here", UndoRedo.MERGE_DISABLE, lvl)
	undo.add_do_method(self, "_insert_event", lvl.events, ev)
	undo.add_undo_method(self, "_remove_event", lvl.events, ev)
	undo.commit_action()
	_say(
		(
			"Event added: enemy %s at cur_loc %d (no. %d / %d)"
			% [str(ev["eventdat"]), t, lvl.events.events.find(ev), lvl.events.events.size()]
		)
	)


## Inserts the event in eventtime order (called by "Add an enemy here" and its redo).
func _insert_event(res: LevelEventResource, ev: Dictionary) -> void:
	var list: Array[Dictionary] = res.events
	var at := list.size()
	for i in list.size():
		if int(list[i]["eventtime"]) > int(ev["eventtime"]):
			at = i
			break
	list.insert(at, ev)
	res.events = list
	res.emit_changed()
	_dirty = true


## Removes the added event (undo). Compared by dictionary identity.
func _remove_event(res: LevelEventResource, ev: Dictionary) -> void:
	var list: Array[Dictionary] = res.events
	for i in list.size():
		if is_same(list[i], ev):
			list.remove_at(i)
			break
	res.events = list
	res.emit_changed()
	_dirty = true


## Enemy names accepted by the game: those of the mod (or of the template if it is not
## exported yet) and the original enemies.
## Empty if the list is not available (missing extension): no check in that case.
func _known_enemy_names(episode: int) -> PackedStringArray:
	var api := _mods_api()
	if api == null:
		return PackedStringArray()
	var dir := _mod_dir()
	var source := (
		ProjectSettings.globalize_path(dir)
		if not dir.is_empty() and FileAccess.file_exists(dir.path_join("mod.json"))
		else ProjectSettings.globalize_path(MOD_TEMPLATE).simplify_path()
	)
	return api.enemy_names(source, episode)


func _mods_api() -> RefCounted:
	if not ClassDB.class_exists("TyrianMods"):
		return null
	var api: RefCounted = ClassDB.instantiate("TyrianMods")
	api.load(ProjectSettings.globalize_path("res://").path_join("../../data").simplify_path(), PackedStringArray())
	return api


## Errors the game would raise for the mod `dir` (same code as `tyrian-mods check`).
func _mod_errors(dir: String, episode: int) -> PackedStringArray:
	var api := _mods_api()
	if api == null:
		return PackedStringArray()
	return api.check_mod(ProjectSettings.globalize_path(dir), episode)


## Mod folder name: lowercase, spaces replaced by "_"; empty if the name is not a valid file name.
func _mod_folder() -> String:
	var name := _mod_name.text.strip_edges().to_lower().replace(" ", "_")
	if name.is_empty() or not name.is_valid_filename():
		return ""
	return name


func _mod_dir() -> String:
	var name := _mod_folder()
	return "" if name.is_empty() else USER_MODS.path_join(name)


## States the name actually used when it differs from the input.
func _folder_note() -> String:
	var typed := _mod_name.text.strip_edges()
	var used := _mod_folder()
	if used.is_empty() or used == typed:
		return ""
	return " [folder name rule: lowercase, spaces become _ — '%s' is written as '%s']" % [typed, used]


## Writes the mod: copy of the template (if it does not exist yet) then
## levels/epN_NN.json with ALL the events of the open level ("events":
## replaces the original list, see tyrian-mods/src/levels.rs).
func export_to_mod() -> void:
	var lvl := _edited_level()
	var dir := _mod_dir()
	if lvl == null or lvl.events == null or dir.is_empty():
		_say('Open a level scene and enter a valid folder name (no / \\ : * ? " < > | and not empty)')
		return
	var abs_dir := ProjectSettings.globalize_path(dir)
	DirAccess.make_dir_recursive_absolute(abs_dir.path_join("levels"))
	if not FileAccess.file_exists(dir.path_join("mod.json")):
		var tpl := FileAccess.get_file_as_string(MOD_TEMPLATE.path_join("mod.json"))
		if tpl.is_empty():
			_say("Mod template not found: " + ProjectSettings.globalize_path(MOD_TEMPLATE))
			return
		tpl = tpl.replace("My First Level", _mod_name.text.strip_edges())
		var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
		f.store_string(tpl)
		f.close()
		var readme := FileAccess.get_file_as_string(MOD_TEMPLATE.path_join("README.md"))
		var r := FileAccess.open(dir.path_join("README.md"), FileAccess.WRITE)
		r.store_string(readme)
		r.close()
	# Named events (ADR 0128): translated by the Rust code, with the type table
	# and the original content names used by the mod reader.
	if not ClassDB.class_exists("TyrianMods"):
		_say("GDExtension missing: build tyrian-godot (scripts/build-linux.sh)")
		return
	var api: RefCounted = ClassDB.instantiate("TyrianMods")
	var data := ProjectSettings.globalize_path("res://").path_join("../../data").simplify_path()
	api.load(data, PackedStringArray())
	var out: Dictionary = api.export_level(lvl.episode, lvl.lvl_file_num, lvl.events.events, lvl.events.random_enemies)
	if not String(out["error"]).is_empty():
		_say("Export failed: " + String(out["error"]))
		return
	var path := dir.path_join("levels/ep%d_%02d.json" % [lvl.episode, lvl.lvl_file_num])
	var previous := FileAccess.get_file_as_string(path) if FileAccess.file_exists(path) else ""
	var lf := FileAccess.open(path, FileAccess.WRITE)
	lf.store_string(String(out["text"]))
	lf.close()
	# Same validation as the game: an invalid mod is refused, the previous file is restored.
	var errors := _mod_errors(dir, lvl.episode)
	if not errors.is_empty():
		if previous.is_empty():
			DirAccess.remove_absolute(ProjectSettings.globalize_path(path))
		else:
			var rf := FileAccess.open(path, FileAccess.WRITE)
			rf.store_string(previous)
			rf.close()
		_say("Export refused, the mod is invalid: " + "; ".join(errors))
		return
	_dirty = false
	_say(
		(
			"Mod exported: %s (%d events)%s"
			% [ProjectSettings.globalize_path(path), lvl.events.events.size(), _folder_note()]
		)
	)


## Distributable package: userdata/mods/<mod>.zip containing <mod>/... (to extract
## into the player's userdata/mods).
func package_mod() -> void:
	var dir := _mod_dir()
	if dir.is_empty() or not FileAccess.file_exists(dir.path_join("mod.json")):
		_say("Export the mod first")
		return
	var lvl := _edited_level()
	var errors := _mod_errors(dir, lvl.episode if lvl != null else 1)
	if not errors.is_empty():
		_say("Package refused, the mod is invalid: " + "; ".join(errors))
		return
	var name := dir.get_file()
	var zip_path := ProjectSettings.globalize_path(USER_MODS.path_join(name + ".zip"))
	var zip := ZIPPacker.new()
	if zip.open(zip_path) != OK:
		_say("Cannot create " + zip_path)
		return
	var n := _zip_dir(zip, ProjectSettings.globalize_path(dir), name)
	zip.close()
	_say("Package built: %s (%d files)%s" % [zip_path, n, _folder_note()])


func _zip_dir(zip: ZIPPacker, abs_dir: String, prefix: String) -> int:
	var n := 0
	var d := DirAccess.open(abs_dir)
	if d == null:
		return 0
	for f in d.get_files():
		zip.start_file(prefix.path_join(f))
		zip.write_file(FileAccess.get_file_as_bytes(abs_dir.path_join(f)))
		zip.close_file()
		n += 1
	for sub in d.get_directories():
		n += _zip_dir(zip, abs_dir.path_join(sub), prefix.path_join(sub))
	return n
