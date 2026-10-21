# Title new game (title_screen.c:newGame, menus.c:gameplaySelect,
# episodeSelect, difficultySelect, timedBattleSelect) and hidden title
# codes (title_screen.c, specialName: Super Arcade, DESTRUCT, ENGAGE), driven
# by real keyboard and gamepad events (Input.parse_input_event).
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const ES := preload("res://scripts/endless_session.gd")
const TITLE := preload("res://scripts/title.gd")
const NG := preload("res://scripts/new_game.gd")


func before() -> void:
	ES.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false
	C.install()
	C.reset_defaults()


func after() -> void:
	C.reset_defaults()
	ES.stop()
	Game.campaign_active = false
	Game.campaign_modes = PackedInt32Array()
	Game.campaign_modes_extra = []
	Game.time_battle_selection = 0
	Game.modes = PackedInt32Array()
	Game.modes_extra = []
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	ES.stop()
	Game.campaign_active = false
	await get_tree().process_frame
	await get_tree().process_frame


func _wait_until(cond: Callable, ms: int) -> void:
	var end := Time.get_ticks_msec() + ms
	while not cond.call() and Time.get_ticks_msec() < end:
		await get_tree().process_frame


func _send(ev: InputEvent) -> void:
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEvent = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()


func _press(code: int, shift := false, unicode := 0) -> void:
	var ev := C.key_event(code)
	ev.shift_pressed = shift
	ev.unicode = unicode
	_send(ev)


## Typed letter (C text event: last_text).
func _type(text: String) -> void:
	for ch in text:
		_press(OS.find_keycode_from_string(ch), false, ch.to_lower().unicode_at(0))
		await get_tree().process_frame


func _pad(button: int) -> void:
	var ev := InputEventJoypadButton.new()
	ev.button_index = button
	ev.pressed = true
	_send(ev)


func _scene() -> Node:
	return Effects.current_game_scene()


## Title at rest, logo already at the top (without intro_logos).
func _title() -> Node:
	TITLE.intro_shown = true
	TITLE.move_tyrian_logo_up = false
	TITLE.selected = 0
	var title: Node = Game.goto_scene(Game.SCENE_TITLE)
	await _wait_until(func() -> bool: return title.idle, 15000)
	assert_bool(title.idle).override_failure_message("title never idle").is_true()
	return title


## Screen `id` of new_game.gd displayed and at rest.
func _screen(id: String) -> Node:
	await _wait_until(
		func() -> bool:
			var s := _scene()
			return (
				s != null
				and s.scene_file_path == Game.SCENE_NEW_GAME
				and s.current_screen == id
				and s.idle
				and not s._busy
			),
		15000
	)
	var s := _scene()
	assert_str(s.scene_file_path).is_equal(Game.SCENE_NEW_GAME)
	assert_str(s.current_screen).override_failure_message("expected screen: %s" % id).is_equal(id)
	return s


## Title -> New Game -> 1 Player Full Game -> Episode 3 -> Hard: the
## campaign starts at episode 3, difficulty Hard (3), starting money of
## episode 3 (initial_cash[] = 20000).
func test_new_game_episode_3_hard() -> void:
	await _title()
	_press(KEY_ENTER)
	var ng := await _screen("gameplay")
	# Gamepad: A confirms "1 Player Full Game".
	_pad(JOY_BUTTON_A)
	ng = await _screen("episode")
	_press(KEY_DOWN)
	_press(KEY_DOWN)
	assert_int(ng.menus.selection()).is_equal(2)
	_press(KEY_ENTER)
	ng = await _screen("difficulty")
	# difficultySelect: Normal preselected, three visible rows.
	assert_int(ng.menus.selection()).is_equal(1)
	assert_int(ng.menus.current().items.size()).is_equal(3)
	_press(KEY_DOWN)
	_press(KEY_ENTER)
	await _wait_until(
		func() -> bool: return Game.campaign_active and _scene().scene_file_path != Game.SCENE_NEW_GAME, 10000
	)
	assert_bool(Game.campaign_active).is_true()
	assert_int(Game.episode).is_equal(3)
	assert_int(Game.initial_difficulty).is_equal(3)
	assert_int(Game.difficulty).is_equal(3)
	assert_int(Game.loadout[10]).is_equal(20000)
	assert_int(Game.loadout[0]).is_equal(1)
	assert_array(Array(Game.campaign_modes)).is_empty()


