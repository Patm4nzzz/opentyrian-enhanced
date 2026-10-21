# title_screen.c:462: stop_all_samples() on entering the title. An effect started
# at the end of a level (1 to 2.5 s voice) does not continue over the menu.
extends GdUnitTestSuite


func test_stop_all_silences_every_channel() -> void:
	var sfx: Node = load("res://scripts/sfx.gd").new()
	sfx.play_in_headless = true
	add_child(sfx)
	sfx.play(14)
	sfx.play(21)
	assert_bool(sfx._players[0].playing).is_true()
	sfx.stop_all()
	for p in sfx._players:
		assert_bool(p.playing).is_false()
	sfx.free()


func test_title_entry_stops_samples() -> void:
	var src := FileAccess.get_file_as_string("res://scripts/title.gd")
	var ready := src.substr(src.find("func _ready()"), 400)
	assert_bool("Sfx.stop_all()" in ready).override_failure_message("title._ready must call Sfx.stop_all()").is_true()


# AUD-02: returning from a title subscreen does not cut the current sound
# (the C only cuts on entering titleScreen()).
func test_title_keeps_samples_when_coming_back_from_a_sub_screen() -> void:
	var src := FileAccess.get_file_as_string("res://scripts/title.gd")
	assert_bool("TITLE_SUB_SCREENS" in src and "Effects.previous_scene_path" in src).is_true()
	var title: Script = load("res://scripts/title.gd")
	for scene in ["load_screen", "high_scores", "instructions", "replays_screen", "jukebox"]:
		assert_bool(("res://scenes/%s.tscn" % scene) in title.TITLE_SUB_SCREENS).is_true()
	assert_bool("res://scenes/level.tscn" in title.TITLE_SUB_SCREENS).is_false()
	assert_bool("res://scenes/shop.tscn" in title.TITLE_SUB_SCREENS).is_false()


# AUD-01 / AUD-03: shop confirmation = S_CLICK (game_menu.c:2813);
# entering a level: fade_song before loading (tyrian2.c:394).
func test_shop_enter_is_click_and_level_entry_fades_song() -> void:
	var shop := FileAccess.get_file_as_string("res://scripts/shop.gd")
	# Shop entry = S_CLICK (game_menu.c:2813); the full behavior
	# (Esc, Left/Right) is verified by shop_sounds_test.
	assert_bool('"select":\n				Sfx.play(Sfx.CLICK)' in shop).is_true()
	var level := FileAccess.get_file_as_string("res://scripts/level.gd")
	var start := level.find("func _music_start()")
	var body := level.substr(start, 700)
	(
		assert_bool(
			body.find("Music.fade_song()") > 0 and body.find("Music.fade_song()") < body.find("fade_and_play_song")
		)
		. is_true()
	)
