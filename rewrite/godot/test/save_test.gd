# Native campaign save (ADR 0129): writing through the shop's Options
# screen (name entry, confirmation), reading back through the title's
# "Load Game", exact resume; corrupted files or files from another version
# refused without crashing.
extends GdUnitTestSuite

const SLOT := 3

var _dir := ""
var _saved_dir := ""


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	_saved_dir = Game.saves_dir
	Game.saves_dir = "user://save_test"
	_dir = ProjectSettings.globalize_path(Game.saves_dir)
	_clear()


func after() -> void:
	_clear()
	DirAccess.remove_absolute(_dir)
	Game.saves_dir = _saved_dir
	Game.campaign_active = false
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func _clear() -> void:
	if DirAccess.dir_exists_absolute(_dir):
		for f in DirAccess.get_files_at(_dir):
			DirAccess.remove_absolute(_dir.path_join(f))


func _check(cond: bool, what: String) -> void:
	assert_bool(cond).override_failure_message(what).is_true()


## Game in progress at the shop of section 4 of episode 1.
func _campaign_at_shop() -> void:
	Game.campaign_active = true
	Game.episode = 1
	Game.initial_difficulty = 2
	Game.difficulty = 3
	Game.level_name = "TYRIAN"
	Game.debrief = {"cube_max": 2}
	# Talon, Multi-Cannon (front) power 4, Vulcan rear power 2,
	# shield, generator, left sidekick, special, money.
	Game.loadout = PackedInt32Array([1, 3, 4, 7, 2, 4, 3, 1, 0, 1, 15267])
	Game.main_level = 4
	Game.advance_campaign()


func _wait_idle(scene: Node, ms := 15000) -> bool:
	var t0 := Time.get_ticks_msec()
	while is_instance_valid(scene) and scene.get("idle") != true and Time.get_ticks_msec() - t0 < ms:
		await get_tree().process_frame
	return is_instance_valid(scene) and scene.get("idle") == true


func _wait_scene(path: String, ms := 15000) -> Node:
	var t0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t0 < ms:
		var cur: Node = Effects.current_game_scene()
		if cur != null and cur.scene_file_path == path and cur.is_inside_tree():
			return cur
		await get_tree().process_frame
	return null


## Keys sent one by one to the current screen, like the keyboard (ADR 0120).
func _keys(names: String) -> void:
	for k in names.split(",", false):
		var cur: Node = Effects.current_game_scene()
		cur.handle_event(TyrianControls.key_event(OS.find_keycode_from_string(k)))
		await get_tree().process_frame
		await _wait_idle(Effects.current_game_scene(), 5000)


func test_save_from_shop_then_load_from_title() -> void:
	_campaign_at_shop()
	var shop: Node = await _wait_scene(Game.SCENE_SHOP)
	_check(shop != null and Game.shop_section == 4, "section 4 shop (%d)" % Game.shop_section)
	if shop == null:
		return
	var before := {
		"episode": Game.episode,
		"section": Game.shop_section,
		"loadout": Game.loadout.duplicate(),
		"difficulty": Game.difficulty
	}
	# Shop root: Data, Ship Specs, Upgrade Ship, Options.
	Sfx.played.clear()
	await _keys("Down,Down,Down,Enter")
	var menu: Node = await _wait_scene(Game.SCENE_SAVE_MENU)
	_check(menu != null, "Options → save screen")
	if menu == null:
		return
	await _wait_idle(menu)
	# Save Game, slot 3, name "MY RUN", confirmation (Y).
	await _keys("Down,Enter,Down,Down,Enter")
	_check(menu.get("_typing") == true, "name entry open")
	await _keys("M,Y,Space,R,U,N,Backspace,N,Enter")
	# Confirmation box (JE_saveRequest): Yes.
	var t0 := Time.get_ticks_msec()
	while not menu.modal_active and Time.get_ticks_msec() - t0 < 5000:
		await get_tree().process_frame
	_check(menu.modal_active, "confirmation shown")
	await _keys("Y")
	t0 = Time.get_ticks_msec()
	while menu.modal_active and Time.get_ticks_msec() - t0 < 5000:
		await get_tree().process_frame
	# game_menu.c:2813: JE_menuFunction plays S_CLICK on each confirmation (Options,
	# slot); S_SELECT only for Yes in JE_saveRequest (game_menu.c:2642).
	var selects := Sfx.played.count(Sfx.SELECT)
	var clicks := Sfx.played.count(Sfx.CLICK)
	_check(
		selects == 1 and clicks >= 2,
		(
			"sounds: S_SELECT once (Yes), S_CLICK on confirmations (select %d, click %d: %s)"
			% [selects, clicks, Sfx.played]
		)
	)
	var path := _dir.path_join("slot_%02d.json" % SLOT)
	_check(FileAccess.file_exists(path), "file written: %s" % path)
	var json = JSON.parse_string(FileAccess.get_file_as_string(path))
	_check(
		(
			json is Dictionary
			and int(json["format_version"]) == 2
			and json["name"] == "MY RUN"
			and str(json["ship"]).begins_with("USP Talon")
			and json["front_weapon"]["power"] == 4
			and json["rear_weapon"] != null
			and json["difficulty"] == "hard"
			and int(json["section"]) == 4
			and int(json["cash"]) == 15267
		),
		"content by names: %s" % [json]
	)
	_check(not FileAccess.file_exists(path + ".tmp"), "no temporary file left")
	# Back to the shop (Exit, Done): same equipment.
	await _keys("Escape,Escape")
	shop = await _wait_scene(Game.SCENE_SHOP)
	_check(shop != null and Game.loadout == before["loadout"], "Done → shop, equipment kept")
	# Game lost, then the title's "Load Game": slot 3.
	Game.campaign_active = false
	Game.loadout = PackedInt32Array(Game.NEW_GAME_LOADOUT)
	Game.episode = 2
	Game.main_level = 0
	var load: Node = Game.goto_scene("res://scenes/load_screen.tscn")
	_check(await _wait_idle(load), "Load Game screen idle")
	await _keys("Down,Down,Enter")
	shop = await _wait_scene(Game.SCENE_SHOP)
	_check(
		(
			shop != null
			and Game.episode == before["episode"]
			and Game.shop_section == before["section"]
			and Game.loadout == before["loadout"]
			and Game.difficulty == before["difficulty"]
			and Game.campaign_active
		),
		"exact resume: ep. %d section %d %s diff %d" % [Game.episode, Game.shop_section, Game.loadout, Game.difficulty]
	)
	await get_tree().process_frame


