# Level end and achievements (tyrian2.c:JE_main): EVENT_LEVEL_COMPLETE, hence the
# "Levels Completed" statistic and the level-end achievements (Survivor,
# Phoenix, Speed Demon, True Pacifist, Zen Master...), is only emitted if a
# player is still alive or if the level is a bonus level. A game over
# also ends the level, but must not unlock anything.
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")


## Simulation reduced to the calls of level.gd:_ach_tick.
class FakeSim:
	extends Node
	var alive := [true, false]
	var two_player := false
	var completions := 0

	func ach_after_tick() -> PackedInt32Array:
		return PackedInt32Array()

	func level_ended() -> bool:
		return true

	var complete_ids := PackedInt32Array()

	func ach_level_complete() -> PackedInt32Array:
		completions += 1
		return complete_ids

	func ach_save(_path: String) -> void:
		pass

	func player(i: int) -> PackedInt32Array:
		return PackedInt32Array([100, 180, 0, 0, 1 if alive[i] else 0, 0, 0, 0])

	func mode_hud() -> Dictionary:
		return {"two_player": two_player}

	func ach_clock_ms() -> int:
		return 0


var _saved_campaign := false
var _saved_bonus := false


func before_test() -> void:
	_saved_campaign = Game.campaign_active
	_saved_bonus = Game.bonus_level_current
	Game.campaign_active = true
	Game.bonus_level_current = false


func after_test() -> void:
	Game.campaign_active = _saved_campaign
	Game.bonus_level_current = _saved_bonus


func _level_with(sim: FakeSim) -> Node:
	var level: Node = auto_free(LEVEL.new())
	level.sim = auto_free(sim)
	level._ach_active = true
	return level


func test_game_over_does_not_complete_level() -> void:
	var sim := FakeSim.new()
	sim.alive = [false, false]
	var level := _level_with(sim)
	level._ach_tick()
	level._ach_tick()
	assert_int(sim.completions).override_failure_message("game over counted as completed level").is_equal(0)


func test_survived_level_completes_once() -> void:
	var sim := FakeSim.new()
	var level := _level_with(sim)
	level._ach_tick()
	level._ach_tick()
	assert_int(sim.completions).is_equal(1)


func test_second_player_alive_completes_level() -> void:
	var sim := FakeSim.new()
	sim.alive = [false, true]
	sim.two_player = true
	var level := _level_with(sim)
	level._ach_tick()
	assert_int(sim.completions).is_equal(1)


func test_bonus_level_completes_even_when_dead() -> void:
	Game.bonus_level_current = true
	var sim := FakeSim.new()
	sim.alive = [false, false]
	var level := _level_with(sim)
	level._ach_tick()
	assert_int(sim.completions).is_equal(1)


## mainint.c:JE_endLevelAni emits EVENT_LEVEL_COMPLETE after the level loop:
## a level-end achievement banner (Survivor) is never drawn onto the
## debriefing's frozen image (enemy_render.c:201 only runs in the loop).
func test_level_complete_achievement_banner_not_on_frozen_frame() -> void:
	var sim := FakeSim.new()
	sim.complete_ids = PackedInt32Array([0])
	var level := _level_with(sim)
	level._ach_tick()
	assert_int(sim.completions).is_equal(1)
	(
		assert_str(level._hud_banner_text())
		. override_failure_message("end-of-level banner on the debriefing image")
		. is_empty()
	)
