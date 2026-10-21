## Title-screen achievements screen (title_screen.c, MENU_ITEM_ACHIEVEMENTS, and
## achievements.c:achievements_display), rendered frame by frame on
## TyrianScreen (ADR 0020).
##
## List sorted by localized name (natural_strcmp), filtered by category
## (Left/Right), 14-line window centered on the cursor, detail of the
## selected achievement. No achievement is unlocked on the Godot side yet (achievement
## tracking is not ported): all rows show "[ ]".
extends TyrianScreen

const SCENE_TITLE := "res://scenes/title.tscn"

## achievements.c : ACH_DISPLAY_*.
const X := 10
const Y_START := 18
const MAX_VISIBLE := 14
const LINE_HEIGHT := 9
const Y_BOTTOM := 148
const SELECTED_Y := 162
const DESC_Y := 173
const PLAY_AREA_WIDTH := 264
const CAT_COUNT := 6
## Darkened panel behind the list (same technique as the pause menu:
## stacked JE_barShade) and a footer band across the full width,
## which covers the background text ("© 199x ... DANC...").
const PANEL_LEFT := 3
const PANEL_RIGHT := 316
const PANEL_TOP := 3
const FOOTER_TOP := 185
const PANEL_SHADES := 4
## Status box: 10 × 8 pixel frame, filled and checked when unlocked.
const BOX_WIDTH := 10
const BOX_HEIGHT := 8
const BOX_DIM_COLOR := 0x56
const BOX_FILL_COLOR := 0xD9
const BOX_EDGE_COLOR := 0xDC
const BOX_CHECK_COLOR := 0xDF
const SELECTION_BAR_COLOR := 0x13
## Pixels of the checkmark inside the box (column, row).
const BOX_CHECK_PIXELS := [
	Vector2i(2, 3), Vector2i(2, 4), Vector2i(3, 4), Vector2i(3, 5), Vector2i(4, 5), Vector2i(5, 4), Vector2i(5, 3),
	Vector2i(6, 3), Vector2i(6, 2), Vector2i(7, 2), Vector2i(7, 1),
]
## Index of ACH_NIGHT_PLAYER in the enum (achievements.h).
const ACH_NIGHT_PLAYER := 64
const CAT_KEYS := ["ach_cat_all", "ach_cat_combat", "ach_cat_survival", "ach_cat_mutator", "ach_cat_progress",
		"ach_cat_meta"]

