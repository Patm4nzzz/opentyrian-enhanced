# Endless, next wave without anomaly draw: tyrian2.c:222-224 does
# fade_song() then fade_black(10) on the last frame of the level before
# loading the next wave.
extends GdUnitTestSuite

const ES := preload("res://scripts/endless_session.gd")

var _order: Array = []
var _saved_run: RefCounted
var _saved_anomalies: bool


class FakeRun:
	extends RefCounted
	var order: Array

	func level_completed() -> void:
		order.append("completed")

	func wave() -> int:
		return 2

	func pick_three(_wave: int) -> PackedInt32Array:
		return PackedInt32Array()

	func next_level() -> bool:
		order.append("next_level")
		return false


class FakeLevel:
	extends Node
	var order: Array

	func _fade_out_before_leaving() -> void:
		order.append("fade")
		await get_tree().process_frame


func before_test() -> void:
	_saved_run = ES.run
	_saved_anomalies = ES.anomalies_enabled
	_order = []


func after_test() -> void:
	ES.run = _saved_run
	ES.anomalies_enabled = _saved_anomalies
	for child in Effects.game_viewport.get_children():
		Effects.game_viewport.remove_child(child)
		child.free()
	await get_tree().process_frame


func test_next_wave_fades_level_before_loading() -> void:
	var run := FakeRun.new()
	run.order = _order
	ES.run = run
	ES.anomalies_enabled = false
	var level: FakeLevel = auto_free(FakeLevel.new())
	level.order = _order
	add_child(level)
	await ES.after_debrief(level)
	assert_array(_order).override_failure_message("order %s" % [_order]).is_equal(["completed", "fade", "next_level"])
