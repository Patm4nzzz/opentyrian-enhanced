# "Games played" counter (stats.c:stats_count_game_start): a campaign game
# increments it only once, at the first level, not at every level
# nor during a demo or a replay; and the level entry transition
# (white flash, fade to black, level_load.c) keeps the simulation stopped.
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")


class FakeSim:
	extends Node
	var starts := 0
	var counted := 0

	func ach_level_start(_path: String, _spectator: bool) -> void:
		starts += 1

	func ach_count_game_start() -> void:
		counted += 1

	func ach_set_context(_language: int, _wave: int, _mods: int) -> void:
		pass

	func ach_clock_ms() -> int:
		return 0


func _level_with(sim: FakeSim) -> Node:
	var level: Node = auto_free(LEVEL.new())
	level.sim = auto_free(sim)
	return level


func test_new_campaign_counts_one_game_over_several_levels() -> void:
	Game.start_campaign(1, 2)
	var sim := FakeSim.new()
	var level := _level_with(sim)
	level._ach_start()
	level._ach_start()
	assert_int(sim.starts).is_equal(2)
	assert_int(sim.counted).is_equal(1)


func test_demo_does_not_count_game() -> void:
	Game.game_start_uncounted = true
	var sim := FakeSim.new()
	var level := _level_with(sim)
	level.demo = true
	level._ach_start()
	assert_int(sim.counted).is_equal(0)


func test_entry_fade_holds_simulation_then_releases() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	# Without a started transition, nothing is retained.
	assert_bool(level._entry_fade_active()).is_false()
	# Fast effects and above: white flash (4 steps) then black (12 steps).
	level._entry_flash_steps = LEVEL.ENTRY_FLASH_STEPS
	level._entry_black_steps = LEVEL.ENTRY_BLACK_STEPS
	level._entry_fade = auto_free(ColorRect.new())
	# Step 1 (the flash lasts 4 steps of 14.4 ms).
	level._entry_fade_start_ms = Time.get_ticks_msec() - 20
	assert_bool(level._entry_fade_active()).is_true()
	assert_float(level._entry_fade.color.r).is_equal(1.0)
	assert_float(level._entry_fade.color.a).is_greater(0.0)
	# 66 steps of 14.4 ms: move the start back by 2 s, the transition is over.
	level._entry_fade_start_ms = Time.get_ticks_msec() - 2000
	assert_bool(level._entry_fade_active()).is_false()


## G-03: in Classic effects (processorType < 2), the C does fade_black(50)
## (level_load.c:971) then the 50-step fade-in (tyrian2.c:438), without a
## white flash: 100 steps, the screen is never white.
func test_classic_effects_fade_to_black_then_fade_in() -> void:
	var saved: int = Game.processor_type
	Game.processor_type = 1
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	add_child(level)
	level._start_entry_fade()
	assert_int(level._entry_flash_steps).is_equal(0)
	assert_int(level._entry_black_steps).is_equal(LEVEL.ENTRY_CLASSIC_BLACK_STEPS)
	assert_object(level._entry_fade).is_not_null()
	# One step = DELAY_PERIOD (the C's setDelay(1), 14.375 ms).
	var step_ms := TyrianScreen.DELAY_PERIOD * 1000.0
	# Step 25: halfway to black, never white.
	level._entry_fade_start_ms = Time.get_ticks_msec() - int(25 * step_ms)
	assert_bool(level._entry_fade_active()).is_true()
	assert_float(level._entry_fade.color.r).is_equal(0.0)
	# Step 85: fade-in still running (steps 50 to 100), the level waits.
	level._entry_fade_start_ms = Time.get_ticks_msec() - int(85 * step_ms)
	assert_bool(level._entry_fade_active()).is_true()
	assert_float(level._entry_fade.color.a).is_between(0.05, 0.5)
	# Step 110: finished.
	level._entry_fade_start_ms = Time.get_ticks_msec() - int(110 * step_ms)
	assert_bool(level._entry_fade_active()).is_false()
	Game.processor_type = saved


## tyrian2.c:438: the fade-in only lights the dashboard frame;
## the play area stays black until the level loop (otherwise it appears
## once fading in, then goes dark again with the opening filter).
func test_entry_fade_in_keeps_playfield_black() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	add_child(level)
	level._start_entry_fade()
	var step_ms := TyrianScreen.DELAY_PERIOD * 1000.0
	var to_black: int = level._entry_flash_steps + level._entry_black_steps
	# During the flash and the descent to black: no cover (the previous screen is visible).
	level._entry_fade_start_ms = Time.get_ticks_msec() - int(2 * step_ms)
	level._entry_last_ms = Time.get_ticks_msec()
	assert_bool(level._entry_fade_active()).is_true()
	assert_bool(level._entry_playfield_mask.visible).is_false()
	# During the fade-in: black play area, only the frame lit.
	level._entry_fade_start_ms = Time.get_ticks_msec() - int((to_black + 25) * step_ms)
	level._entry_last_ms = Time.get_ticks_msec()
	assert_bool(level._entry_fade_active()).is_true()
	(
		assert_bool(level._entry_playfield_mask.visible)
		. override_failure_message("play area lit during the entry fade")
		. is_true()
	)
	assert_vector(level._entry_playfield_mask.size).is_equal(Vector2(HudRenderer.PLAY_W, HudRenderer.PLAY_H))


## A long frame (HD image loading) does not skip the flash: the
## C presents every step of its fade.
func test_entry_fade_long_frame_does_not_skip_steps() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	add_child(level)
	level._start_entry_fade()
	# 600 ms without a pass (a frame frozen by a load).
	level._entry_fade_start_ms -= 600
	level._entry_last_ms -= 600
	assert_bool(level._entry_fade_active()).is_true()
	# At most ENTRY_MAX_GAP_MS counted: we are still in the white flash.
	assert_float(level._entry_fade.color.r).override_failure_message("flash skipped after a long frame").is_equal(1.0)
	assert_float(level._entry_fade.color.a).is_less(1.0)


## The demo keeps the fade-in of tyrian2.c:438 but skips JE_loadMap.
func test_demo_keeps_fade_in_only() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	level.demo = true
	add_child(level)
	level._start_entry_fade()
	assert_int(level._entry_flash_steps + level._entry_black_steps).is_equal(0)
	assert_object(level._entry_fade).is_not_null()
	assert_float(level._entry_fade.color.a).is_equal(1.0)
	level._entry_fade_start_ms = Time.get_ticks_msec() - 1200
	assert_bool(level._entry_fade_active()).is_false()


## tyrian2.c:325-327: leaving the level (abandon or death) goes through
## fade_song() then fade_black(10) before the shop; without this fade the screen
## jumped straight from the level to the shop.
func test_leaving_level_fades_to_black() -> void:
	var level: Node = auto_free(LEVEL.new())
	level.interactive = true
	add_child(level)
	var done := [false]
	var run := func() -> void:
		await level._fade_out_before_leaving()
		done[0] = true
	run.call()
	var veil: ColorRect = level.get_node_or_null("LeaveFade")
	assert_object(veil).override_failure_message("no fade to black when leaving the level").is_not_null()
	assert_bool(done[0]).is_false()
	await get_tree().create_timer(0.4).timeout
	assert_bool(done[0]).override_failure_message("fade not finished after 10 steps").is_true()
	assert_float(veil.color.a).is_equal(1.0)
