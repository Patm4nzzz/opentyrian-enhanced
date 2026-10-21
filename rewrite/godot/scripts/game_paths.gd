## Folders of the files SHIPPED with the exported game: data/ (original game
## data), mods/ (example mods) and upscaled/ (HD (AI) images). The player's
## own files (settings, saves, replays, screenshots, mods) live in UserData.
##
## Windows and Linux: next to the executable (unzipped package folder).
## macOS: the executable is `<App>.app/Contents/MacOS/<name>`; the shipped
## files are in `<App>.app/Contents/Resources/` (rewrite/scripts/package.sh
## --only=macos), the standard place of an application bundle's resources. The bundle
## is signed and may be translocated (read-only copy): nothing is ever written there.
##
## HD (AI) images on macOS: first `Contents/Resources/upscaled` (full package,
## the default), then `upscaled/` in the player data folder
## (~/Library/Application Support/OpenTyrian2000 Enhanced/upscaled): the
## separate HD pack is unzipped there, never into the bundle (that would
## break its signature).
##
## The `*_for` functions are stateless (OS name and executable path passed in):
## tested for the three systems on any machine (test/game_paths_test.gd).
class_name GamePaths
extends RefCounted

const MACOS := "macOS"


## Folder of the shipped files for an OS (OS.get_name()) and an executable path.
static func shipped_dir_for(os_name: String, exe_path: String) -> String:
	var exe_dir := exe_path.get_base_dir()
	if os_name == MACOS:
		return exe_dir.path_join("../Resources").simplify_path()
	return exe_dir


## Folder of the shipped files of the running game.
static func shipped_dir() -> String:
	return shipped_dir_for(OS.get_name(), OS.get_executable_path())


## Shipped file or subfolder of the running game.
static func shipped(relative: String) -> String:
	return shipped_dir().path_join(relative)


## upscaled/ folders to try, in order of preference.
static func upscaled_candidates_for(os_name: String, exe_path: String, user_dir: String) -> PackedStringArray:
	var out := PackedStringArray([shipped_dir_for(os_name, exe_path).path_join("upscaled")])
	if os_name == MACOS:
		out.append(user_dir.path_join("upscaled"))
	return out


## upscaled/ folders of the running game, in order of preference.
static func upscaled_candidates() -> PackedStringArray:
	return upscaled_candidates_for(OS.get_name(), OS.get_executable_path(), UserData.dir())
