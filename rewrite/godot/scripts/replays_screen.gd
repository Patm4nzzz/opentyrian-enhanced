## Replay browser of the title (title_screen.c:replay_browser), rendered
## frame by frame on TyrianScreen (ADR 0020).
##
## Ported: the list of `.rep` files in `<user folder>/replays` (headers read
## by replay.c:replay_read_header via TyrianVga.replay_header), sorted by name
## in descending order, 8 per page, Up/Down navigation with wrap-around and scrolling,
## and the empty-folder screen. Folder: `--replay-dir=` (captures), otherwise
## `userdata/replays`. Enter on a file: replay_load checks (file
## refused, no frames: red message at the foot
## of the page, SPRING sound) then playback in the level scene, through the same
## path as `--rep=` (Game.replay_file, TyrianSim.load_replay). Esc -> title.
extends TyrianScreen

const SCENE_TITLE := "res://scenes/title.tscn"
const ITEMS_PER_PAGE := 8
const UP_INDICATOR_COLOR := 15 * 16 + 9  # same color as the "v" (text_shade colorbank 15, brightness 2)
const BROWSER_MAX := 64  # title_screen.c:REPLAY_BROWSER_MAX

var idle := false
var _busy := true
var _names: Array[String] = []
var _headers: Array[PackedInt64Array] = []
## File refusal: replaces the footer for ~2.5 s (status_text /
## status_until of replay_browser).
var _status := ""
var _status_until := 0
## File list (shared engine, ADR 0121).
var menus := MenuEngine.new(self)


func _ready() -> void:
	mouse_cursor = false
	_scan()
	_run.call_deferred()


## achievements.c and replay.c have no mouse access: no hover, no click,
## no cursor.
func handle_mouse(_event: InputEvent) -> void:
	pass


func _replay_dir() -> String:
	return ReplayRecording.directory()


## Folder scan: only the `.rep` files whose header is accepted.
func _scan() -> void:
	var dir := _replay_dir()
	var entries: Array = []
	# Missing folder (no replay recorded): empty list without error,
	# like a failing opendir_utf8 in title_screen.c:replay_browser.
	if not DirAccess.dir_exists_absolute(dir):
		return
	for f in DirAccess.get_files_at(dir):
		if entries.size() >= BROWSER_MAX:
			break
		if f.length() < 5 or not f.ends_with(".rep"):
			continue
		var h: PackedInt64Array = TyrianVga.replay_header(dir.path_join(f))
		if h.is_empty():
			continue
		entries.append([f.substr(0, f.length() - 4), h])
	# Sorted by name in descending order (strcmp; timestamped names YYYYMMDD-HHMMSS).
	entries.sort_custom(func(a, b): return String(a[0]) > String(b[0]))
	for e in entries:
		_names.append(e[0])
		_headers.append(e[1])


func _run() -> void:
	# JE_loadPic(VGAScreen, 4): palette of image 4, screen cleared afterwards.
	vga.load_pic(VGA_SCREEN, 4, false)
	_draw()
	present()
	await fade_palette(10, 0, 255)
	idle = true
	_busy = false


## Replay list: shared engine, ROWS style (ADR 0121).
func _open_menu() -> void:
	var rows := []
	for idx in _names.size():
		rows.append(MenuEngine.item(_names[idx], "", MenuEngine.Kind.CUSTOM, {"index": idx, "draw": _draw_row}))
	menus.open("", rows, {
		"style": MenuEngine.Style.ROWS, "draw": false,
		"row_x": 20, "row_y": 30, "row_step": 20, "visible": ITEMS_PER_PAGE,
		"redraw_on_other": true,
		"on_activate": _activate,
		"background": func() -> void: vga.clear(VGA_SCREEN, 0),
		"header_draw": func() -> void:
			TyrianText.draw_font_hv(self, 160, 8, TyrianText.L("menu_replays"), NORMAL_FONT, CENTERED, 15, -1),
		"decorate": _draw_footer,
		"on_back": func() -> void:
			idle = false
			_busy = true
			Game.goto_scene(SCENE_TITLE),
	})


## Body of the replay_browser loop (without present()).
func _draw() -> void:
	if menus.stack.is_empty():
		_open_menu()
	menus.render()


## One row: file name then header summary.
func _draw_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var idx: int = it.index
	TyrianText.draw_font_hv(self, x, y, _names[idx], NORMAL_FONT, LEFT, 15, -1 if sel else -4)
	var h := _headers[idx]
	var diff_name := TyrianText.L("diffb_%d" % h[2]) if h[2] < 11 else "???"
	var mode_name := TyrianText.L("replay_full_game")
	if h[3] == 1:
		mode_name = TyrianText.L("replay_1p_arcade")
	elif h[3] == 2:
		mode_name = TyrianText.L("replay_2p_arcade")
	var info := "%s  Ep%d Lv%d  %s  %d frames" % [mode_name, h[0], h[1], diff_name, h[4]]
	TyrianText.text_shade(self, x, y + 10, info, 15, 2 if sel else 0, TyrianText.FULL_SHADE)


## Empty-folder screen, scroll indicators, help.
func _draw_footer() -> void:
	if _names.is_empty():
		TyrianText.text_shade(self, 60, 80, TyrianText.L("replay_no_files"), 15, 0, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, 60, 100, TyrianText.L("replay_no_files_desc"), 15, 0, TyrianText.FULL_SHADE)
	else:
		var top: int = menus.current().top
		if top > 0:
			_draw_up_indicator(290, 30)
		if top + ITEMS_PER_PAGE < _names.size():
			TyrianText.text_shade(self, 290, 180, "v", 15, 2, TyrianText.FULL_SHADE)
	if not _status.is_empty() and Time.get_ticks_msec() < _status_until:
		TyrianText.text_shade(self, 20, 192, _status, 4, 4, TyrianText.FULL_SHADE)
	else:
		TyrianText.text_shade(self, 20, 192, TyrianText.L("replay_controls"), 15, 0, TyrianText.FULL_SHADE)


## The "^" glyph does not exist in the fonts (fonthand.c:font_ascii['^'] = -1,
## so the C code displays nothing): upward chevron drawn pixel by pixel,
## with the drop shadow of the "v" (1 pixel down and to the right).
func _draw_up_indicator(x: int, y: int) -> void:
	for pass_index in 2:
		var offset := 1 - pass_index
		var c := 0 if pass_index == 0 else UP_INDICATOR_COLOR
		for row in 3:
			vga.set_pixel(VGA_SCREEN, x + 2 - row + offset, y + row + offset, c)
			vga.set_pixel(VGA_SCREEN, x + 2 + row + offset, y + row + offset, c)


## Enter on a file (replay_browser, INPUT_MENU_CONFIRM).
func _activate(sel: int) -> void:
	if _names.is_empty():
		menus.draw()
		return
	var path := _replay_dir().path_join(_names[sel] + ".rep")
	var refusal: String = ["", "replay_invalid", "replay_empty"][clampi(TyrianVga.replay_check(Game.data_dir, path), 0, 2)]
	if not refusal.is_empty():
		Sfx.play(Sfx.SPRING)
		_status = TyrianText.L(refusal)
		_status_until = Time.get_ticks_msec() + 2500
		menus.draw()
		return
	idle = false
	_busy = true
	Game.start_replay(path)


## Navigation (Up/Down with wrap-around, Esc): shared engine.
func handle_action(key: StringName) -> void:
	if _busy or not idle:
		return
	menus.handle_action(key)
