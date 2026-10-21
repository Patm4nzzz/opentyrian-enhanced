## Level overlays from the C code: DPS counter (F6), STATISTICS table (F4) and
## achievements list (F3), drawn in the dashboard layers
## (HudRenderer) as enemy_render.c draws them in VGAScreenSeg.
##
## C source: stats.c:stats_display, achievements.c:achievements_display,
## enemy_render.c ("DPS:%u" at (6, 164)), input_map.c:input_action_toggle_*
## (F3 closes F4 and vice versa; the DPS is independent).
class_name LevelOverlays
extends RefCounted

const ACHIEVEMENTS := preload("res://scripts/achievements_screen.gd")
const OVER_A := 1
const OVER_B := 2
const PLAY_AREA_X := 24
const PLAY_AREA_WIDTH := 264
## C: TINY_FONT (width of JE_textWidth).
const TINY_FONT := 2

## stats.c: STATS_COL1_X, STATS_COL2_RIGHT, STATS_COL3_RIGHT, STATS_TITLE_Y.
const STATS_COL1 := 6
const STATS_COL2_RIGHT := 170
const STATS_COL3_RIGHT := 258
const STATS_TITLE_Y := 20
## Darkened band (JE_barShade, two passes).
const STATS_SHADE_Y0 := 12
const STATS_SHADE_Y1 := 136
const ACH_SHADE_Y0 := 4
const ACH_SHADE_Y1 := 183
## achievements.c: ACH_DISPLAY_*.
const ACH_X := 10
const ACH_Y_START := 18
const ACH_MAX_VISIBLE := 14
const ACH_LINE_HEIGHT := 9
const ACH_Y_BOTTOM := 148
const ACH_SELECTED_Y := 162
const ACH_DESC_Y := 173
const ACH_CATEGORY_COUNT := 6
## achievements.c:achievements_draw_notification: Y coordinate of the banner
## when one of the two lists is open.
const NOTIFY_Y_WITH_OVERLAY := 170
## Index of ACH_NIGHT_PLAYER (achievements.h): unlocked by the local time.
const ACH_NIGHT_PLAYER := 64

var stats_active := false
var achievements_active := false
var dps_active := false
var cursor := 0
var category := 0

var _sorted: Array = []
var _filtered: Array = []
var _sorted_language := ""


## input_map.c:input_action_toggle_achievements (F3): opens the list and
## closes the statistics.
func toggle_achievements() -> void:
	achievements_active = not achievements_active
	if achievements_active:
		stats_active = false


## input_map.c:input_action_toggle_stats (F4).
func toggle_stats() -> void:
	stats_active = not stats_active
	if stats_active:
		achievements_active = false


## input_map.c:input_action_toggle_dps (F6).
func toggle_dps() -> void:
	dps_active = not dps_active


## One of the two lists is open (the banner then moves down to y 170).
func is_open() -> bool:
	return stats_active or achievements_active


## Darkened band of the play area: [y0, y1], or [-1, -1].
func shade_rows() -> Vector2i:
	if achievements_active:
		return Vector2i(ACH_SHADE_Y0, ACH_SHADE_Y1)
	if stats_active:
		return Vector2i(STATS_SHADE_Y0, STATS_SHADE_Y1)
	return Vector2i(-1, -1)


## achievements.c:achievements_scroll_up / down (bounded cursor, no wrap-around).
func scroll(step: int) -> void:
	_ensure_sorted()
	cursor = clampi(cursor + step, 0, maxi(_filtered.size() - 1, 0))


## achievements.c:achievements_category_next / prev.
func change_category(step: int) -> void:
	category = (category + step + ACH_CATEGORY_COUNT) % ACH_CATEGORY_COUNT
	cursor = 0
	_ensure_sorted(true)


## achievements.c:ensure_sorted + rebuild_filtered_index: sort by localized
## name (natural_strcmp), redone when the language changes.
func _ensure_sorted(force := false) -> void:
	var language := TyrianText.L("ach_screen_title")
	if _sorted.is_empty() or language != _sorted_language:
		_sorted_language = language
		_sorted = range(ACHIEVEMENTS.ACHIEVEMENTS.size())
		_sorted.sort_custom(func(x: int, y: int) -> bool: return ACHIEVEMENTS.natural_strcmp(_name(x), _name(y)) < 0)
		force = true
	if not force:
		return
	_filtered = []
	for id in _sorted:
		if category == 0 or ACHIEVEMENTS.ACHIEVEMENTS[id][2] == category:
			_filtered.append(id)
	cursor = clampi(cursor, 0, maxi(_filtered.size() - 1, 0))


func _name(i: int) -> String:
	return TyrianText.L(ACHIEVEMENTS.ACHIEVEMENTS[i][0])


func _width(text: String) -> int:
	return int(ClassDB.class_call_static("TyrianVga", "text_width", text, TINY_FONT))


## JE_textShade(..., color, brightness, FULL_SHADE) in both layers.
func _text(vga: RefCounted, x: int, y: int, text: String, color: int, shade: int) -> void:
	for s in [OVER_A, OVER_B]:
		vga.text_shade(s, x + PLAY_AREA_X, y, text, color, shade, TyrianText.FULL_SHADE)


## enemy_render.c: snprintf("DPS:%u"), untranslated literal (the "DPS : %u" label
## of the French locale is only used on the STATISTICS screen).
static func dps_text(dps: int) -> String:
	return "DPS:%d" % dps


