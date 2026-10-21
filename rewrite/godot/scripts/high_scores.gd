## High scores screen (highscore.c:JE_highScoreScreen), frame by
## frame like the C version (ADR 0020).
##
## Tables: those drawn at random by config.c:JE_loadConfiguration when there is
## no save (the rewrite does not read `opentyrian.sav` yet),
## with the same rng.c generator and the same seed (rng_seed(0) in
## opentyr.c:main; --rng-seed=N for captures, see fixed_seed_shim.c).
extends TyrianScreen

const SCENE_TITLE := "res://scenes/title.tscn"

## highscore.c:JE_highScoreScreen.
const EPISODE_COUNT := 8  # five episodes, three timed battles
## Pages: the 8 above then the Endless leaderboard (deliberate divergence, decision of
## The maintainer on 2026-10-10: divergences.md, "Endless leaderboard in High Scores").
const ENDLESS_PAGE := EPISODE_COUNT
const PAGE_COUNT := EPISODE_COUNT + 1
## Endless leaderboard: 10 rows from y 50, spacing 10 (the style of the episode
## rows), mutator legend below the last one.
const Y_ENDLESS_ROWS := 50
const Y_ENDLESS_LEGEND := 152
const X_ENDLESS_MARK := 215
## Fixed x of the score (the rank "#10:" is the widest).
const ENDLESS_SCORE_X := 48
const X_CENTER := 160
const Y_MENU_HEADER := 3
const Y_EPISODE_HEADER := 30
const X_LEFT_CONTROL := 83
const X_RIGHT_CONTROL := 213
const Y_CONTROLS := 179
## Stray line in the backdrop (image 2) under row 2: see _erase_backdrop_dash.
const BACKDROP_DASH_Y := 86
const BACKDROP_DASH_X_FIRST := 100
const BACKDROP_DASH_X_LAST := 106
## config.c: COUNTOF(defaultHighScoreNames), COUNTOF(defaultTeamNames),
## SAVE_FILES_NUM.
const HIGH_SCORE_NAMES := 39
const TEAM_NAMES := 10
const SAVE_FILES_NUM := 22
## highscore.h:MUTATOR_SCORE_MARK.
const MUTATOR_SCORE_MARK := "*"

## Path of the high scores persistence file (JSON format).
static var SCORES_PATH: String = UserData.path("highscores.json")


## highscore.c:highscore_name_is_marked: the name ends with " *".
static func _is_marked(name: String) -> bool:
	var n := name.rstrip(" ")
	return n.ends_with(" *") or n.ends_with("  *")


## Serializes the 20 tables × 3 rows to JSON and writes them to SCORES_PATH.
## Does not touch opentyrian.dat: the rewrite uses a dedicated file.
static func save_scores() -> void:
	var rows := []
	for table in t2k_high_scores:
		var t := []
		for e in table:
			t.append({"score": e.score, "name": e.name, "difficulty": e.difficulty})
		rows.append(t)
	var path := ProjectSettings.globalize_path(SCORES_PATH)
	var f: FileAccess = FileAccess.open(path, FileAccess.WRITE)
	if f:
		f.store_string(JSON.stringify({"version": 1, "tables": rows}))
		f.close()
	else:
		# config.c:JE_saveConfiguration: temporary file not opened = warning
		# in the log only, the game continues.
		push_warning("scores not written (%s)" % error_string(FileAccess.get_open_error()))


## Reads SCORES_PATH and replaces t2k_high_scores; true if read successfully.
static func load_scores() -> bool:
	var path := ProjectSettings.globalize_path(SCORES_PATH)
	if not FileAccess.file_exists(path):
		return false
	var f: FileAccess = FileAccess.open(path, FileAccess.READ)
	if not f:
		return false
	var text := f.get_as_text()
	f.close()
	var parsed: Variant = JSON.parse_string(text)
	if not (parsed is Dictionary) or not parsed.has("tables"):
		return false
	var tables: Array = parsed["tables"]
	if tables.size() != EPISODE_COUNT + 12:	# 20 tables
		# Files written before the refactoring are also accepted.
		if tables.size() != 20:
			return false
	var out := []
	for t in tables:
		var rows: Array = t if t is Array else []
		var r := []
		for e: Variant in rows:
			r.append(
				{"score": int(e.get("score", 0)), "name": str(e.get("name", "")), "difficulty": int(e.get("difficulty", 0))}
			)
		out.append(r)
	sort_high_scores(out)
	t2k_high_scores = out
	return true


