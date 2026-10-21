# Folders of the shipped files (data/, mods/, upscaled/) and of the player data
# for each system, simulated by the OS name and executable path passed in:
# Windows and Linux unchanged (next to the executable), macOS in the .app bundle
# (Contents/Resources) and in ~/Library/Application Support.
extends GdUnitTestSuite

const MAC_EXE := "/Applications/OpenTyrian2000 Enhanced.app/Contents/MacOS/OpenTyrian2000 Enhanced"
const MAC_RESOURCES := "/Applications/OpenTyrian2000 Enhanced.app/Contents/Resources"
const MAC_USER := "/Users/tester/Library/Application Support/OpenTyrian2000 Enhanced"


func test_windows_and_linux_ship_next_to_the_executable() -> void:
	var win_exe := "C:/Games/OpenTyrian2000-Enhanced/OpenTyrian2000-Enhanced.exe"
	assert_str(GamePaths.shipped_dir_for("Windows", win_exe)).is_equal("C:/Games/OpenTyrian2000-Enhanced")
	var linux_exe := "/home/p/OpenTyrian2000-Enhanced/OpenTyrian2000-Enhanced.x86_64"
	assert_str(GamePaths.shipped_dir_for("Linux", linux_exe)).is_equal("/home/p/OpenTyrian2000-Enhanced")


func test_macos_ships_in_the_bundle_resources() -> void:
	assert_str(GamePaths.shipped_dir_for("macOS", MAC_EXE)).is_equal(MAC_RESOURCES)


# Gatekeeper path randomization: the bundle runs from a random read-only
# mount; the resources stay relative to the executable.
func test_macos_translocated_bundle() -> void:
	var app := "/private/var/folders/x1/T/AppTranslocation/4F2A/d/OpenTyrian2000 Enhanced.app"
	var exe := app.path_join("Contents/MacOS/OpenTyrian2000 Enhanced")
	assert_str(GamePaths.shipped_dir_for("macOS", exe)).is_equal(app.path_join("Contents/Resources"))


func test_upscaled_candidates_windows_linux_unchanged() -> void:
	for os_name in ["Windows", "Linux"]:
		var got := GamePaths.upscaled_candidates_for(os_name, "/game/OpenTyrian2000-Enhanced.x86_64", "/user")
		assert_array(Array(got)).is_equal(["/game/upscaled"])


func test_upscaled_candidates_macos_bundle_then_user_folder() -> void:
	var got := GamePaths.upscaled_candidates_for("macOS", MAC_EXE, MAC_USER)
	assert_array(Array(got)).is_equal([MAC_RESOURCES + "/upscaled", MAC_USER + "/upscaled"])


func test_player_data_folder_choice_per_system() -> void:
	assert_bool(UserData.uses_godot_user_dir("Windows", false)).is_false()
	assert_bool(UserData.uses_godot_user_dir("Linux", false)).is_false()
	assert_bool(UserData.uses_godot_user_dir("macOS", false)).is_true()
	assert_bool(UserData.uses_godot_user_dir("Linux", true)).is_true()


# macOS end to end: even with a writable folder beside the executable, the
# player data goes to Godot's user folder and nothing is written in the bundle.
func test_macos_never_writes_in_the_bundle() -> void:
	var tmp := OS.get_environment("TMPDIR") if not OS.get_environment("TMPDIR").is_empty() else "/tmp"
	var root := tmp.path_join("game_paths_test_%d" % Time.get_ticks_usec())
	var macos_dir := root.path_join("Game.app/Contents/MacOS")
	var user := root.path_join("Application Support/OpenTyrian2000 Enhanced")
	DirAccess.make_dir_recursive_absolute(macos_dir)
	var dir := UserData.resolve(PackedStringArray(), macos_dir, UserData.uses_godot_user_dir("macOS", false), user)
	assert_str(dir).is_equal(user)
	assert_bool(DirAccess.dir_exists_absolute(macos_dir.path_join(UserData.FOLDER_NAME))).is_false()
	OS.execute("rm", ["-rf", root])


# project.godot: user:// is ~/Library/Application Support/OpenTyrian2000 Enhanced
# on macOS only (feature override); Windows and Linux keep Godot's default.
func test_custom_user_dir_only_on_macos() -> void:
	var cfg := ConfigFile.new()
	assert_int(cfg.load("res://project.godot")).is_equal(OK)
	assert_bool(cfg.get_value("application", "config/use_custom_user_dir.macos", false)).is_true()
	assert_bool(cfg.get_value("application", "config/use_custom_user_dir", false)).is_false()
	assert_str(cfg.get_value("application", "config/custom_user_dir_name", "")).is_equal("OpenTyrian2000 Enhanced")