## Esc goes back from a screen like the C (episodeThenDifficultySelect):
## difficulty -> episode -> title; in the mode choice, Esc -> title.
func test_escape_goes_back_one_screen() -> void:
	await _title()
	_press(KEY_ENTER)
	await _screen("gameplay")
	_press(KEY_ENTER)
	await _screen("episode")
	_press(KEY_ENTER)
	await _screen("difficulty")
	_press(KEY_ESCAPE)
	await _screen("episode")
	_pad(JOY_BUTTON_B)  # B = Esc
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_TITLE, 10000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)
	assert_bool(Game.campaign_active).is_false()
	# Mode choice: Esc also goes back to the title.
	var title: Node = _scene()
	await _wait_until(func() -> bool: return title.idle, 15000)
	_press(KEY_ENTER)
	await _screen("gameplay")
	_press(KEY_ESCAPE)
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_TITLE, 10000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)


## Timed Battle: battle 1 (episode 1, ]T[ -> section 43, DELI); the
## difficulty refused with Esc goes back to the battle choice.
func test_timed_battle() -> void:
	await _title()
	_press(KEY_ENTER)
	var ng := await _screen("gameplay")
	for i in 3:
		_press(KEY_DOWN)
	assert_int(ng.menus.selection()).is_equal(NG.MODE_TIMED)
	_press(KEY_ENTER)
	ng = await _screen("timed")
	assert_int(ng.menus.current().items.size()).is_equal(3)
	_press(KEY_ENTER)
	ng = await _screen("difficulty")
	_press(KEY_ESCAPE)
	ng = await _screen("timed")
	_press(KEY_ENTER)
	await _screen("difficulty")
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_LEVEL, 10000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_LEVEL)
	assert_int(Game.time_battle_selection).is_equal(1)
	assert_str(Game.level_name).is_equal("DELI")
	assert_int(Game.modes[2]).is_equal(1)  # onePlayerAction
	assert_int(Game.modes[5]).is_equal(1)  # timed
	assert_int(Game.loadout[0]).is_equal(8)  # Stalker
	assert_int(Game.loadout[10]).is_equal(0)


## 1 Player Arcade: Stalker, money at 0, MENU_1_PLAYER_ARCADE shop;
## "Start Level" starts the level in arcade mode (onePlayerAction).
func test_arcade_mode_shop_and_level() -> void:
	await _title()
	_press(KEY_ENTER)
	await _screen("gameplay")
	_press(KEY_DOWN)
	_press(KEY_ENTER)
	await _screen("episode")
	_press(KEY_ENTER)
	await _screen("difficulty")
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_SHOP and _scene().idle, 10000)
	var shop := _scene()
	assert_str(shop.scene_file_path).is_equal(Game.SCENE_SHOP)
	assert_int(shop._arcade_menu).is_equal(10)
	assert_int(Game.loadout[0]).is_equal(8)
	assert_int(Game.loadout[10]).is_equal(0)
	_press(KEY_ENTER)  # Start Level
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_LEVEL, 10000)
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_LEVEL)
	assert_str(Game.level_name).is_equal("TYRIAN")
	assert_int(Game.modes[2]).is_equal(1)


## 2 Player Arcade: Silver Ship (11), difficultyLevel + 1, player 2 mouse,
## MENU_2_PLAYER_ARCADE shop.
func test_two_player_arcade() -> void:
	await _title()
	_press(KEY_ENTER)
	await _screen("gameplay")
	for i in 4:
		_press(KEY_DOWN)
	_press(KEY_ENTER)
	await _screen("episode")
	_press(KEY_ENTER)
	await _screen("difficulty")
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return _scene().scene_file_path == Game.SCENE_SHOP and _scene().idle, 10000)
	assert_int(_scene()._arcade_menu).is_equal(9)
	assert_int(Game.loadout[0]).is_equal(11)
	assert_int(Game.initial_difficulty).is_equal(2)
	assert_int(Game.difficulty).is_equal(3)
	assert_int(Game.campaign_modes[0]).is_equal(1)


## Network entry: grayed out, refused (S_SPRING), the screen stays displayed.
func test_network_entry_is_refused() -> void:
	await _title()
	_press(KEY_ENTER)
	var ng := await _screen("gameplay")
	_press(KEY_UP)
	assert_int(ng.menus.selection()).is_equal(NG.MODE_NETWORK)
	_press(KEY_ENTER)
	await get_tree().process_frame
	await _wait_until(func() -> bool: return ng.idle, 2000)
	assert_object(_scene()).is_same(ng)
	assert_str(ng.current_screen).is_equal("gameplay")
	assert_bool(Game.campaign_active).is_false()