## mutators_apply_score: score × Risk/Reward multiplier.
static func apply_multiplier(score: int, mult: float) -> int:
	# mutators.c:mutators_apply_score: (int32_t)((double)score * mult), truncation, cap at
	# INT32_MAX; a score <= 0 is returned as is.
	if score <= 0 or mult == 1.0:
		return maxi(score, 0)
	return mini(int(float(score) * mult), 2147483647)


## mutators.c:mutators_score_multiplier: 1 + 0.25 × (number of the 7 other mutators) when
## Risk/Reward (0x80) is active, otherwise 1 (B-04).
static func mutator_multiplier(muts: int) -> float:
	if (muts & 0x80) == 0:
		return 1.0
	var count := 0
	for i in 7:
		if (muts & (1 << i)) != 0:
			count += 1
	return 1.0 + 0.25 * count


## Rank (0..2) at which `score` enters the table `table`, -1 if it does not enter
## (highscore.c: "temp_score > t2kHighScores[table][slot].score").
static func qualifying_slot(table: int, score: int) -> int:
	if t2k_high_scores.is_empty():
		if not load_scores():
			t2k_high_scores = default_high_scores()
	var t: Array = t2k_high_scores[table]
	for i in 3:
		if score > t[i].score:
			return i
	return -1


## "shift down old scores": done before the entry, so also when the
## player cancels it (row `slot` then remains a copy of the previous one).
static func shift_down(table: int, slot: int) -> void:
	var t: Array = t2k_high_scores[table]
	for i in range(2, slot, -1):
		t[i] = t[i - 1].duplicate()


## New row: name ("*" mark appended if mutators were active,
## name truncated to 26), score and difficulty.
static func set_entry(table: int, slot: int, score: int, name: String, difficulty: int, mut_marked: bool) -> void:
	var entry_name := name.strip_edges(false, true)
	if mut_marked:
		entry_name = entry_name.left(26) + " *"
	t2k_high_scores[table][slot] = {"score": score, "name": entry_name, "difficulty": difficulty}


## t2kHighScores[20][3]: {score, name, difficulty}, drawn only once.
static var t2k_high_scores: Array = []

## episodeIndex: local to the C, reset to 0 on each entry.
var episode_index := 0
## True when the screen is waiting for a key (for the capture tool).
var idle := false
var _busy := true


func _ready() -> void:
	if t2k_high_scores.is_empty():
		if not load_scores():
			t2k_high_scores = default_high_scores()
	_run.call_deferred()


## Seed of rng_seed(0): time(NULL) ^ (getpid() << 16) (rng.c:rng_seed).
static func _rng_seed() -> int:
	if Capture.has_option("rng-seed"):
		return int(Capture._options["rng-seed"])
	return int(Time.get_unix_time_from_system()) ^ (OS.get_process_id() << 16)


## config.c:JE_loadConfiguration, branch "We didn't have a save file!":
## same rng_next() draws, in the same order (saves then scores).
static func default_high_scores() -> Array:
	# Draws for the save slots (consumed, not displayed):
	# highScore1, then highScore2 + team name or a player name.
	var draws := 0
	for z in SAVE_FILES_NUM:
		draws += 3 if z % 6 > 2 else 2
	# Scores: 30 × 2 draws (timed battles) + 30 × 2 (episodes).
	var seq: PackedInt64Array = TyrianVga.rng_sequence(_rng_seed(), draws + 120)
	var k := draws
	var tables := []
	for z in 20:
		var rows := []
		for y in 3:
			var score: int
			var name: String
			if z < 10:
				score = (seq[k] % 50 + 1) * 100
				name = TyrianText.L("hsName_%d" % (seq[k + 1] % HIGH_SCORE_NAMES))
			else:
				score = (seq[k] % 20 + 1) * 1000
				if z & 1:
					name = TyrianText.L("teamName_%d" % (seq[k + 1] % TEAM_NAMES))
				else:
					name = TyrianText.L("hsName_%d" % (seq[k + 1] % HIGH_SCORE_NAMES))
			k += 2
			rows.append({"score": score, "name": name, "difficulty": 0})
		tables.append(rows)
	sort_high_scores(tables)
	return tables


## highscore.c:JE_sortHighScores, called by opentyr.c:main before the title:
## three conditional swaps (bubble sort of three rows, stable).
static func sort_high_scores(tables: Array) -> void:
	for t in tables:
		for pair in [[0, 1], [1, 2], [0, 1]]:
			if t[pair[1]].score > t[pair[0]].score:
				var tmp: Dictionary = t[pair[0]]
				t[pair[0]] = t[pair[1]]
				t[pair[1]] = tmp


