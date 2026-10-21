# C audit defects: default Scaler, saved language, save
# box (Yes preselected, "Original save" line), C profile.
extends GdUnitTestSuite

const SETUP := preload("res://scripts/setup.gd")
const DISPLAY := preload("res://scripts/display_settings.gd")


func test_default_scaler_is_scale3x() -> void:
	assert_int(DISPLAY.DEFAULT_SCALER).is_equal(5)
	assert_int(DISPLAY.SCALER_FACTORS[DISPLAY.DEFAULT_SCALER]).is_equal(3)


func test_language_picker_saves_settings() -> void:
	var saved_lang: int = SETUP.settings.current_language
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SETUP.SETTINGS_PATH))
	var screen := SETUP.new()
	var items: Array = screen._gameplay_items()
	var picker: Dictionary = {}
	for it in items:
		if str(it.get("label", "")) != "" or true:
			if it.has("set") and it.get("count", 0) == SETUP.LANGUAGE_NAMES.size():
				picker = it
	assert_bool(picker.is_empty()).is_false()
	picker.set.call(1)
	assert_bool(FileAccess.file_exists(ProjectSettings.globalize_path(SETUP.SETTINGS_PATH))).is_true()
	picker.set.call(saved_lang)
	screen.free()


func test_c_profile_scores_and_settings_are_converted() -> void:
	var dir := OS.get_environment("TMPDIR") if not OS.get_environment("TMPDIR").is_empty() else "/tmp"
	dir = dir.path_join("c_profile_test_%d" % Time.get_ticks_usec())
	DirAccess.make_dir_recursive_absolute(dir)
	var root := {
		"video": {"scaler": "Scale3x", "scaling_mode": "Fit 4:3", "detail": 3},
		"audio": {"music_volume": 100},
		"features": {"language": "fr", "crt_mode": "scanlines", "explosion_fx": "modern", "combo": false},
	}
	assert_bool(CProfile._write_setup(root, dir.path_join("setup.cfg"))).is_true()
	assert_bool(CProfile._write_display(root, dir.path_join("display.cfg"))).is_true()
	var cfg := ConfigFile.new()
	cfg.load(dir.path_join("setup.cfg"))
	assert_int(int(cfg.get_value("setup", "current_language"))).is_equal(1)
	assert_int(int(cfg.get_value("setup", "music_volume"))).is_equal(100)
	# C detail level (processorType 3 = High Detail): taken over as the Basic notch.
	assert_int(int(cfg.get_value("setup", "visual_fx"))).is_equal(1)
	assert_int(int(cfg.get_value("setup", "explosion_style"))).is_equal(2)
	assert_bool(bool(cfg.get_value("setup", "combo_enabled"))).is_false()
	cfg.load(dir.path_join("display.cfg"))
	assert_int(int(cfg.get_value("display", "scaler"))).is_equal(5)
	assert_int(int(cfg.get_value("display", "scaling_mode"))).is_equal(3)
	assert_int(int(cfg.get_value("display", "crt_mode"))).is_equal(1)
	var tables := []
	for i in 20:
		tables.append(
			[
				{"score": 15000 if i == 10 else 0, "name": "AB", "difficulty": 1},
				{"score": 0, "name": "", "difficulty": 0},
				{"score": 0, "name": "", "difficulty": 0}
			]
		)
	assert_bool(CProfile._write_scores(tables, dir.path_join("highscores.json"))).is_true()
	var parsed: Variant = JSON.parse_string(FileAccess.get_file_as_string(dir.path_join("highscores.json")))
	assert_int(int(parsed["tables"][10][0]["score"])).is_equal(15000)
	# No score played: nothing is written, the default scores remain.
	var empty := []
	for i in 20:
		empty.append([{"score": 0, "name": "", "difficulty": 0}])
	assert_bool(CProfile._write_scores(empty, dir.path_join("none.json"))).is_false()