## Order of the ACH_* enum (achievements.h): name key, description
## key, category (ach_category_of). All texts come from the
## locales (all six languages have the 200 keys: locale_check).
const ACHIEVEMENTS := [
	["ach_annihilator", "achd_annihilator", 1],
	["ach_boss_slayer", "achd_boss_slayer", 1],
	["ach_bullet_hell", "achd_bullet_hell", 1],
	["ach_bullet_storm", "achd_bullet_storm", 1],
	["ach_bullet_time", "achd_bullet_time", 1],
	["ach_centurion", "achd_centurion", 1],
	["ach_chain_reaction", "achd_chain_reaction", 1],
	["ach_close_call", "achd_close_call", 2],
	["ach_combo_king", "achd_combo_king", 1],
	["ach_daredevil", "achd_daredevil", 3],
	["ach_dedicated", "achd_dedicated", 4],
	["ach_destroyer", "achd_destroyer", 1],
	["ach_die_hard", "achd_die_hard", 5],
	["ach_endurance", "achd_endurance", 5],
	["ach_firepower", "achd_firepower", 1],
	["ach_firepower2", "achd_firepower2", 1],
	["ach_episode_clear", "achd_episode_clear", 4],
	["ach_first_blood", "achd_first_blood", 1],
	["ach_games_10", "achd_games_10", 4],
	["ach_games_50", "achd_games_50", 4],
	["ach_genocide", "achd_genocide", 1],
	["ach_glass_cannon", "achd_glass_cannon", 3],
	["ach_glass_house", "achd_glass_house", 3],
	["ach_hard_mode", "achd_hard_mode", 3],
	["ach_high_roller", "achd_high_roller", 3],
	["ach_immortal", "achd_immortal", 2],
	["ach_impossible", "achd_impossible", 3],
	["ach_insanity", "achd_insanity", 3],
	["ach_ironman", "achd_ironman", 2],
	["ach_level_5", "achd_level_5", 4],
	["ach_level_20", "achd_level_20", 4],
	["ach_level_50", "achd_level_50", 4],
	["ach_marathon", "achd_marathon", 4],
	["ach_max_power", "achd_max_power", 4],
	["ach_millionaire", "achd_millionaire", 4],
	["ach_mirror_master", "achd_mirror_master", 3],
	["ach_mirror_veteran", "achd_mirror_veteran", 3],
	["ach_modifier", "achd_modifier", 3],
	["ach_mut_survivor", "achd_mut_survivor", 3],
	["ach_night_owl", "achd_night_owl", 4],
	["ach_obsessed", "achd_obsessed", 4],
	["ach_overkill", "achd_overkill", 1],
	["ach_pacifist", "achd_pacifist", 2],
	["ach_perseverance", "achd_perseverance", 5],
	["ach_phoenix", "achd_phoenix", 2],
	["ach_photographer", "achd_photographer", 5],
	["ach_rich", "achd_rich", 4],
	["ach_screenshot_pro", "achd_screenshot_pro", 5],
	["ach_sharpshooter", "achd_sharpshooter", 1],
	["ach_shopaholic", "achd_shopaholic", 4],
	["ach_slow_motion_pro", "achd_slow_motion_pro", 1],
	["ach_sniper", "achd_sniper", 1],
	["ach_speedster", "achd_speedster", 3],
	["ach_survivor", "achd_survivor", 2],
	["ach_trigger_happy", "achd_trigger_happy", 1],
	["ach_untouchable", "achd_untouchable", 2],
	["ach_all_episodes", "achd_all_episodes", 4],
	["ach_boss_shredder", "achd_boss_shredder", 1],
	["ach_comeback_kid", "achd_comeback_kid", 2],
	["ach_filmmaker", "achd_filmmaker", 5],
	["ach_full_house", "achd_full_house", 4],
	["ach_glass_master", "achd_glass_master", 3],
	["ach_mega_rich", "achd_mega_rich", 4],
	["ach_modded_run", "achd_modded_run", 5],
	["ach_night_player", "achd_night_player", 5],
	["ach_perfect_score", "achd_perfect_score", 1],
	["ach_polyglot", "achd_polyglot", 5],
	["ach_quick_draw", "achd_quick_draw", 1],
	["ach_true_pacifist", "achd_true_pacifist", 2],
	["ach_zen_master", "achd_zen_master", 2],
	["ach_boss_rush", "achd_boss_rush", 1],
	["ach_perfect_boss", "achd_perfect_boss", 1],
	["ach_overclock", "achd_overclock", 1],
	["ach_arsenal_master", "achd_arsenal_master", 1],
	["ach_execution_chain", "achd_execution_chain", 1],
	["ach_bullet_blizzard", "achd_bullet_blizzard", 1],
	["ach_iron_curtain", "achd_iron_curtain", 2],
	["ach_dodge_artist", "achd_dodge_artist", 2],
	["ach_flawless_episode", "achd_flawless_episode", 2],
	["ach_adamantium", "achd_adamantium", 2],
	["ach_bulletproof", "achd_bulletproof", 2],
	["ach_last_stand", "achd_last_stand", 2],
	["ach_bullet_hell_boss", "achd_bullet_hell_boss", 3],
	["ach_one_life_episode", "achd_one_life_episode", 3],
	["ach_mirror_boss", "achd_mirror_boss", 3],
	["ach_risk_taker", "achd_risk_taker", 3],
	["ach_chaos_theory", "achd_chaos_theory", 3],
	["ach_mutator_collector", "achd_mutator_collector", 3],
	["ach_level_100", "achd_level_100", 4],
	["ach_billionaire", "achd_billionaire", 4],
	["ach_weapon_collector", "achd_weapon_collector", 4],
	["ach_ship_collector", "achd_ship_collector", 4],
	["ach_sidekick_swap", "achd_sidekick_swap", 4],
	["ach_patient_investor", "achd_patient_investor", 4],
	["ach_completionist", "achd_completionist", 5],
	["ach_achievement_hunter", "achd_achievement_hunter", 5],
	["ach_speed_demon", "achd_speed_demon", 5],
	["ach_endless_warrior", "achd_endless_warrior", 5],
	["ach_endless_legend", "achd_endless_legend", 5],
	["ach_arcade_entry", "achd_arcade_entry", 5],
]