func _run() -> void:
	# `restart`: backdrop (image 2, bottom band erased) and title in VGAScreen2.
	vga.load_pic(VGA_SCREEN2, 2, false)
	vga.fill_rectangle_xy(VGA_SCREEN2, 0, 192, 319, 199, 0)
	_erase_backdrop_dash()
	TyrianText.draw_font_hv_shadow(self, X_CENTER, Y_MENU_HEADER, TyrianText.L("hs_title"), LARGE_FONT, CENTERED, 15,
			-3, false, 2, VGA_SCREEN2)
	_draw()
	await fade_palette(10, 0, 255)
	present()
	_busy = false
	idle = true


## Image 2 carries a light one-pixel-high line (x 100 to 106, y 86), left over from the planet
## drawing: it fell just before the name of the 2nd row. The pixels are redone
## from the row below, without touching the rest of the backdrop.
func _erase_backdrop_dash() -> void:
	for x in range(BACKDROP_DASH_X_FIRST, BACKDROP_DASH_X_LAST + 1):
		vga.set_pixel(VGA_SCREEN2, x, BACKDROP_DASH_Y, vga.get_pixel(VGA_SCREEN2, x, BACKDROP_DASH_Y + 1))


func _draw_rows(board: int, y0: int) -> void:
	for i in 3:
		var e: Dictionary = t2k_high_scores[board][i]
		if e.difficulty > 9:
			e.difficulty = 0
		var rank: int = e.difficulty
		var y := y0 + 10 * i
		TyrianText.text_shade(self, 20, y, "~#%d:~  %d" % [i + 1, e.score], 15, 0, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, 110, y, e.name, 15, 2, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, 250, y, TyrianText.L("diffb_%d" % rank), 15, rank + (0 if rank == 0 else -1),
				TyrianText.FULL_SHADE)



## Body of the JE_highScoreScreen loop (before presentation).
func _draw() -> void:
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	if episode_index == ENDLESS_PAGE:
		_draw_endless_page()
	else:
		_draw_episode_page()
	# Pagination arrows (shopSpriteSheet = newsh1.shp, JE_loadCompShapes '1').
	if episode_index > 0:
		vga.blit_sprite2(VGA_SCREEN, X_LEFT_CONTROL, Y_CONTROLS, "newsh1.shp", 279, 0, 0, true)
	if episode_index < PAGE_COUNT - 1:
		vga.blit_sprite2(VGA_SCREEN, X_RIGHT_CONTROL, Y_CONTROLS, "newsh1.shp", 281, 0, 0, true)
	# Endless page: dedicated label ("change episode" would be wrong here).
	var help_key := "hs_help_endless" if episode_index == ENDLESS_PAGE else "hs_help"
	TyrianText.help_box(self, 103, 182, TyrianText.L(help_key), 25, 15)


## Endless page: rank and score, name ("*" mutators), "2P" mark and wave,
## on the x 20 / 110 / 215 / 250 columns of the episode pages. Empty leaderboard:
## the 10 rows stay displayed, at zero and with no name.
func _draw_endless_page() -> void:
	TyrianText.draw_font_hv_shadow(self, X_CENTER, Y_EPISODE_HEADER, TyrianText.L("endless_leaderboard_title"),
			NORMAL_FONT, CENTERED, 15, -3, false, 2)
	var board := EndlessSession.load_board()
	var any_mutators := false
	for i in EndlessSession.LEADERBOARD_SIZE:
		var y := Y_ENDLESS_ROWS + 10 * i
		var e: Dictionary = board[i] if i < board.size() else {}
		var score := int(e.get("score", 0))
		# Rank and score at fixed x: "#10:" is wider than "#1:" and shifted the score.
		TyrianText.text_shade(self, 20, y, "~#%d:~" % (i + 1), 15, 0, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, ENDLESS_SCORE_X, y, str(score), 15, 0, TyrianText.FULL_SHADE)
		if e.is_empty():
			continue
		var name := str(e.get("name", ""))
		var two_players := int(e.get("players", 1)) == 2
		var marked := int(e.get("mutators", 0)) != 0
		# Same cut as the end-of-game leaderboard (endless_end.gd): the "2P" mark stays readable.
		if marked:
			any_mutators = true
			name = EndlessSession.shorten_name(name, 7 if two_players else 11) + " *"
		else:
			name = EndlessSession.shorten_name(name, 9 if two_players else 13)
		TyrianText.text_shade(self, 110, y, name, 15, 2, TyrianText.FULL_SHADE)
		if two_players:
			TyrianText.text_shade(self, X_ENDLESS_MARK, y, TyrianText.L("endless_two_player_mark"), 12, 4,
					TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, 250, y,
				TyrianText.L("endless_wave_label").replace("%d", str(int(e.get("wave", 0)))), 15, 4,
				TyrianText.FULL_SHADE)
	if any_mutators:
		TyrianText.text_shade(self, 20, Y_ENDLESS_LEGEND,
				"%s %s" % [MUTATOR_SCORE_MARK, TyrianText.L("mut_score_legend")], 15, 0, TyrianText.FULL_SHADE)


