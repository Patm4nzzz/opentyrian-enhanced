# Music Widen: enabling widening does not replace the bus effect.
# Removing then re-adding an effect waits for the audio thread: the main loop
# froze for 0.4 to 16 s on each activation (Setup keys ignored then
# processed in a burst).
extends GdUnitTestSuite


func test_enabling_widen_keeps_the_same_bus_effect() -> void:
	var idx := AudioServer.get_bus_index(Music.BUS_OPL)
	assert_int(idx).is_greater_equal(0)
	if AudioServer.get_bus_effect_count(idx) == 0:
		return  # extension missing: nothing to widen
	var before := AudioServer.get_bus_effect(idx, 0)
	Music.widen = true
	var while_on := AudioServer.get_bus_effect(idx, 0)
	Music.widen = false
	assert_bool(while_on == before).override_failure_message("the bus effect was replaced").is_true()
	assert_bool(AudioServer.is_bus_effect_enabled(idx, 0)).is_false()
