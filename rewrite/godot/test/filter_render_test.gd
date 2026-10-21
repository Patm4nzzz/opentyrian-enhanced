# Image filters (backgrnd.c:iced_blur_filter, lava_filter, water_filter, blur_filter):
# the C only applies them once per tick, blending the tick's image with that of the
# previous tick. The Godot rendering recomposes the image at every presentation: it must restart
# from the state before the tick (otherwise the image blends with itself and the grain differs from the
# C: D2 ep. 3-4, ICESECRET / LAVA RUN / ICE EXIT ...). Two renders of the same tick
# must therefore be identical, and differ from a render that blended with itself.
extends GdUnitTestSuite


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


func _palette() -> PackedByteArray:
	var p := PackedByteArray()
	p.resize(768)
	for i in 256:
		p[i * 3] = i
		p[i * 3 + 1] = (i * 7) & 0xff
		p[i * 3 + 2] = 255 - i
	return p


func _icy_level() -> Object:
	var sim: Object = ClassDB.instantiate("TyrianSim")
	sim.set_loadout(PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 30000]))
	sim.set_assist(true)
	# ICESECRET: episode 4, file 20 ("iced blur" filter mid-level).
	sim.load_level(Game.data_dir, 4, 20, 2, 449)
	return sim


func _bytes(img: Image) -> PackedByteArray:
	return img.get_data()


func test_filtered_level_render_is_stable_and_uses_previous_tick() -> void:
	var sim := _icy_level()
	add_child(sim)
	var pal := _palette()
	var filtered_ticks := 0
	var checked := 0
	for t in 700:
		sim.step(0, 0)
		sim.playfield_filters_tick(Game.data_dir)
		if t < 600 or t % 7 != 0:
			continue
		var a: Image = sim.playfield_background(Game.data_dir, pal)
		var b: Image = sim.playfield_background(Game.data_dir, pal)
		assert_object(a).is_not_null()
		checked += 1
		if _bytes(a) != _bytes(b):
			filtered_ticks += 1
	sim.queue_free()
	assert_int(checked).is_greater(5)
	(
		assert_int(filtered_ticks)
		. override_failure_message(
			"the render of the same tick changes at each presentation (filter mixed with itself)"
		)
		. is_equal(0)
	)