## Draws whatever is open. `stats`: TyrianSim.ach_stats(); `unlocked`:
## TyrianSim.ach_unlocked_flags() (the session's achievements count).
func draw(vga: RefCounted, stats: Dictionary, unlocked: PackedByteArray) -> void:
	if dps_active:
		# enemy_render.c: JE_textShade(VGAScreenSeg, 6, 164, "DPS:%u", 15, 4, FULL_SHADE).
		_text(vga, 6, 164, dps_text(int(stats.get("dps", 0))), 15, 4)
	if not is_open():
		return
	var rows := shade_rows()
	for pass_number in 2:
		vga.shade_overlay(PLAY_AREA_X, rows.x, PLAY_AREA_X + PLAY_AREA_WIDTH - 1, rows.y)
	if achievements_active:
		_draw_achievements(vga, unlocked)
	else:
		_draw_stats(vga, stats)


## stats.c:stats_display.
func _draw_stats(vga: RefCounted, stats: Dictionary) -> void:
	var dual: bool = bool(stats.get("run_active", false))
	var y := STATS_TITLE_Y
	var title := TyrianText.L("stats_title")
	_text(vga, 132 - _width(title) / 2, y, title, 15, 2)
	y += 14
	if dual:
		var header_run := TyrianText.L("stats_hdr_run")
		var header_global := TyrianText.L("stats_hdr_global")
		_text(vga, STATS_COL2_RIGHT - _width(header_run), y, header_run, 15, 2)
		_text(vga, STATS_COL3_RIGHT - _width(header_global), y, header_global, 15, 2)
		y += 12
	var rows := [
		["stats_lbl_enemies", str(stats.get("run_kills", 0)), str(stats.get("game_kills", 0))],
		["stats_lbl_shots", str(stats.get("run_shots", 0)), str(stats.get("game_shots", 0))],
		["stats_lbl_ratio", "%d%%" % _hit_ratio(stats, "run"), "%d%%" % _hit_ratio(stats, "game")],
		["stats_lbl_deaths", str(stats.get("run_deaths", 0)), str(stats.get("game_deaths", 0))],
		["stats_lbl_levels", str(stats.get("run_levels", 0)), str(stats.get("game_levels", 0))],
		["stats_lbl_score", str(stats.get("run_score", 0)), str(stats.get("game_score", 0))],
		["stats_lbl_time", _duration(int(stats.get("run_time_ms", 0))), _duration(int(stats.get("game_time_ms", 0)))],
	]
	for r in rows:
		_stat_row(vga, y, TyrianText.L(r[0]), r[1], r[2], dual)
		y += 12


## stats.c:stats_draw_stat: label, main value (the run's if it is
## active, otherwise the lifetime value) right-aligned, then the lifetime value.
func _stat_row(vga: RefCounted, y: int, label: String, run_value: String, life_value: String, dual: bool) -> void:
	_text(vga, STATS_COL1, y, label, 15, 2)
	var primary := run_value if dual else life_value
	_text(vga, STATS_COL2_RIGHT - _width(primary), y, primary, 15, 2)
	if dual:
		_text(vga, STATS_COL3_RIGHT - _width(life_value), y, life_value, 15, 2)


## stats.c: "(enemies_killed * 100) / shots_fired", 0 with no shots.
func _hit_ratio(stats: Dictionary, prefix: String) -> int:
	var shots := int(stats.get(prefix + "_shots", 0))
	return int(stats.get(prefix + "_kills", 0)) * 100 / shots if shots > 0 else 0


## stats.c: "%uh%um%us" (hours, minutes, seconds of time_played_ms).
func _duration(ms: int) -> String:
	var seconds := ms / 1000
	var minutes := seconds / 60
	return "%dh%dm%ds" % [minutes / 60, minutes % 60, seconds % 60]


## achievements.c:achievements_display.
func _draw_achievements(vga: RefCounted, unlocked_flags: PackedByteArray) -> void:
	_ensure_sorted()
	var flags := unlocked_flags.duplicate()
	flags.resize(ACHIEVEMENTS.ACHIEVEMENTS.size())
	# Night Player (0:00 to 4:59) is unlocked and saved by the simulation (achievements_check):
	# no more checked box at display time only.
	var unlocked_count := 0
	for f in flags:
		if f != 0:
			unlocked_count += 1
	var title := TyrianText.L("ach_screen_title")
	_text(vga, 160 - _width(title) / 2, 10, title, 15, 4)
	_text(vga, ACH_X, 10, "%d/%d" % [unlocked_count, ACHIEVEMENTS.ACHIEVEMENTS.size()], 15, 4)
	var category_name := TyrianText.L(ACHIEVEMENTS.CAT_KEYS[category])
	_text(vga, PLAY_AREA_WIDTH - 6 - _width(category_name), 10, category_name, 15, 4)
	var y := ACH_Y_START + 4
	if _filtered.is_empty():
		_text(vga, ACH_X, y, TyrianText.L("ach_empty_cat"), 14, 0)
		return
	# 14-line window centered on the cursor.
	var start := cursor - ACH_MAX_VISIBLE / 2
	if start > _filtered.size() - ACH_MAX_VISIBLE:
		start = _filtered.size() - ACH_MAX_VISIBLE
	start = maxi(start, 0)
	for vi in range(start, _filtered.size()):
		if y > ACH_Y_BOTTOM:
			break
		var id: int = _filtered[vi]
		var selected := vi == cursor
		var done: bool = flags[id] != 0
		_text(vga, ACH_X, y, "[X]" if done else "[ ]", 10 if done else 7, 4 if selected else 2)
		_text(vga, ACH_X + 24, y, _name(id), 15, 4 if selected else 2)
		y += ACH_LINE_HEIGHT
	var current: int = _filtered[cursor]
	_text(vga, ACH_X + 24, ACH_SELECTED_Y, _name(current), 10 if flags[current] != 0 else 15, 4)
	var description := TyrianText.L(ACHIEVEMENTS.ACHIEVEMENTS[current][1])
	if not description.is_empty():
		_text(vga, ACH_X + 24, ACH_DESC_Y, description, 15, 2)
