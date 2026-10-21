# Player data folder next to the executable (the maintainer's decision of
# 2026-10-11) and C profile import that never takes the game's file.
extends GdUnitTestSuite

var _root := ""


func before_test() -> void:
	var tmp := OS.get_environment("TMPDIR") if not OS.get_environment("TMPDIR").is_empty() else "/tmp"
	_root = tmp.path_join("user_data_test_%d" % Time.get_ticks_usec())
	DirAccess.make_dir_recursive_absolute(_root.path_join("game"))


func after_test() -> void:
	OS.execute("rm", ["-rf", _root])


func test_exported_game_writes_in_userdata_beside_executable() -> void:
	var game := _root.path_join("game")
	var dir := UserData.resolve(PackedStringArray(), game, false, _root.path_join("fallback"))
	assert_str(dir).is_equal(game.path_join("userdata"))
	assert_bool(DirAccess.dir_exists_absolute(dir)).is_true()
	# The write-test file does not remain.
	assert_array(Array(DirAccess.get_files_at(dir))).is_empty()


func test_user_dir_option_wins() -> void:
	var forced := _root.path_join("tool_profile")
	var args := PackedStringArray(["--windowed", "--user-dir=" + forced])
	assert_str(UserData.resolve(args, _root.path_join("game"), false, _root.path_join("fallback"))).is_equal(forced)
	assert_bool(DirAccess.dir_exists_absolute(forced)).is_true()


func test_read_only_game_folder_falls_back_to_godot_user_dir() -> void:
	# /proc: no folder can be created there, even for root (chmod 555 does not stop it).
	var fallback := _root.path_join("fallback")
	assert_bool(UserData.is_writable("/proc/userdata")).is_false()
	assert_str(UserData.resolve(PackedStringArray(), "/proc", false, fallback)).is_equal(fallback)
	assert_bool(DirAccess.dir_exists_absolute(fallback)).is_true()


func test_editor_and_macos_use_godot_user_dir() -> void:
	var fallback := _root.path_join("fallback")
	assert_str(UserData.resolve(PackedStringArray(), _root.path_join("game"), true, fallback)).is_equal(fallback)
	assert_bool(DirAccess.dir_exists_absolute(_root.path_join("game/userdata"))).is_false()


# Trap 1: same file name (opentyrian.dat) for the C profile and the
# game's. The game's file is never taken for a C profile, even when the
# data folder points at the executable's folder.
func test_c_profile_import_never_takes_the_game_own_file() -> void:
	var game := _root.path_join("game")
	var own := game.path_join(CProfile.FILE_NAME)
	var sibling := _root.path_join("OpenTyrian2000").path_join(CProfile.FILE_NAME)
	DirAccess.make_dir_recursive_absolute(sibling.get_base_dir())
	for p in [own, sibling]:
		var f := FileAccess.open(p, FileAccess.WRITE)
		f.store_string(p)
		f.close()
	var candidates: Array[String] = [own, game.path_join("../game/./") + CProfile.FILE_NAME, sibling]
	assert_str(CProfile.first_c_profile(candidates, own)).is_equal(sibling)
	# Without a neighboring C profile: nothing to import.
	DirAccess.remove_absolute(sibling)
	assert_str(CProfile.first_c_profile(candidates, own)).is_equal("")


func test_default_game_file_is_not_a_c_candidate() -> void:
	for p in CProfile.candidate_paths():
		assert_str(p.simplify_path()).is_not_equal(UserData.path(CProfile.FILE_NAME).simplify_path())