## Slot 11 (automatic "LAST LEVEL" save) is written before
## each campaign level and restarts from the shop it was chosen from
## (level_load.c: saveLevel = mainLevel at the start of JE_loadMap).
func test_autosave_before_level() -> void:
	_clear()
	_campaign_at_shop()
	await _wait_scene(Game.SCENE_SHOP)
	Game.campaign_leave_shop(0)
	var slots: Array = Game.campaign_slots()
	var auto = slots[Game.AUTOSAVE_SLOT - 1]
	_check(
		(
			auto != null
			and auto["error"] == ""
			and auto["name"] == TyrianText.L("save_last_level")
			and int(auto["section"]) == 4
		),
		"autosave: %s" % [auto]
	)
	Game.campaign_active = false
	await get_tree().process_frame


## Truncated file, future version, unknown name: slot marked
## unreadable, loading refused, no crash.
func test_corrupt_and_future_files_are_refused() -> void:
	_clear()
	DirAccess.make_dir_recursive_absolute(_dir)
	_campaign_at_shop()
	await _wait_scene(Game.SCENE_SHOP)
	_check(Game.save_campaign(1, "GOOD"), "reference save")
	var good := FileAccess.get_file_as_string(_dir.path_join("slot_01.json"))
	var bad := {
		2: good.left(good.length() / 2),
		4: good.replace('"format_version": 2', '"format_version": 99'),
		5: good.replace('"ship": "', '"ship": "Unknown ').replace('"slot": 1', '"slot": 5'),
		6: "not json at all",
	}
	for slot in bad:
		var f := FileAccess.open(_dir.path_join("slot_%02d.json" % slot), FileAccess.WRITE)
		f.store_string(bad[slot])
		f.close()
	var slots: Array = Game.campaign_slots()
	_check(slots.size() == Game.SAVE_SLOTS, "%d slots (%d)" % [Game.SAVE_SLOTS, slots.size()])
	_check(slots[0] != null and slots[0]["error"] == "", "slot 1 readable")
	for slot in bad:
		var s = slots[slot - 1]
		_check(s != null and str(s["error"]) != "", "slot %d refused: %s" % [slot, s])
	_check(str(slots[3]["error"]).contains("99"), "future version named in the message: %s" % slots[3]["error"])
	# Slots 12..22 (2 players): empty (null).
	for i in range(11, 22):
		_check(slots[i] == null, "slot %d (2 players) empty: %s" % [i + 1, slots[i]])
	# Load Game on an unreadable slot: refused, we stay on the screen.
	Game.campaign_active = false
	var load: Node = Game.goto_scene("res://scenes/load_screen.tscn")
	await _wait_idle(load)
	await _keys("Down,Enter")
	_check(Effects.current_game_scene() == load and not Game.campaign_active, "load refused without a screen change")
	await get_tree().process_frame


