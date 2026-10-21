## Endless launched from the menu (menus.c, MENU_ITEM_1/2_PLAYER_ENDLESS): seed drawn
## from the time like endless.c:endless_start (rng_seed(time(NULL))), except "--seed=N".
extends GdUnitTestSuite

var _seed_before: int
var _forced_before: bool


func before_test() -> void:
	_seed_before = Game.rng_seed
	_forced_before = Game.seed_forced
	EndlessSession.clear_resume()


func after_test() -> void:
	EndlessSession.stop()
	Game.rng_seed = _seed_before
	Game.seed_forced = _forced_before
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


## Without --seed: the game's seed follows the time (no longer seed 1 on every launch).
func test_menu_endless_seed_follows_time() -> void:
	Game.seed_forced = false
	Game.rng_seed = 1
	var now := int(Time.get_unix_time_from_system())
	assert_bool(EndlessSession.enter(false)).is_true()
	await get_tree().process_frame
	assert_int(EndlessSession.run_seed).override_failure_message("seed drawn from the time like the C").is_between(
		now - 5, now + 5
	)


## --seed=N forces the seed (replayable runs and golden masters).
func test_forced_seed_is_kept() -> void:
	Game.seed_forced = true
	Game.rng_seed = 7
	assert_bool(EndlessSession.enter(false)).is_true()
	await get_tree().process_frame
	assert_int(EndlessSession.run_seed).is_equal(7)