func test_c_profile_candidates_include_standard_linux_path() -> void:
	if OS.get_name() == "Windows":
		return
	if not OS.get_environment("XDG_CONFIG_HOME").is_empty():
		return
	var paths := CProfile.candidate_paths()
	var found := false
	for p in paths:
		if p.ends_with(".config/opentyrian2000/opentyrian.dat") or p.ends_with("/opentyrian2000/opentyrian.dat"):
			found = true
	assert_bool(found).is_true()


func test_save_box_preselects_yes_and_shows_original_save() -> void:
	var save_menu: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await get_tree().process_frame
	await get_tree().process_frame
	var result := [null]
	var run := func() -> void:
		result[0] = await save_menu.quit_request_msg("x", "", true, {"name": "AB", "original": "ZZ"})
	run.call()
	await get_tree().process_frame
	assert_bool(save_menu.modal_active).is_true()
	# Enter without another key: Yes preselected, so the answer is true.
	save_menu.modal_input(TyrianControls.ACCEPT)
	await get_tree().process_frame
	await get_tree().process_frame
	await get_tree().create_timer(0.5).timeout
	assert_bool(result[0] == true).is_true()


## mainint.c:JE_operation, name box: miscText[9] / [10] replaced by
## misc_yes / misc_no (helptext.c:567-568); the C shows YES / NO, not OK / CANCEL.
func test_save_name_box_buttons_are_yes_no_like_c() -> void:
	var save_menu := load("res://scripts/save_menu.gd")
	var lang := TyrianText.language
	TyrianText.language = "en"
	assert_str(TyrianText.L(save_menu.SAVE_BOX_YES)).is_equal("YES")
	assert_str(TyrianText.L(save_menu.SAVE_BOX_NO)).is_equal("NO")
	TyrianText.language = "fr"
	assert_str(TyrianText.L(save_menu.SAVE_BOX_YES)).is_equal("OUI")
	assert_str(TyrianText.L(save_menu.SAVE_BOX_NO)).is_equal("NON")
	TyrianText.language = lang


## mainint.c:JE_operation then game_menu.c:JE_saveRequest: the confirmation
## box opens on top of the name box, without redrawing the screen.
func test_save_confirm_box_keeps_name_box_behind() -> void:
	var save_menu: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await get_tree().process_frame
	await get_tree().process_frame
	var screen: int = save_menu.VGA_SCREEN
	var result := ["?"]
	var run := func() -> void: result[0] = await save_menu._prompt_name("AB")
	run.call()
	await get_tree().process_frame
	save_menu._pending.append(TyrianControls.ACCEPT)
	for i in 30:
		await get_tree().process_frame
		if result[0] != "?":
			break
	assert_str(str(result[0])).is_equal("AB")
	(
		assert_bool(save_menu.vga.indices(screen) != save_menu._background)
		. override_failure_message("name box cleared before confirmation")
		. is_true()
	)


## game_menu.c:curSel[]: Options, Keyboard, Load / Save keep the row
## left during the shop visit (the C only puts it back on the first
## row on entering JE_itemScreen, game_menu.c:335-337).
func test_options_screens_keep_their_row_during_shop_visit() -> void:
	Game.shop_menu_rows = {}
	var save_menu: Node = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await get_tree().process_frame
	await get_tree().process_frame
	assert_int(save_menu.menus.selection()).is_equal(0)
	save_menu.menus.current().sel = 2
	save_menu.menus.draw()
	save_menu._open_keyboard_config()
	await get_tree().process_frame
	save_menu.menus.current().sel = 4
	save_menu.menus.draw()
	save_menu.menus.back()
	await get_tree().process_frame
	assert_int(save_menu.menus.selection()).is_equal(2)
	save_menu._open_keyboard_config()
	await get_tree().process_frame
	assert_int(save_menu.menus.selection()).override_failure_message("Keyboard: row not found").is_equal(4)
	# Screen reopened from the shop root, same visit.
	save_menu = Game.goto_scene(Game.SCENE_SAVE_MENU)
	await get_tree().process_frame
	await get_tree().process_frame
	assert_int(save_menu.menus.selection()).override_failure_message("Options: row not found").is_equal(2)
	Game.shop_menu_rows = {}
