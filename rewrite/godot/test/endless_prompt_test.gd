## Endless anomaly draw screen (endless_prompt.gd): the mouse must
## target the card under the pointer. The C (anomalies_ui.c) only reads the
## keyboard; the mouse is a rewrite addition. Hover moves the selection, the click confirms the card
## clicked, and the keyboard path does not change.
extends GdUnitTestSuite

const ES := preload("res://scripts/endless_session.gd")
const EndlessPromptScene := preload("res://scenes/endless_prompt.tscn")
## Geometry of anomalies_ui.c (endless_prompt.gd).
const CARD_W := 96
const CARD_GAP := 6
const CARDS_Y0 := 38
const CARD_H := 128


func before() -> void:
	TyrianControls.install()


func after() -> void:
	ES.clear_resume()
	ES.stop()
	ES.two_player = false
	# Static flag set by the draw tests: the first draw before wave 1
	# (ENH-08) must not spill over onto the following suites.
	ES.anomalies_enabled = false
	Game.modes = PackedInt32Array()
	Game.mutators = 0
	Game.anomalies = 0
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func _clear_viewport() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
	await get_tree().process_frame


## Waits for a boolean state carried by the node; only uses the node while it
## exists (the screen loop may launch another right after).
func _wait_idle(scr: Node, ms: int) -> void:
	var deadline := Time.get_ticks_msec() + ms
	while is_instance_valid(scr) and not scr.idle and Time.get_ticks_msec() < deadline:
		await get_tree().process_frame


func _wait_result(scr: Node, ms: int) -> void:
	var deadline := Time.get_ticks_msec() + ms
	while is_instance_valid(scr) and scr.result == null and Time.get_ticks_msec() < deadline:
		await get_tree().process_frame


## Opens the draw screen with three cards drawn from a new game.
func _open_pick() -> Array:
	await _clear_viewport()
	ES.clear_resume()
	Game.rng_seed = 7
	ES.start(false, false)
	ES.anomalies_enabled = true
	var cards: PackedInt32Array = ES.run.pick_three(int(ES.run.wave()) - 1)
	ES.prompt_mode = "pick"
	ES.prompt_picks = cards
	var scr: Node = auto_free(EndlessPromptScene.instantiate())
	get_tree().root.add_child(scr)
	await _wait_idle(scr, 5000)
	return [scr, cards]


## Opens the resume screen with a saved game.
func _open_resume() -> Node:
	await _clear_viewport()
	ES.clear_resume()
	Game.rng_seed = 7
	ES.start(false, false)
	ES.save_resume()
	ES.prompt_mode = "resume"
	var scr: Node = auto_free(EndlessPromptScene.instantiate())
	get_tree().root.add_child(scr)
	await _wait_idle(scr, 5000)
	return scr


static func _motion(x: int, y: int) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 0)
	return ev


static func _left_press(x: int, y: int) -> InputEventMouseButton:
	var ev := InputEventMouseButton.new()
	ev.position = Vector2(x, y)
	ev.button_index = MOUSE_BUTTON_LEFT
	ev.pressed = true
	return ev


static func _key(code: Key) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.keycode = code
	ev.physical_keycode = code
	ev.pressed = true
	return ev


## Center of card `i` for `n` offered cards (same centering as pick).
static func _card_center(n: int, i: int) -> Vector2i:
	var row_w := n * CARD_W + (n - 1) * CARD_GAP
	var x0 := (320 - row_w) / 2
	return Vector2i(x0 + i * (CARD_W + CARD_GAP) + CARD_W / 2, CARDS_Y0 + CARD_H / 2)


## Hover highlights the card under the pointer, on each one.
func test_hover_selects_the_card_under_the_pointer() -> void:
	var setup: Array = await _open_pick()
	var scr: Node = setup[0]
	var cards: PackedInt32Array = setup[1]
	assert_int(cards.size()).override_failure_message("draw of three cards").is_equal(3)
	TyrianMouse.inactive = false
	for i in cards.size():
		var p := _card_center(cards.size(), i)
		scr.handle_mouse(_motion(p.x, p.y))
		await get_tree().process_frame
		assert_int(scr._pick_cursor).override_failure_message("hover over card %d" % i).is_equal(i)


## A left click confirms the clicked card, not the card highlighted by default.
func test_click_confirms_the_clicked_card() -> void:
	for i in [1, 2]:
		var setup: Array = await _open_pick()
		var scr: Node = setup[0]
		var cards: PackedInt32Array = setup[1]
		assert_int(cards.size()).is_equal(3)
		TyrianMouse.inactive = false
		var p := _card_center(cards.size(), i)
		scr.handle_mouse(_left_press(p.x, p.y))
		await _wait_result(scr, 5000)
		var ok := is_instance_valid(scr) and scr.result != null
		assert_bool(ok).override_failure_message("click on card %d: no result" % i).is_true()
		if ok:
			assert_int(scr.result).override_failure_message("click on card %d" % i).is_equal(int(cards[i]))


## The keyboard path (Right Right Enter) stays unchanged.
func test_keyboard_path_unchanged() -> void:
	var setup: Array = await _open_pick()
	var scr: Node = setup[0]
	var cards: PackedInt32Array = setup[1]
	assert_int(cards.size()).is_equal(3)
	for code in [KEY_RIGHT, KEY_RIGHT, KEY_ENTER]:
		scr._unhandled_input(_key(code))
		await get_tree().process_frame
	await _wait_result(scr, 5000)
	var ok := is_instance_valid(scr) and scr.result != null
	assert_bool(ok).override_failure_message("keyboard: no result").is_true()
	if ok:
		assert_int(scr.result).override_failure_message("keyboard: Right Right Enter").is_equal(int(cards[2]))


## Resume screen: Enter resumes, Esc starts a new game (C).
func test_resume_screen_enter_and_escape() -> void:
	for pair in [[KEY_ENTER, true], [KEY_ESCAPE, false]]:
		var scr: Node = await _open_resume()
		scr._unhandled_input(_key(pair[0]))
		await _wait_result(scr, 5000)
		var ok := is_instance_valid(scr) and scr.result != null
		assert_bool(ok).override_failure_message("resume: no result").is_true()
		if ok:
			assert_bool(scr.result == pair[1]).override_failure_message("resume: key %d" % pair[0]).is_true()
		await get_tree().process_frame
