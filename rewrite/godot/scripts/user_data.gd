## Player data folder: profile (opentyrian.dat), settings, scores,
## saves, replays, screenshots and the player's mods.
##
## Exported game: `userdata/` subfolder next to the executable, like the C code under
## Windows which writes next to its own (config.c:get_user_directory). A single
## subfolder rather than files at the root: it does not mix with the data/
## and mods/ shipped with the game, and copying it is enough to keep one's game.
## Fallback to Godot's user folder (user://) if this folder is not
## writable (Program Files, read-only mount), always under macOS
## (~/Library/Application Support/OpenTyrian2000 Enhanced: writing into the .app
## bundle breaks its signature, and a translocated app is read-only; the
## files shipped with the game are found by GamePaths) and in the editor (development project,
## tests isolated by XDG_DATA_HOME).
## `--user-dir=DIR` (after `--`) imposes the folder: reserved for test tools
## that launch several games from the same package, each with its own profile.
## Logs and shader cache stay in user:// (technical).
class_name UserData
extends RefCounted

const FOLDER_NAME := "userdata"
const OPTION := "--user-dir="

static var _dir := ""


## Absolute path of the folder, created on the first call.
static func dir() -> String:
	if _dir.is_empty():
		_dir = resolve(
			OS.get_cmdline_user_args(),
			OS.get_executable_path().get_base_dir(),
			uses_godot_user_dir(OS.get_name(), OS.has_feature("editor")),
			OS.get_user_data_dir()
		)
	return _dir


## True if the player data goes straight to Godot's user folder rather than
## `userdata/` next to the executable: in the editor, and always under macOS
## (inside a .app bundle, read-only or translocated). On macOS, user:// is
## ~/Library/Application Support/OpenTyrian2000 Enhanced (project.godot:
## application/config/use_custom_user_dir.macos). Stateless (tested).
static func uses_godot_user_dir(os_name: String, is_editor: bool) -> bool:
	return is_editor or os_name == GamePaths.MACOS


## Absolute path of a file or subfolder of the player data folder.
static func path(relative: String) -> String:
	return dir().path_join(relative)


## Folder choice, stateless (testable): `--user-dir=` option, otherwise
## `<exe_dir>/userdata` if writable, otherwise `fallback`.
static func resolve(user_args: PackedStringArray, exe_dir: String, use_fallback: bool, fallback: String) -> String:
	for arg in user_args:
		if arg.begins_with(OPTION):
			var forced := arg.trim_prefix(OPTION).simplify_path()
			DirAccess.make_dir_recursive_absolute(forced)
			return forced
	if not use_fallback:
		var beside := exe_dir.path_join(FOLDER_NAME)
		if is_writable(beside):
			return beside
		print("Game folder not writable (%s): player data in %s" % [beside, fallback])
	DirAccess.make_dir_recursive_absolute(fallback)
	return fallback


## True if a file can be written in the folder (created if needed). Write test
## in the parent folder before creating it: a read-only game folder
## leaves neither an empty folder nor an engine error in the log.
static func is_writable(folder: String) -> bool:
	if not DirAccess.dir_exists_absolute(folder):
		if not _can_write_in(folder.get_base_dir()):
			return false
		if DirAccess.make_dir_recursive_absolute(folder) != OK:
			return false
	return _can_write_in(folder)


static func _can_write_in(folder: String) -> bool:
	var probe := folder.path_join(".write_test")
	var f := FileAccess.open(probe, FileAccess.WRITE)
	if f == null:
		return false
	f.close()
	DirAccess.remove_absolute(probe)
	return true