## config.c:JE_saveGame/JE_loadGame (two players): 2-player game
## saved in slot 12 (page 2), read back with both players' equipment
## and money (score, score2, lastItems, input1/input2).
func test_two_player_save_and_load() -> void:
	_clear()
	# 2-player game active: modes[0] = 1, modes_extra = [cash1, cash2, diff, lo2].
	Game.campaign_active = true
	Game.episode = 1
	Game.initial_difficulty = 2
	Game.difficulty = 2
	Game.level_name = "TYRIAN"
	Game.debrief = {"cube_max": 0}
	Game.main_level = 4
	# Player 1 equipment (money 20000).
	Game.loadout = PackedInt32Array([1, 3, 4, 7, 2, 4, 3, 1, 0, 1, 20000])
	# modes: two players, devices 1 (keyboard) and 2 (mouse).
	Game.modes = PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2])
	# modes_extra: [cash1, cash2, initial_difficulty, loadout2_without_money].
	# Player 2 equipment: ship 1, front weapon 1 power 3, shield 1, generator 1.
	var lo2 := PackedInt32Array([1, 1, 3, 0, 1, 1, 1, 0, 0, 0])
	Game.modes_extra = [20000, 8500, 2, lo2]
	# Save into slot 12 (first 2-player slot).
	var ok := Game.save_campaign(12, "DUO RUN")
	_check(ok, "2-player save in slot 12")
	var path := _dir.path_join("slot_12.json")
	_check(FileAccess.file_exists(path), "slot_12.json created")
	var json = JSON.parse_string(FileAccess.get_file_as_string(path))
	_check(
		json is Dictionary and int(json.get("players", 0)) == 2,
		"players field == 2 in the JSON: %s" % [json.get("players")]
	)
	_check(json.get("name") == "DUO RUN", "game name: %s" % json.get("name"))
	_check(int(json.get("cash", -1)) == 20000, "p1 money: %s" % json.get("cash"))
	_check(int(json.get("cash2", -1)) == 8500, "p2 money (score2): %s" % json.get("cash2"))
	_check(json.get("ship2") != null, "ship2 present in the JSON")
	_check(json.get("input_devices") != null, "input_devices present")
	# Reload: both players restored.
	var slots: Array = Game.campaign_slots()
	_check(slots.size() == Game.SAVE_SLOTS, "%d slots" % slots.size())
	var s = slots[11]  # slot 12 = index 11
	_check(s != null and s.get("error", "") == "", "slot 12 readable: %s" % s)
	_check(int(s.get("players", 0)) == 2, "players == 2 in the slot: %s" % s.get("players"))
	# Loading: modes restored (two_player = 1, devices = [1, 2]).
	Game.campaign_active = false
	Game.modes = PackedInt32Array()
	Game.modes_extra = []
	Game.load_campaign_slot(s)
	_check(
		Game.campaign_active and Game.modes.size() >= 1 and Game.modes[0] == 1,
		"two players restored: modes[0] = %d" % (Game.modes[0] if Game.modes.size() >= 1 else -1)
	)
	_check(
		Game.modes.size() >= 10 and Game.modes[8] == 1 and Game.modes[9] == 2,
		(
			"devices restored: [%d, %d]"
			% [Game.modes[8] if Game.modes.size() >= 9 else -1, Game.modes[9] if Game.modes.size() >= 10 else -1]
		)
	)
	_check(int(Game.loadout[10]) == 20000, "p1 money restored: %d" % Game.loadout[10])
	_check(
		Game.modes_extra.size() >= 2 and int(Game.modes_extra[1]) == 8500,
		"p2 money restored (modes_extra[1]): %s" % Game.modes_extra[1]
	)
	_check(
		Game.modes_extra.size() >= 4 and Game.modes_extra[3] is PackedInt32Array,
		"p2 equipment restored (modes_extra[3]): %s" % Game.modes_extra[3]
	)
	await get_tree().process_frame


