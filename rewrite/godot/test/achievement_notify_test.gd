# In-game achievement notification (achievements.c:achievements_unlock then
# achievements_draw_notification): on unlock, "Achievement: NAME" is
# shown for 3 s of play in the play area; a
# second achievement unlocked in the meantime waits its turn (queue of 8).
# Verified here in real play: interactive level, fire held on the keyboard until the
# first enemy is destroyed ("First Blood").
# No unlock sound in a level (divergence 63): the C's S_SOUL_OF_ZINGLON, a weapon
# sweep on the front gun's channel 0, was cut by the next shot and heard as a
# truncated weapon sound.
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()
	# Fresh achievements: the test folder's save may carry those
	# unlocked by a previous suite.
	DirAccess.remove_absolute(ProjectSettings.globalize_path("user://opentyrian.dat"))
	# ach_api.rs:TRACKER (thread-local): persists between suites in the same
	# Godot process. Reset it so that First Blood is not already
	# unlocked by a previous suite (overlay_test, pause_test).
	if ClassDB.class_exists("TyrianSim"):
		var tracker_sim: Node = ClassDB.instantiate("TyrianSim")
		tracker_sim.ach_tracker_reset()
		tracker_sim.free()


func after() -> void:
	Input.action_release("tyrian_fire")
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame


func _hold(code: int, pressed: bool) -> void:
	var ev := C.key_event(code)
	ev.pressed = pressed
	Input.parse_input_event(ev)
	Input.flush_buffered_events()


func test_first_blood_banner_shown_then_cleared() -> void:
	# Night Player (0:00 - 4:59) unlocks based on local time: fixed noon for the test.
	Game.local_hour_override = 12
	Game.episode = 1
	Game.lvl_file_num = 9
	Game.difficulty = 2
	Game.rng_seed = 1
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 10000])
	var sounds_before: int = Sfx.played.size()
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	assert_bool(level.sim != null and level._ach_active).override_failure_message("achievements off in game").is_true()
	_hold(KEY_SPACE, true)
	var first_blood := -1
	for i in level.ACHIEVEMENT_LIST.size():
		if level.ACHIEVEMENT_LIST[i][0] == "ach_first_blood":
			first_blood = i
	var end := Time.get_ticks_msec() + 30000
	while level._ach_notify_id < 0 and Time.get_ticks_msec() < end:
		await get_tree().physics_frame
	_hold(KEY_SPACE, false)
	# First unlock of the level: "First Blood" (one enemy destroyed).
	assert_int(level._ach_notify_id).is_equal(first_blood)
	# Silent unlock: no S_SOUL_OF_ZINGLON (21) among the out-of-queue sounds.
	(
		assert_array(Sfx.played.slice(sounds_before))
		. override_failure_message("achievement sound played in a level")
		. not_contains([21])
	)
	var expected := TyrianText.L("ach_notify").replace("%s", TyrianText.L("ach_first_blood"))
	var shown: String = level._hud_vga.notification if level._hud_vga != null else ""
	var in_playfield: bool = (
		level._playfield_bg != null and ClassDB.class_has_method("TyrianSim", "set_playfield_banner")
	)
	(
		assert_bool(shown == expected or in_playfield)
		. override_failure_message("banner '%s' not shown ('%s')" % [expected, shown])
		. is_true()
	)
	# Drawn into the HUD's witness surfaces (JE_textShade, color 15):
	# lit pixels in the banner's strip.
	if not in_playfield:
		await get_tree().physics_frame
		var img: Image = level._hud_vga.vga.image(level._hud_vga.OVER_A)
		var lit := 0
		for y in range(3, 12):
			for x in range(24, 288):
				if img.get_pixel(x, y).v > 0.5:
					lit += 1
		assert_int(lit).override_failure_message("banner missing from the HUD image").is_greater(20)
	# Disappears after ACH_NOTIFY_DURATION_MS (3 s of play, simulation clock).
	var t0: int = level.sim.ach_clock_ms()
	end = Time.get_ticks_msec() + 30000
	while level.sim.ach_clock_ms() - t0 < level.ACH_NOTIFY_MS + 200 and Time.get_ticks_msec() < end:
		await get_tree().physics_frame
	if level._ach_queue.is_empty():
		assert_int(level._ach_notify_id).is_equal(-1)
	else:
		assert_int(level._ach_notify_id).is_not_equal(first_blood)