var category := 0
var cursor := 0
var idle := false
var _busy := true
var _sorted: Array = []
var _filtered: Array = []
var _unlocked := PackedByteArray()


## Achievements list (shared engine, ADR 0121).
var menus := MenuEngine.new(self)


func _ready() -> void:
	mouse_cursor = false
	_run.call_deferred()


## achievements.c and replay.c have no mouse access: no hover, no click,
## no cursor. Only the wheel scrolls the list, one row per notch,
## like Up / Down (deliberate divergence requested by the maintainer, 2026-10-10).
func handle_mouse(event: InputEvent) -> void:
	var step := TyrianMouse.wheel_step(event)
	if step != 0:
		handle_action(TyrianControls.UP if step < 0 else TyrianControls.DOWN)


func _name(i: int) -> String:
	return TyrianText.L(ACHIEVEMENTS[i][0])


func _desc(i: int) -> String:
	return TyrianText.L(ACHIEVEMENTS[i][1])


## Name bytes as C sorts them: locale.c:utf8_to_cp437 is applied when the
## locale loads, so "á" counts as "a" (achievements.c:1365).
static func cp437_bytes(s: String) -> PackedByteArray:
	if ClassDB.class_exists("TyrianVga"):
		return ClassDB.class_call_static("TyrianVga", "cp437_bytes", s)
	return s.to_utf8_buffer()


## achievements.c:natural_strcmp (CP437 bytes, numbers compared by value).
static func natural_strcmp(sa: String, sb: String) -> int:
	var a := cp437_bytes(sa)
	var b := cp437_bytes(sb)
	var i := 0
	var j := 0
	while i < a.size() and j < b.size():
		if a[i] >= 48 and a[i] <= 57 and b[j] >= 48 and b[j] <= 57:
			var na := 0
			var nb := 0
			while i < a.size() and a[i] >= 48 and a[i] <= 57:
				na = na * 10 + a[i] - 48
				i += 1
			while j < b.size() and b[j] >= 48 and b[j] <= 57:
				nb = nb * 10 + b[j] - 48
				j += 1
			if na != nb:
				return -1 if na < nb else 1
		else:
			if a[i] != b[j]:
				return a[i] - b[j]
			i += 1
			j += 1
	return (a[i] if i < a.size() else 0) - (b[j] if j < b.size() else 0)


## ensure_sorted + rebuild_filtered_index.
func _rebuild() -> void:
	if _sorted.is_empty():
		_sorted = range(ACHIEVEMENTS.size())
		_sorted.sort_custom(func(x, y): return natural_strcmp(_name(x), _name(y)) < 0)
	_filtered = []
	for id in _sorted:
		if category == 0 or ACHIEVEMENTS[id][2] == category:
			_filtered.append(id)
	if cursor >= _filtered.size():
		cursor = _filtered.size() - 1 if _filtered.size() > 0 else 0