## menus.c:542-582: Shift+G shows Impossible, Shift+] Suicide, then L O R D
## typed in order "Lord of Game" (DIFFICULTY_ZINGLON = 8).
func test_lord_of_game_unlock() -> void:
	await _title()
	_press(KEY_ENTER)
	await _screen("gameplay")
	_press(KEY_ENTER)
	await _screen("episode")
	_press(KEY_ENTER)
	var ng := await _screen("difficulty")
	# "LORD" has no effect as long as Suicide is not visible.
	for k in [KEY_L, KEY_O, KEY_R, KEY_D]:
		_press(k)
	assert_int(ng.menus.current().items.size()).is_equal(3)
	_press(KEY_BRACKETRIGHT, true)
	assert_int(ng.menus.current().items.size()).override_failure_message("Shift+] before Shift+G").is_equal(3)
	_press(KEY_G, true)
	assert_int(ng.menus.current().items.size()).is_equal(4)
	_press(KEY_BRACKETRIGHT, true)
	assert_int(ng.menus.current().items.size()).is_equal(5)
	# Sequence interrupted: starts again at L.
	_press(KEY_L)
	_press(KEY_X)
	_press(KEY_O)
	assert_int(ng.menus.current().items.size()).is_equal(5)
	for k in [KEY_L, KEY_O, KEY_R, KEY_D]:
		_press(k)
	assert_int(ng.menus.current().items.size()).is_equal(6)
	_press(KEY_UP)
	_press(KEY_UP)  # Normal -> Easy -> Lord of Game (wraparound)
	assert_int(ng.menus.selection()).is_equal(5)
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return Game.campaign_active, 10000)
	assert_int(Game.initial_difficulty).is_equal(8)


## STEALTH code typed at the title: episode, difficulty, ship screen then
## Super Arcade game (Ninja Star = SAShip[0] = 3, difficulty + 1).
func test_title_code_super_arcade() -> void:
	await _title()
	await _type("STEALTH")
	await _screen("episode")
	assert_int(NG.flow).is_equal(NG.Flow.SUPER_ARCADE)
	assert_int(NG.super_arcade).is_equal(0)
	_press(KEY_ENTER)
	await _screen("difficulty")
	_press(KEY_ENTER)
	await _wait_until(
		func() -> bool:
			var s := _scene()
			return s.scene_file_path == Game.SCENE_NEW_GAME and s.current_screen == "super_arcade" and s.idle,
		10000
	)
	_press(KEY_SPACE)
	await _wait_until(func() -> bool: return Game.campaign_active, 10000)
	assert_int(Game.loadout[0]).is_equal(3)
	assert_int(Game.loadout[1]).is_equal(9)
	assert_int(Game.loadout[9]).is_equal(7)
	assert_int(Game.initial_difficulty).is_equal(3)
	assert_int(Game.campaign_modes[3]).is_equal(1)


## Overlapping codes: "ttechno" triggers TECHNO (S10), a wrong
## letter resets the progress to zero.
func test_title_code_progress() -> void:
	var title: Node = await _title()
	await _type("STEAX")
	assert_str(_scene().scene_file_path).is_equal(Game.SCENE_TITLE)
	assert_int(title._code_progress[0]).is_equal(0)
	await _type("TTECHNO")
	await _screen("episode")
	assert_int(NG.super_arcade).is_equal(2)


## DESTRUCT: the mini-game if shipped (destruct.tscn scene), otherwise the title resumes.
func test_title_code_destruct() -> void:
	await _title()
	await _type("DESTRUCT")
	if ResourceLoader.exists(TITLE.SCENE_DESTRUCT):
		await _wait_until(func() -> bool: return _scene().scene_file_path == TITLE.SCENE_DESTRUCT, 5000)
		assert_str(_scene().scene_file_path).is_equal(TITLE.SCENE_DESTRUCT)
		return
	var title: Node = _scene()
	await _wait_until(func() -> bool: return title.idle, 15000)
	assert_str(title.scene_file_path).is_equal(Game.SCENE_TITLE)
	assert_bool(title.idle).is_true()


## ENGAGE: JE_whoa, Super Tyrian screen, episode choice, Stalker 21.126
## (13) and Atomic RailGun (39), difficulty Lord of Game (8).
func test_title_code_engage() -> void:
	await _title()
	await _type("ENGAGE")
	await _wait_until(
		func() -> bool:
			var s := _scene()
			return s.scene_file_path == Game.SCENE_NEW_GAME and s.current_screen == "super_tyrian" and s.idle,
		15000
	)
	assert_str(_scene().current_screen).is_equal("super_tyrian")
	_press(KEY_SPACE)
	await _screen("episode")
	_press(KEY_ENTER)
	await _wait_until(func() -> bool: return Game.campaign_active, 10000)
	assert_int(Game.loadout[0]).is_equal(13)
	assert_int(Game.loadout[1]).is_equal(39)
	assert_int(Game.initial_difficulty).is_equal(8)
	assert_int(Game.campaign_modes[4]).is_equal(1)
