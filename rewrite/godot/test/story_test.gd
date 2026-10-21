# Episode end and story pages (level_load.c:JE_loadMap, case 'P', 'W',
# 'Q'; mainint.c:JE_nextEpisode; highscore.c:JE_highScoreCheck), with
# real keys (Input.parse_input_event):
# - end of episode 1 (section 26): "Score:" page + secret
#   tip, "Congratulations" with name entry on the keyboard, score
#   table, "Episode 2: Treachery", then the episode 2 shop;
# - Esc during entry: score not recorded;
# - story page ]Wy (section 11, minefield warning) before the
#   section 12 shop;
# - reloaded game ("Load Game") started at episode 4: the best
#   score is offered at the end of episode 4 (initial_episode read from
#   the save);
# - timed battle: score = money, battle table;
# - reloaded 2-player save: 2-player shop.
extends GdUnitTestSuite

const HS := preload("res://scripts/high_scores.gd")

var _saved_dir := ""
var _saved_scores: Array = []


func before() -> void:
	EndlessSession.stop()
	_saved_dir = Game.saves_dir
	Game.saves_dir = "user://story_test"
	_saved_scores = HS.t2k_high_scores.duplicate(true)


func after() -> void:
	var dir := ProjectSettings.globalize_path(Game.saves_dir)
	if DirAccess.dir_exists_absolute(dir):
		for f in DirAccess.get_files_at(dir):
			DirAccess.remove_absolute(dir.path_join(f))
		DirAccess.remove_absolute(dir)
	Game.saves_dir = _saved_dir
	Game.campaign_active = false
	HS.t2k_high_scores = _saved_scores
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func before_test() -> void:
	# Low tables: any campaign score enters at the first rank.
	var tables := []
	for _i in 20:
		(
			tables
			. append(
				[
					{"score": 300, "name": "AAA", "difficulty": 2},
					{"score": 200, "name": "BBB", "difficulty": 2},
					{"score": 100, "name": "CCC", "difficulty": 2},
				]
			)
		)
	HS.t2k_high_scores = tables


func _check(cond: bool, what: String) -> void:
	assert_bool(cond).override_failure_message(what).is_true()


func _key(code: int, unicode := 0) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = code as Key
	ev.keycode = code as Key
	ev.unicode = unicode
	ev.pressed = true
	Input.parse_input_event(ev)
	var up := ev.duplicate() as InputEventKey
	up.pressed = false
	Input.parse_input_event(up)
	await get_tree().process_frame
	await get_tree().process_frame


func _type(text: String) -> void:
	for c in text:
		await _key(OS.find_keycode_from_string(c), c.unicode_at(0))


func _story() -> Node:
	var cur: Node = Effects.current_game_scene()
	return cur if cur != null and cur.scene_file_path == Game.SCENE_STORY else null


## Waits for the story scene to wait for a key (`idle`) or for the name
## entry (`name_entry`); null if another scene took its place. The screen must
## first leave the previous wait (`_left`: the key was taken).
var _left := true


func _wait_story(field: String, ms := 20000) -> Node:
	var t0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t0 < ms:
		var s := _story()
		if s != null and s.get(field) == true and _left:
			_left = false
			return s
		if s == null or (s.get("idle") != true and s.get("name_entry") != true):
			_left = true
		await get_tree().process_frame
	return null


func _wait_scene(path: String, ms := 20000) -> Node:
	var t0 := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t0 < ms:
		var cur: Node = Effects.current_game_scene()
		if cur != null and cur.scene_file_path == path and cur.is_inside_tree():
			return cur
		await get_tree().process_frame
	return null


## Lit pixels (value > 0.2) of the displayed image in a rectangle.
func _lit(scene: Node, x0: int, y0: int, x1: int, y1: int) -> int:
	var img: Image = scene.vga.image(0)
	var pal: PackedByteArray = scene.vga.palette_bytes()
	var n := 0
	for y in range(y0, y1):
		for x in range(x0, x1):
			if img.get_pixel(x, y).v > 0.2:
				n += 1
	return n if pal.size() > 0 else 0