## Bytes 0/1 of the OTYE block (ACH_* order), empty if no valid file.
func _load_unlocked() -> void:
	var path := UserData.path(CProfile.FILE_NAME)
	if Capture.has_option("ot-dat"):
		path = str(Capture._options["ot-dat"])
	if not Capture.active:
		# title_screen.c: achievements_check() before the screen, all the rules
		# (not just Night Player), profile rewritten if anything triggers.
		var hour_now: int = Game.local_hour()
		_unlocked = TyrianVga.ach_title_check(path, Game.language_index(), hour_now, Game.enabled_mod_count())
		if _unlocked.size() < ACHIEVEMENTS.size():
			_unlocked.resize(ACHIEVEMENTS.size())
		return
	_unlocked = TyrianVga.achievements_unlocked_from_dat(path)
	if _unlocked.size() < ACHIEVEMENTS.size():
		_unlocked.resize(ACHIEVEMENTS.size())  # zeros: locked
	# title_screen.c calls achievements_check() before the screen; the only
	# rule that can trigger there without a game in progress is Night Player
	# (achievements.c:achievements_check, localtime() between 0:00 and 4:59).
	# `--unix-time=N`: frozen instant, like time() under fixed_seed_shim.c
	# (0x5EED) for parity captures; local time = current time zone.
	var hour: int = Time.get_datetime_dict_from_system(false)["hour"]
	if Capture.has_option("unix-time"):
		var bias: int = Time.get_time_zone_from_system()["bias"]
		hour = Time.get_datetime_dict_from_unix_time(int(Capture._options["unix-time"]) + bias * 60)["hour"]
	if hour < 5:
		_unlocked[ACH_NIGHT_PLAYER] = 1


func _is_unlocked(i: int) -> bool:
	return i < _unlocked.size() and _unlocked[i] != 0


func _run() -> void:
	_load_unlocked()
	vga.load_pic(VGA_SCREEN2, 2, false)
	_rebuild()
	_open_menu()
	_draw()
	present()
	await fade_palette(10, 0, 255)
	idle = true
	_busy = false


func _unlocked_count() -> int:
	var n := 0
	for i in ACHIEVEMENTS.size():
		if _is_unlocked(i):
			n += 1
	return n


## Achievements list: shared engine, ROWS style (ADR 0121). Rows are
## rebuilt on every category change.
func _open_menu() -> void:
	var rows := []
	for vi in _filtered.size():
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"id": _filtered[vi], "draw": _draw_row}))
	menus.stack.clear()
	menus.open("", rows, {
		"style": MenuEngine.Style.ROWS, "draw": false, "sel": cursor, "wrap": false, "redraw_on_other": true,
		# title_screen.c:855-871: no sound when scrolling (no JE_playSampleNum).
		"move_sfx": -1,
		"row_x": X, "row_y": Y_START + 4, "row_step": LINE_HEIGHT,
		# Loop "y <= ACH_DISPLAY_Y_BOTTOM": at most 15 rows.
		"visible": (Y_BOTTOM - Y_START - 4) / LINE_HEIGHT + 1,
		# Window centered on the cursor (achievements_display).
		"window": func(sel: int, n: int) -> int:
			var start := sel - MAX_VISIBLE / 2
			if start > n - MAX_VISIBLE:
				start = n - MAX_VISIBLE
			return maxi(start, 0),
		"background": _draw_background,
		"header_draw": _draw_header,
		"decorate": _draw_details,
		"shortcuts": {
			TyrianControls.LEFT: _change_category.bind(CAT_COUNT - 1),
			TyrianControls.RIGHT: _change_category.bind(1),
			TyrianControls.ACCEPT: _leave,
		},
		"on_back": _leave,
	})


## Loop body: background, achievements_display, help (without present()).
## (Also the virtual CanvasItem._draw, called before `_run`: nothing to do
## until the list is open.)
func _draw() -> void:
	if not menus.stack.is_empty():
		menus.render()