## config.c:JE_saveGame/JE_loadGame: the one-player mode (pItems[2] =
## super_arcade_mode) follows the save. Without it, a reloaded Super Arcade
## game became a full game (whole shop, buyable ship).
func test_one_player_mode_survives_save_and_load() -> void:
	for case in [
		[PackedInt32Array([0, 0, 1, 4, 0, 0, 0, 0, 0, 0]), 4, 10],
		[PackedInt32Array([0, 0, 1, 0, 1, 0, 0, 0, 0, 0]), 254, 13],
		[PackedInt32Array([0, 0, 1, 0, 0, 0, 0, 0, 0, 0]), 255, 10],
		[PackedInt32Array(), 0, 0]
	]:
		_clear()
		Game.campaign_active = true
		Game.episode = 1
		Game.initial_difficulty = 2
		Game.difficulty = 2
		Game.level_name = "TYRIAN"
		Game.debrief = {"cube_max": 0}
		Game.main_level = 4
		Game.loadout = PackedInt32Array([1, 3, 4, 7, 2, 4, 3, 1, 0, 1, 1000])
		Game.campaign_modes = case[0]
		Game.modes = case[0]
		_check(Game.save_campaign(SLOT, "MODE"), "save of mode %d" % case[1])
		var json = JSON.parse_string(FileAccess.get_file_as_string(_dir.path_join("slot_%02d.json" % SLOT)))
		_check(
			int(json.get("super_arcade_mode", 0)) == case[1],
			"mode written: %s (expected %d)" % [json.get("super_arcade_mode"), case[1]]
		)
		var s = Game.campaign_slots()[SLOT - 1]
		Game.campaign_modes = PackedInt32Array()
		Game.modes = PackedInt32Array()
		Game.load_campaign_slot(s)
		_check(
			Game.shop_start_menu() == case[2],
			"shop menu after loading: %d (expected %d)" % [Game.shop_start_menu(), case[2]]
		)
		if case[1] == 4:
			_check(Game.campaign_modes[3] == 4, "Super Arcade ship read back: %d" % Game.campaign_modes[3])
		await get_tree().process_frame
	Game.campaign_active = false


## A game saved before any level is played carries the name "NEW GAME" (key
## save_new_game, mainint.c:JE_initPlayerData), not an empty name.
func test_save_before_any_level_keeps_new_game_name() -> void:
	_clear()
	Game.start_campaign(1, 2)
	var expected := TyrianText.L("save_new_game").left(10)
	_check(expected != "" and Game.level_name == expected, "level name of a new game: '%s'" % Game.level_name)
	_check(Game.save_campaign(SLOT, "BLANK", 1), "save written")
	var slot = Game.campaign_slots()[SLOT - 1]
	_check(slot != null and slot["error"] == "" and slot["last_level"] == expected, "last level read back: %s" % [slot])
	Game.campaign_active = false
	await get_tree().process_frame


## Help at the bottom of the Options menu (helptext.c:menuHelp[MENU_OPTIONS]): Music and Sound
## (mmhelp_14), Joystick, Keyboard and Mouse carry their own text.
func test_options_help_matches_each_row() -> void:
	var menu: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await _wait_idle(menu)
	var helps := []
	for it in menu.menus.current().items:
		helps.append(it.help)
	_check(
		(
			helps
			== ["mmhelp_12", "mmhelp_13", "mmhelp_14", "mmhelp_14", "mmhelp_15", "mmhelp_16", "mmhelp_34", "mmhelp_11"]
		),
		"Options row help: %s" % [helps]
	)
	menu.queue_free()
	await get_tree().process_frame


## Save name typed with accents (French keyboard): mainint.c:2620-2631
## (SDL_TEXTINPUT) keeps only ASCII turned uppercase that has a glyph;
## "é", "É", "ç", "à", "ß" are ignored, without sound. Godot receives the
## character through InputEventKey.unicode (xdotool does not pass it under Xvfb).
func test_save_name_with_accents_keeps_ascii_like_c() -> void:
	_clear()
	_campaign_at_shop()
	var shop: Node = await _wait_scene(Game.SCENE_SHOP)
	if shop == null:
		_check(false, "shop")
		return
	await _keys("Down,Down,Down,Enter")
	var menu: Node = await _wait_scene(Game.SCENE_SAVE_MENU)
	await _wait_idle(menu)
	await _keys("Down,Enter,Down,Down,Enter")
	_check(menu.get("_typing") == true, "name entry open")
	Sfx.played.clear()
	for ch in "éaÉçb àß1!":
		var ev := InputEventKey.new()
		ev.pressed = true
		var code := ch.unicode_at(0)
		ev.keycode = OS.find_keycode_from_string(ch.to_upper()) if code < 128 else KEY_NONE
		ev.unicode = code
		menu.handle_event(ev)
		await get_tree().process_frame
	await _keys("Enter")
	# "éaÉçb àß1!" -> A, B, space, 1, !: 5 characters, 5 S_CURSOR.
	_check(Sfx.played.count(Sfx.CURSOR) == 5, "one sound per kept character: %s" % [Sfx.played])
	var t0 := Time.get_ticks_msec()
	while not menu.modal_active and Time.get_ticks_msec() - t0 < 5000:
		await get_tree().process_frame
	await _keys("Y")
	t0 = Time.get_ticks_msec()
	while menu.modal_active and Time.get_ticks_msec() - t0 < 5000:
		await get_tree().process_frame
	var json = JSON.parse_string(FileAccess.get_file_as_string(_dir.path_join("slot_%02d.json" % SLOT)))
	_check(
		json is Dictionary and json["name"] == "AB 1!",
		"name saved: %s" % [json["name"] if json is Dictionary else json]
	)
	await _keys("Escape,Escape")
	Game.campaign_active = false
	await get_tree().process_frame