## Game at the last section of episode `ep` (before ]Q[).
func _campaign_at(ep: int, section: int, initial_ep: int) -> void:
	Game.start_campaign(initial_ep, 2)
	Game.episode = ep
	Game.campaign_active = true
	Game.loadout = PackedInt32Array([1, 3, 4, 7, 2, 4, 3, 1, 0, 1, 15267])
	Game.debrief = {"all_players_alive": true}
	Game.main_level = section
	Game.advance_campaign()


func test_episode1_end_shows_score_hint_name_entry_table_then_episode2() -> void:
	_campaign_at(1, 26, 1)
	var s := await _wait_story("idle", 60000)
	_check(s != null, "end of episode 1 → score and tip page ('Press a key')")
	if s == null:
		return
	var score: int = Game.total_score()
	_check(score > 15267, "JE_totalScore: money + equipment value (%d)" % score)
	_check(_lit(s, 10, 55, 300, 65) > 40, "'Score:' row drawn")
	await _key(KEY_SPACE)
	s = await _wait_story("name_entry", 30000)
	_check(s != null, "score in the top three → 'Enter your name'")
	if s == null:
		return
	await get_tree().create_timer(1.0).timeout
	_check(_lit(s, 0, 3, 320, 25) > 100, "'Congratulations' in large font (%d)" % _lit(s, 0, 3, 320, 25))
	await _type("ace")
	await _key(KEY_BACKSPACE)
	await _type("e pilot")
	await _key(KEY_ENTER)
	s = await _wait_story("idle", 30000)
	_check(s != null, "score table then 'Press a key'")
	var row: Dictionary = HS.t2k_high_scores[10][0]
	_check(
		row.name == "ACE PILOT" and row.score == score,
		"1st rank episode 1: %s %d (expected ACE PILOT %d)" % [row.name, row.score, score]
	)
	_check(HS.t2k_high_scores[10][1].name == "AAA", "old scores shifted (%s)" % HS.t2k_high_scores[10][1].name)
	await _key(KEY_SPACE)
	s = await _wait_story("idle", 30000)
	_check(s != null and Game.episode == 2, "'Episode 2: Treachery / Press a key' (episode %d)" % Game.episode)
	_check(
		Game.level_name == "Completed" and Game.game_has_repeated == false,
		"lastLevelName = Completed (%s)" % Game.level_name
	)
	await _key(KEY_SPACE)
	var shop := await _wait_scene(Game.SCENE_SHOP, 30000)
	_check(
		shop != null and Game.episode == 2 and Game.shop_section >= 1, "episode 2 shop (section %d)" % Game.shop_section
	)


func test_escape_cancels_name_entry() -> void:
	_campaign_at(1, 26, 1)
	var s := await _wait_story("idle", 60000)
	if s == null:
		_check(false, "end page missing")
		return
	await _key(KEY_SPACE)
	s = await _wait_story("name_entry", 30000)
	_check(s != null, "name entry open")
	await _type("NOPE")
	await _key(KEY_ESCAPE)
	s = await _wait_story("idle", 30000)
	_check(s != null, "Escape → table anyway (highscore.c)")
	_check(
		HS.t2k_high_scores[10][0].name == "AAA" and HS.t2k_high_scores[10][0].score == 300,
		"Escape: nothing entered at 1st rank (%s)" % HS.t2k_high_scores[10][0].name
	)


func test_warning_page_before_deliani_shop() -> void:
	_campaign_at(1, 11, 1)
	var s := await _wait_story("idle", 60000)
	_check(s != null, "section 11: 'Warning:' page (]Wy 01[) before the shop")
	if s == null:
		return
	_check(_lit(s, 0, 0, 320, 6) > 300, "alert bars at the top of the screen")
	await _key(KEY_SPACE)
	var shop := await _wait_scene(Game.SCENE_SHOP, 30000)
	_check(shop != null and Game.shop_section == 12, "then section 12 shop (%d)" % Game.shop_section)


