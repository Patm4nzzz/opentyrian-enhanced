# Pause menu > Visual Effects (mainint.c:vfx_row_act): the preset is
# applied right away by JE_initProcessorType, hence to the running level
# (layer 2 of water / ice, filter, explosion cascade). Godot only
# changed processorType when loading the next level: switching to Off
# in the pause changed nothing in the level being played.
extends GdUnitTestSuite

const LEVEL := preload("res://scripts/level.gd")


class FakeSim:
	extends Node
	var pt := 4
	var set_calls: Array[int] = []

	func set_effects(_a, _b, _c, _d, _e) -> void:
		pass

	func set_screen_fx(_shake: bool, _pt: int) -> void:
		pass

	func processor_type() -> int:
		return pt

	func set_processor_type(v: int) -> void:
		set_calls.append(v)
		pt = v


func test_visual_fx_change_reaches_the_running_level() -> void:
	var saved: int = Game.processor_type
	var level: Node = auto_free(LEVEL.new())
	var sim: FakeSim = auto_free(FakeSim.new())
	level.sim = sim
	Game.processor_type = 1
	level.apply_effects()
	assert_array(sim.set_calls).is_equal([1])
	# Same value: nothing is redone (the stars are not removed).
	level.apply_effects()
	assert_array(sim.set_calls).is_equal([1])
	Game.processor_type = saved


func test_replay_keeps_its_processor_type() -> void:
	var saved: int = Game.processor_type
	var level: Node = auto_free(LEVEL.new())
	var sim: FakeSim = auto_free(FakeSim.new())
	level.sim = sim
	level.replaying = true
	Game.processor_type = 1
	level.apply_effects()
	assert_array(sim.set_calls).is_empty()
	Game.processor_type = saved
