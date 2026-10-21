# Two-player Endless game. 12 checks, in order (each
# step depends on the previous one, hence a single test case).
# Simulation mode and equipment, combos active, anomaly draw after
# the wave, final score deck × mutators, save then resume screen.
extends GdUnitTestSuite

const ES := preload("res://scripts/endless_session.gd")

var _combos_saved := true


func before() -> void:
	# Setup > Gameplay combos: the machine's C profile, imported on first
	# launch, may turn them off ("combo": false); this test checks active combos.
	_combos_saved = Game.combos_enabled
	Game.combos_enabled = true


func after() -> void:
	Game.combos_enabled = _combos_saved
	# The suites share the same process: the Endless session and the
	# modes it set must not spill over onto the next one.
	ES.clear_resume()
	ES.stop()
	ES.two_player = false
	Game.modes = PackedInt32Array()
	Game.mutators = 0
	Game.anomalies = 0
	# The resumed game starts music: releasing it avoids resources
	# held at shutdown (same reason as Game.quit_cleanly).
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func _sim() -> Object:
	for n in get_tree().root.find_children("TyrianSim", "", true, false):
		return n
	return null


func _wait_until(cond: Callable, ms: int) -> void:
	var deadline := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < deadline:
		await get_tree().process_frame


func test_two_player_run_save_and_resume() -> void:
	ES.clear_resume()
	Game.rng_seed = 1
	assert_bool(ES.start(true, true)).override_failure_message("start 2 players").is_true()
	for i in 10:
		await get_tree().process_frame
	var sim := _sim()
	assert_bool(sim != null and sim.is_loaded()).override_failure_message("TyrianSim loaded").is_true()
	if sim == null:
		return
	var h: Dictionary = sim.mode_hud()
	assert_bool(bool(h.get("two_player", false))).override_failure_message("two-player mode (twoPlayerMode)").is_true()
	assert_array(Array(sim.endless_state()).slice(0, 2)).override_failure_message("lives 3:3").is_equal([3, 3])
	# Continuous fire from both players: destructions chain together.
	var best := 1
	var tiers := 0
	for t in 1500:
		if sim.level_ended():
			break
		sim.step(16, 16)
		var c: PackedInt32Array = sim.combo_state()
		best = maxi(best, c[1])
		if c[2] >= 0:
			tiers += 1
	var st: PackedInt64Array = sim.endless_state()
	(
		assert_bool(best > 1 and tiers > 0)
		. override_failure_message("combo active: max x%d, tiers %d" % [best, tiers])
		. is_true()
	)
	# Forced wave end: draw with anomalies on.
	ES.anomalies_enabled = true
	var run: RefCounted = ES.run
	run.end_wave_2p(int(st[0]), int(st[1]), st[2])
	run.level_completed()
	var cards: PackedInt32Array = run.pick_three(int(run.wave()) - 1)
	assert_int(cards.size()).override_failure_message("draw of 3 cards").is_equal(3)
	assert_bool(run.add_card(cards[0])).override_failure_message("card added").is_true()
	var base: int = ES.score()
	var fin := int(run.final_score(0))
	assert_int(fin).override_failure_message("final score %d < base %d" % [fin, base]).is_greater_equal(base)
	# Draw screen.
	ES.prompt_mode = "pick"
	ES.prompt_picks = run.pick_three(int(run.wave()) - 1)
	# Save then resume screen.
	ES.save_resume()
	var info: Dictionary = ES.resume_info()
	(
		assert_bool(bool(info.get("two_player", false)) and info.get("deck", []).size() == 1)
		. override_failure_message("two-player save + deck: %s" % [info])
		. is_true()
	)
	var scr: Node = auto_free(load("res://scenes/endless_prompt.tscn").instantiate())
	ES.prompt_mode = "resume"
	get_tree().root.add_child(scr)
	# The fade-in follows timers: wait bounded in real time.
	await _wait_until(func() -> bool: return scr.idle, 5000)
	assert_bool(scr.idle).override_failure_message("resume screen waiting").is_true()
	# Physical Enter, translated by the shared action tyrian_menu_accept (ADR 0120).
	var ev := InputEventKey.new()
	ev.keycode = KEY_ENTER
	ev.physical_keycode = KEY_ENTER
	ev.pressed = true
	scr._unhandled_input(ev)
	await _wait_until(func() -> bool: return scr.result != null, 5000)
	assert_bool(scr.result == true).override_failure_message("ENTER = resume").is_true()
	for i in 30:
		await get_tree().process_frame
	(
		assert_bool(ES.two_player and ES.run != null and ES.run.deck_cards().size() == 1)
		. override_failure_message("game resumed with two players, deck restored")
		. is_true()
	)


## Fake Sim: only `endless_state` is read by `bank_current_wave`.
class FakeSim:
	extends Node

	func endless_state() -> PackedInt64Array:
		return PackedInt64Array([2, 0, 777])


## ENH-01: quitting through the pause menu counts the current wave's score.
func test_quit_counts_current_wave_score() -> void:
	ES.clear_resume()
	Game.rng_seed = 1
	assert_bool(ES.start(false, false)).is_true()
	assert_int(ES.score()).is_equal(0)
	var fake: Node = auto_free(FakeSim.new())
	ES.bank_current_wave(fake)
	assert_int(ES.score()).override_failure_message("current wave score banked").is_equal(777)
	assert_int(ES.lives()).is_equal(2)


## ENH-02: the difficulty ramp is called on wave change.
func test_difficulty_ramps_in_run() -> void:
	Game.rng_seed = 1
	assert_bool(ES.start(false, false)).is_true()
	assert_int(ES.run.difficulty()).is_equal(3)
	ES.run.adjust_difficulty(900000)
	assert_int(ES.run.difficulty()).override_failure_message("difficulty rises with the score").is_greater(3)
	assert_int(ES.run.initial_difficulty()).is_equal(3)


## ENH-04: the resume save carries difficulty, equipment and draw state.
func test_resume_snapshot_keeps_run_state() -> void:
	Game.rng_seed = 1
	assert_bool(ES.start(false, false)).is_true()
	ES.run.adjust_difficulty(900000)
	var d: int = ES.run.difficulty()
	Game.loadout[2] = 9
	ES.save_resume()
	var info: Dictionary = ES.resume_info()
	assert_int(int(info.get("difficulty", -1))).is_equal(d)
	assert_int(int((info.get("loadout", [0, 0, 0]) as Array)[2])).is_equal(9)
	assert_bool(info.has("deck_rng") and info.has("armor")).is_true()
	assert_bool(ES.resume(false)).is_true()
	assert_int(ES.run.difficulty()).override_failure_message("difficulty restored").is_equal(d)
	assert_int(Game.loadout[2]).override_failure_message("weapon power restored").is_equal(9)