func _draw_header() -> void:
	var title := TyrianText.L("ach_screen_title")
	TyrianText.text_shade(self, 160 - TyrianText.width(title, SMALL_FONT) / 2, 10, title, 15, 4, TyrianText.FULL_SHADE)
	TyrianText.text_shade(self, X, 10, "%d/%d" % [_unlocked_count(), ACHIEVEMENTS.size()], 15, 4, TyrianText.FULL_SHADE)
	var cat_name := TyrianText.L(CAT_KEYS[category])
	TyrianText.text_shade(self, PLAY_AREA_WIDTH - 6 - TyrianText.width(cat_name, SMALL_FONT), 10, cat_name, 15, 4,
			TyrianText.FULL_SHADE)


## Title background, darkened panel behind the list, then the footer band.
func _draw_background() -> void:
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	vga.fill_rectangle_xy(VGA_SCREEN, 0, FOOTER_TOP, 319, 199, 0)
	for i in PANEL_SHADES:
		vga.bar_shade(VGA_SCREEN, PANEL_LEFT, PANEL_TOP, PANEL_RIGHT, FOOTER_TOP - 1)


## Status box: empty dark frame (locked) or filled checked box (unlocked).
func _draw_box(x: int, y: int, unlocked: bool) -> void:
	var right := x + BOX_WIDTH - 1
	var bottom := y + BOX_HEIGHT - 1
	if not unlocked:
		vga.fill_rectangle_xy(VGA_SCREEN, x, y, right, bottom, BOX_DIM_COLOR)
		vga.fill_rectangle_xy(VGA_SCREEN, x + 1, y + 1, right - 1, bottom - 1, 0)
		return
	vga.fill_rectangle_xy(VGA_SCREEN, x, y, right, bottom, BOX_EDGE_COLOR)
	vga.fill_rectangle_xy(VGA_SCREEN, x + 1, y + 1, right - 1, bottom - 1, BOX_FILL_COLOR)
	for p in BOX_CHECK_PIXELS:
		vga.set_pixel(VGA_SCREEN, x + p.x, y + p.y, BOX_CHECK_COLOR)


func _draw_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var un := _is_unlocked(it.id)
	if sel:
		vga.fill_rectangle_xy(VGA_SCREEN, PANEL_LEFT + 2, y - 1, PANEL_RIGHT - 2, y + 7, SELECTION_BAR_COLOR)
	_draw_box(x + 2, y, un)
	var brightness := (4 if sel else 3) if un else (2 if sel else 0)
	TyrianText.text_shade(self, x + 24, y, _name(it.id), 15, brightness, TyrianText.FULL_SHADE)


## Empty category, detail of the selected achievement, help.
func _draw_details() -> void:
	if _filtered.is_empty():
		TyrianText.text_shade(self, X, Y_START + 4, TyrianText.L("ach_empty_cat"), 14, 0, TyrianText.FULL_SHADE)
	else:
		var idx: int = _filtered[menus.selection()]
		TyrianText.text_shade(self, X + 24, SELECTED_Y, _name(idx), 10 if _is_unlocked(idx) else 15, 4,
				TyrianText.FULL_SHADE)
		var d := _desc(idx)
		if d != "":
			TyrianText.text_shade(self, X + 24, DESC_Y, d, 15, 2, TyrianText.FULL_SHADE)
	TyrianText.text_shade(self, 10, 190, TyrianText.L("ach_scroll"), 15, 4, TyrianText.FULL_SHADE)


## Left / Right: previous / next category, cursor at the top.
func _change_category(step: int) -> bool:
	category = (category + step) % CAT_COUNT
	cursor = 0
	_rebuild()
	_open_menu()
	return true


func _leave() -> void:
	idle = false
	_busy = true
	Game.goto_scene(SCENE_TITLE)  # `restart = true` (no fade)


## Navigation: shared engine (clamped cursor, no wrapping).
func handle_action(key: StringName) -> void:
	if _busy or not idle:
		return
	menus.handle_action(key)
	cursor = menus.selection()