func _draw_episode_page() -> void:
	var title: String
	var board_one: int
	var board_two: int
	if episode_index < 5:
		title = TyrianText.L("ep_%d" % (episode_index + 1))
		board_one = 10 + episode_index * 2
		board_two = 11 + episode_index * 2
	else:
		title = "%s %s" % [TyrianText.L("timedBattle_0"), TyrianText.L("timedBattle_%d" % (episode_index - 4))]
		board_one = episode_index - 5
		board_two = -1
	TyrianText.draw_font_hv_shadow(self, X_CENTER, Y_EPISODE_HEADER, title, NORMAL_FONT, CENTERED, 15, -3, false, 2)
	TyrianText.draw_font_hv_shadow(self, X_CENTER, 55, TyrianText.L("hs_1player"), NORMAL_FONT, CENTERED, 15, -3, false,
			2)
	_draw_rows(board_one, 75)
	if board_two >= 0:
		TyrianText.draw_font_hv_shadow(self, X_CENTER, 120, TyrianText.L("hs_2players"), NORMAL_FONT, CENTERED, 15, -3,
				false, 2)
		_draw_rows(board_two, 135)
	var any_marked := false
	for i in 3:
		any_marked = any_marked or _is_marked(t2k_high_scores[board_one][i].name)
		if board_two >= 0:
			any_marked = any_marked or _is_marked(t2k_high_scores[board_two][i].name)
	if any_marked:
		TyrianText.text_shade(self, 20, 106, "%s %s" % [MUTATOR_SCORE_MARK, TyrianText.L("mut_score_legend")], 15, 0,
				TyrianText.FULL_SHADE)


## Mouse (highscore.c:JE_highScoreScreen): left click on a visible
## arrow (x 83 / 213, width 24, y >= 179) = previous / next episode;
## right click = exit. Wheel (addition, absent from the C): previous /
## next episode, like Left / Right.
func handle_mouse(event: InputEvent) -> void:
	if _busy or not idle:
		return
	TyrianMouse.note(event)
	if TyrianMouse.is_wheel(event):
		return
	if not TyrianMouse.is_button_press(event):
		return
	var p: Vector2 = event.position
	if event.button_index == MOUSE_BUTTON_RIGHT:
		handle_action(TyrianControls.BACK)
	elif event.button_index == MOUSE_BUTTON_LEFT and p.y >= Y_CONTROLS:
		if episode_index > 0 and p.x >= X_LEFT_CONTROL and p.x < X_LEFT_CONTROL + 24:
			handle_action(TyrianControls.LEFT)
		elif episode_index < PAGE_COUNT - 1 and p.x >= X_RIGHT_CONTROL and p.x < X_RIGHT_CONTROL + 24:
			handle_action(TyrianControls.RIGHT)


## `switch (lastkey_scan)` of JE_highScoreScreen.
func handle_action(key: StringName) -> void:
	if _busy or not idle:
		return
	match key:
		TyrianControls.LEFT:
			Sfx.play(Sfx.CURSOR)
			episode_index = PAGE_COUNT - 1 if episode_index == 0 else episode_index - 1
		TyrianControls.RIGHT:
			Sfx.play(Sfx.CURSOR)
			episode_index = 0 if episode_index == PAGE_COUNT - 1 else episode_index + 1
		TyrianControls.SELECT, TyrianControls.ACCEPT, TyrianControls.BACK:
			Sfx.play(Sfx.SPRING)
			idle = false
			_busy = true
			await fade_black(15)
			Game.goto_scene(SCENE_TITLE)  # `restart = true` in titleScreen
			return
		_:
			pass  # any other key: new loop turn, same image
	_draw()
	present()