func test_episode4_end_reached_from_load_game_asks_name() -> void:
	# Game started at episode 4, saved at section 43 (end of the
	# episode, ]Q[), then reloaded through "Load Game".
	Game.start_campaign(4, 2)
	await get_tree().process_frame
	Game.loadout = PackedInt32Array([1, 3, 4, 7, 2, 4, 3, 1, 0, 1, 15267])
	_check(Game.save_campaign(3, "EP4", 43), "game save")
	Game.initial_episode = 1
	var slot: Dictionary = Game.campaign_slots()[2]
	_check(
		int(slot.get("initial_episode", 0)) == 4,
		"starting episode read back from the save (%s)" % slot.get("initial_episode")
	)
	Game.load_campaign_slot(slot)
	_check(Game.initial_episode == 4, "load_campaign_slot pose initial_episode (%d)" % Game.initial_episode)
	var s := await _wait_story("idle", 60000)
	_check(s != null, "episode 4: score and tip page")
	await _key(KEY_SPACE)
	s = await _wait_story("name_entry", 30000)
	_check(s != null, "reloaded game started at episode 4 → name entry")
	await _key(KEY_ENTER)
	await _wait_story("idle", 30000)
	_check(HS.t2k_high_scores[16][0].score > 300, "episode 4 table (%d)" % HS.t2k_high_scores[16][0].score)


func test_two_player_save_reopens_two_player_shop() -> void:
	(
		Game
		. start_campaign(
			1,
			2,
			{
				"loadout": PackedInt32Array([11, 1, 1, 0, 1, 4, 2, 0, 0, 0, 500]),
				"difficulty": 3,
				"modes": PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2]),
				"modes_extra": [500, 700, 2, PackedInt32Array([11, 1, 1, 15, 1, 4, 2, 0, 0, 0])],
			}
		)
	)
	await _wait_scene(Game.SCENE_SHOP)
	_check(Game.save_campaign(13, "DUO"), "2-player save")
	Game.campaign_modes = PackedInt32Array()
	Game.campaign_modes_extra = []
	var slot: Dictionary = Game.campaign_slots()[12]
	Game.load_campaign_slot(slot)
	var shop := await _wait_scene(Game.SCENE_SHOP)
	_check(shop != null and Game.two_player_game(), "2-player game reloaded (modes %s)" % [Game.campaign_modes])
	_check(Game.shop_start_menu() == 9, "2-player shop (menu %d)" % Game.shop_start_menu())
	_check(int(Game.campaign_modes_extra[1]) == 700, "player 2 money restored (%s)" % [Game.campaign_modes_extra])
	Game.campaign_modes = PackedInt32Array()
	Game.campaign_modes_extra = []
	Game.modes = PackedInt32Array()
	Game.modes_extra = []
	Game.goto_scene(Game.SCENE_TITLE)
	await get_tree().process_frame
	await get_tree().process_frame


func test_timed_battle_score_is_cash_in_its_table() -> void:
	# Timed battle 2, finished: section 48 of episode 1 (]q[).
	Game.campaign_active = true
	Game.episode = 1
	Game.initial_episode = 1
	Game.campaign_modes = PackedInt32Array([0, 0, 1, 0, 0, 2, 0, 0, 0, 0])
	Game.campaign_modes_extra = []
	Game.modes = Game.campaign_modes
	Game.time_battle_selection = 2
	Game.loadout = PackedInt32Array([8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 4200])
	Game.debrief = {"all_players_alive": true}
	Game.main_level = 48
	Game.advance_campaign()
	var s := await _wait_story("name_entry", 30000)
	_check(s != null, "timed battle: name entry")
	await _key(KEY_ESCAPE)
	await _wait_story("idle", 30000)
	_check(
		HS.t2k_high_scores[1][0].score == 4200,
		"battle 2 table: money 4200, Escape without effect (%d)" % HS.t2k_high_scores[1][0].score
	)
	await _key(KEY_SPACE)
	var title := await _wait_scene(Game.SCENE_TITLE, 30000)
	_check(title != null, "then return to the title")
	Game.time_battle_selection = 0
	Game.campaign_modes = PackedInt32Array()
	Game.modes = PackedInt32Array()
	await get_tree().process_frame
	await get_tree().process_frame
