# Lifetime achievements (achievements.c:achievements_check, stats.c): the rules that
# do not depend on a level - games played, episode endings, Arcade Entry,
# shop purchases, languages - go through TyrianVga/TyrianSim and are written
# to opentyrian.dat (OTYE + OTY3 block), read back by the achievements screen.
extends GdUnitTestSuite

const SCREEN := preload("res://scripts/achievements_screen.gd")

var _dat := ""


func before_test() -> void:
	_dat = ProjectSettings.globalize_path("user://ach_lifetime_test.dat")
	DirAccess.remove_absolute(_dat)
	_reset_tracker()


func after_test() -> void:
	DirAccess.remove_absolute(_dat)
	_reset_tracker()


func _reset_tracker() -> void:
	var sim: Node = ClassDB.instantiate("TyrianSim")
	sim.ach_tracker_reset()
	sim.free()


## level.gd: ach_level_start creates the tracker, then ach_count_game_start counts the game.
func _game_started() -> void:
	TyrianVga.ach_ensure(_dat)
	var sim: Node = ClassDB.instantiate("TyrianSim")
	sim.ach_count_game_start()
	sim.free()


func _index(key: String) -> int:
	for i in SCREEN.ACHIEVEMENTS.size():
		if SCREEN.ACHIEVEMENTS[i][0] == key:
			return i
	return -1


func _flags() -> PackedByteArray:
	return TyrianVga.achievements_unlocked_from_dat(_dat)


func _unlocked(key: String) -> bool:
	var f := _flags()
	var i := _index(key)
	return i >= 0 and i < f.size() and f[i] != 0


func _reload_from_disk() -> void:
	# Game exit (JE_tyrianHalt writes the profile) then new launch:
	# the tracker is rebuilt from opentyrian.dat.
	TyrianVga.ach_save_current(_dat)
	_reset_tracker()
	TyrianVga.ach_ensure(_dat)


func test_ten_games_unlock_games_10_and_survive_a_restart() -> void:
	for i in 9:
		_game_started()
	TyrianVga.ach_title_check(_dat, 0, 12, 0)
	assert_bool(_unlocked("ach_games_10")).is_false()
	_game_started()
	var flags := TyrianVga.ach_title_check(_dat, 0, 12, 0)
	assert_int(flags[_index("ach_games_10")]).is_equal(1)
	assert_bool(_unlocked("ach_games_10")).override_failure_message("Games 10 not written to the profile").is_true()
	# The counter is lifetime: read back after a restart, the fiftieth game reaches 50.
	_reload_from_disk()
	for i in 39:
		_game_started()
	TyrianVga.ach_title_check(_dat, 0, 12, 0)
	assert_bool(_unlocked("ach_games_50")).is_false()
	_game_started()
	TyrianVga.ach_title_check(_dat, 0, 12, 0)
	assert_bool(_unlocked("ach_games_50")).is_true()


func test_episode_clear_and_all_episodes() -> void:
	for ep in [1, 2, 3]:
		var ids: PackedInt32Array = TyrianVga.ach_episode_done(_dat, ep, 0)
		assert_bool(ids.has(_index("ach_episode_clear"))).is_equal(ep == 1)
	assert_bool(_unlocked("ach_all_episodes")).is_false()
	var last: PackedInt32Array = TyrianVga.ach_episode_done(_dat, 4, 0)
	assert_bool(last.has(_index("ach_all_episodes"))).is_true()
	TyrianVga.ach_save_current(_dat)
	assert_bool(_unlocked("ach_all_episodes")).is_true()


func test_one_life_episode_needs_the_mutator() -> void:
	assert_bool(TyrianVga.ach_episode_done(_dat, 1, 0).has(_index("ach_one_life_episode"))).is_false()
	assert_bool(TyrianVga.ach_episode_done(_dat, 2, 0x20).has(_index("ach_one_life_episode"))).is_true()


func test_arcade_entry_by_name() -> void:
	var ids: PackedInt32Array = TyrianVga.ach_unlock_by_name(_dat, "ARCADE_ENTRY")
	assert_array(Array(ids)).is_equal([_index("ach_arcade_entry")])
	assert_int(TyrianVga.ach_unlock_by_name(_dat, "ARCADE_ENTRY").size()).is_equal(0)


func test_polyglot_needs_the_six_languages_across_launches() -> void:
	for lang in 6:
		TyrianVga.ach_title_check(_dat, lang, 12, 0)
		assert_bool(_unlocked("ach_polyglot")).is_equal(lang == 5)
		_reload_from_disk()


func test_title_check_unlocks_night_player_and_modded_run_rules_only_when_due() -> void:
	TyrianVga.ach_title_check(_dat, 0, 12, 0)
	assert_bool(_unlocked("ach_night_player")).is_false()
	TyrianVga.ach_title_check(_dat, 0, 3, 0)
	assert_bool(_unlocked("ach_night_player")).is_true()


func test_achievements_screen_reads_the_unlocked_profile() -> void:
	for i in 10:
		_game_started()
	TyrianVga.ach_title_check(_dat, 0, 12, 0)
	var flags := _flags()
	assert_int(flags.size()).is_equal(100)
	assert_int(flags[_index("ach_games_10")]).is_equal(1)
	assert_int(SCREEN.ACHIEVEMENTS.size()).is_equal(100)
