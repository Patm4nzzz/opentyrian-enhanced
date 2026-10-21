## "Mods" menu (equivalent of Setup -> Mods in the C version, mod_manager.c:
## mod_manager_draw_menu): list of installed mods, activation with Enter,
## hot reload on exit (Esc). Preferences are kept
## in userdata/mods.cfg; the mods' mod.json is never rewritten.
##
## Below the list, two rows specific to the rewrite: open the player's mods
## folder (userdata/mods) and install a mod from a .zip, chosen through
## a dialog box or dropped on the window (Mods.install_zip).
##
## Navigation: shared menu engine (MenuEngine, ADR 0121), one CUSTOM row
## per mod; the drawing (node-based screen with TyrianText) is provided by
## the `render` option.
extends Node2D

const ROWS := 9
## Detail area below the list: usable width and number of text lines.
const DETAIL_W := 296
const DETAIL_LINES := 3
## Right edge of the text of a list row.
const ROW_RIGHT := 308
## Action rows after the mods: [label, description].
const ACTION_ROWS := [
	["gd_mods_open_folder", "gd_mods_folder_desc"],
	["gd_mods_install", "gd_mods_install_desc"],
]

var mods: Array = []
## Selected row.
var index: int:
	get:
		return menus.selection() if not menus.stack.is_empty() else 0
var menus := MenuEngine.new(self)
## Result of the last installation (translated text) and its state.
var install_message := ""
var install_failed := false
var _changed := false
## Last toggle / reload sound played (checked by the tests).
var last_sfx := -1
## Backdrop and 8-bit framebuffer of the screen (TyrianScreen.menu_backdrop).
var screen: TyrianScreen
## "Install a mod" dialog, open, null otherwise: it never outlives the screen.
var _dialog: FileDialog


func _ready() -> void:
	screen = TyrianScreen.menu_backdrop(self)
	get_window().files_dropped.connect(_on_files_dropped)
	_open()


func _exit_tree() -> void:
	_close_dialog()
	if get_window().files_dropped.is_connected(_on_files_dropped):
		get_window().files_dropped.disconnect(_on_files_dropped)


## (Re)opens the list on selection `sel`.
func _open(sel := 0) -> void:
	mods = Mods.list()
	var rows := []
	for m in mods:
		rows.append(
			MenuEngine.item(
				"",
				"",
				MenuEngine.Kind.CUSTOM,
				{
					"activate": func(_it: Dictionary) -> void: toggle(),
					"adjust": func(_it: Dictionary, _dir: int) -> void: pass,
				}
			)
		)
	rows.append(
		MenuEngine.item(
			ACTION_ROWS[0][0],
			ACTION_ROWS[0][1],
			MenuEngine.Kind.CUSTOM,
			{
				"activate": func(_it: Dictionary) -> void: open_folder(),
				"adjust": func(_it: Dictionary, _dir: int) -> void: pass,
			}
		)
	)
	rows.append(
		MenuEngine.item(
			ACTION_ROWS[1][0],
			ACTION_ROWS[1][1],
			MenuEngine.Kind.CUSTOM,
			{
				"activate": func(_it: Dictionary) -> void: choose_zip(),
				"adjust": func(_it: Dictionary, _dir: int) -> void: pass,
			}
		)
	)
	menus.stack.clear()
	(
		menus
		. open(
			"gd_mods_title",
			rows,
			{
				"sel": clampi(sel, 0, rows.size() - 1),
				"render": _redraw,
				"move_sfx": Sfx.CURSOR,
				"activate_sfx": -1,
				# mod_manager.c:1667-1683: Up / Down bounded (no wrap-around); only
				# INPUT_MENU_CONFIRM (Enter) toggles the mod, not Space.
				"wrap": false,
				"accept": [TyrianControls.ACCEPT],
				"on_back": leave,
			}
		)
	)


func _row_count() -> int:
	return mods.size() + ACTION_ROWS.size()


## Drawing of the screen in the 8-bit framebuffer, with the hues and
## brightness values of mod_manager.c:mod_manager_menu (JE_textShade FULL_SHADE,
## TINY font) on image 2 and its palette: title 15/4, name 15/4 selected and
## 15/2 otherwise, status ON 10, OFF 7, ERR 4, "by" card 15/4, description
## 14/2, error 4/2, help 15/4. The layout (columns, action rows,
## multi-line card) remains the rewrite's.
func _redraw() -> void:
	if screen == null:
		return
	screen.begin_menu_frame()
	_text(TyrianText.L("mods_header"), 160, 5, 15, 4, TyrianText.Align.CENTER)
	if not Mods.available():
		_text(TyrianText.L("gd_mods_unavailable"), 160, 60, 4, 2, TyrianText.Align.CENTER)
		screen.present()
		return
	var first := clampi(index - ROWS + 1, 0, maxi(0, _row_count() - ROWS))
	for i in range(first, mini(first + ROWS, _row_count())):
		var y := 28 + (i - first) * 12
		var sel := i == index
		if i >= mods.size():
			_draw_action_row(i - mods.size(), y, sel)
			continue
		var m: Dictionary = mods[i]
		var failed := not String(m["error"]).is_empty()
		var state := TyrianText.L("mods_err") if failed else TyrianText.L("mods_on" if m["enabled"] else "mods_off")
		# mod_manager.c:1558-1563: ERR hue 4, ON 10, OFF 7.
		var hue := 4 if failed else (10 if m["enabled"] else 7)
		# OFF is the only hint to activate: lighter, selected or not.
		var tone := 6 if not (m["enabled"] or failed) else (4 if sel else 2)
		_text(state, 12, y, hue, tone)
		var name := String(m["name"]).left(34)
		_text(name, 40, y, 15, 4 if sel else 2)
		# Mod in error: the reason follows the name on its row (cut to the room
		# left), so that "ERR" explains itself even without selecting the mod.
		if failed:
			_text_after(name, 40, y, _error_reason(m), 4, 2)
	if mods.is_empty():
		_text(TyrianText.L("gd_mods_none"), 160, 60, 15, 2, TyrianText.Align.CENTER)
	var y0 := 28 + ROWS * 12 + 4
	if index >= mods.size():
		_draw_action_detail(index - mods.size(), y0)
	else:
		_draw_mod_detail(mods[index], y0)
	_text(TyrianText.L("gd_mods_keys"), 160, 190, 15, 4, TyrianText.Align.CENTER)
	screen.present()


## Action row; the install row carries the result of the last
## installation, like a mod's error reason on its row.
func _draw_action_row(a: int, y: int, sel: bool) -> void:
	var label := TyrianText.L(ACTION_ROWS[a][0])
	_text(label, 12, y, 15, 4 if sel else 2)
	if a == 1 and not install_message.is_empty():
		_text_after(label, 12, y, install_message, _install_hue(), 2)


func _draw_action_detail(a: int, y0: int) -> void:
	var text := TyrianText.L(ACTION_ROWS[a][1])
	if a == 0:
		text = text % ProjectSettings.globalize_path(Mods.USER_MODS)
	# One more line than for a mod (no "name v... by..." line): the
	# full folder path fits.
	var lines: Array = []
	for l in wrap_lines(text, DETAIL_W, DETAIL_LINES + 1):
		lines.append([l, 14, 2])
	if a == 1 and not install_message.is_empty():
		for l in wrap_lines(install_message, DETAIL_W, maxi(1, DETAIL_LINES + 1 - lines.size())):
			lines.append([l, _install_hue(), 2])
	for k in lines.size():
		_text(lines[k][0], 12, y0 + 11 * k, lines[k][1], lines[k][2])


func _draw_mod_detail(m: Dictionary, y0: int) -> void:
	# mod_manager.c:1609-1620: "by ..." 15/4, description 14/2, error 4/2.
	# Cut to the screen like the description: a long author name (Alien Ship
	# Pack) wrapped around the screen edge and was drawn over the left margin.
	var by := TyrianText.L("gd_mods_by") % [String(m["name"]), m["version"], m["author"]]
	if TyrianText.width(by, TyrianText.FontKind.SMALL) > DETAIL_W:
		by = wrap_lines(by, DETAIL_W, 1)[0]
	_text(by, 12, y0, 15, 4)
	# Description, then error reason, on whole lines (word wrap;
	# ellipsis if the text exceeds the room).
	var lines: Array = []
	for l in wrap_lines(String(m["description"]), DETAIL_W, 1):
		lines.append([l, 14, 2])
	var reason := _error_reason(m)
	if not reason.is_empty():
		for l in wrap_lines(reason, DETAIL_W, DETAIL_LINES - lines.size()):
			lines.append([l, 4, 2])
	elif not m["enabled"]:
		lines.append([TyrianText.L("gd_mods_disabled"), 7, 6])
	else:
		lines.append(
			[
				(
					TyrianText.L("gd_mods_counts")
					% [m["weapons"], m["ships"], m["sidekicks"], int(m["enemies"]) + int(m["new_enemies"]), m["levels"]]
				),
				15,
				1
			]
		)
	for k in lines.size():
		_text(lines[k][0], 12, y0 + 11 * (k + 1), lines[k][1], lines[k][2])


## Hue of the install result: ERR (4) on failure, ON (10) otherwise.
func _install_hue() -> int:
	return 4 if install_failed else 10


## Short text after `before` (drawn at x) on the same line, cut to the
## room left.
func _text_after(before: String, x: int, y: int, text: String, hue: int, brightness: int) -> void:
	var x2 := x + TyrianText.width(before, TyrianText.FontKind.SMALL) + 6
	var short: Array = wrap_lines(text, ROW_RIGHT - x2, 1)
	if not short.is_empty():
		_text(short[0], x2, y, hue, brightness)


## Reason displayed for a mod in error ("" otherwise). Load error:
## translated message (key mod_err_*). Errors in the mod's content: their
## count, the detail (for the mod author) being given by the mod
## verification tool, not by the game.
func _error_reason(m: Dictionary) -> String:
	var errors := Mods.diagnostics().filter(
		func(d: Dictionary) -> bool: return d["severity"] == "error" and d["folder"] == m["folder"]
	)
	if not String(m.get("error_key", "")).is_empty():
		var args: Array = Array(String(m.get("error_args", "")).split("\t", false))
		return TyrianText.L(m["error_key"]) % args if not args.is_empty() else TyrianText.L(m["error_key"])
	if not errors.is_empty() or not String(m["error"]).is_empty():
		return TyrianText.L("gd_mods_errors") % maxi(errors.size(), 1)
	return ""


## Splits `text` at word boundaries into lines of at most `width` px (small font), in
## at most `max_lines` lines; the last one ends with "..." if the text
## does not fit. A word wider than a line is split at the letter.
static func wrap_lines(text: String, width: int, max_lines: int) -> Array:
	var out: Array = []
	var line := ""
	var words := text.strip_edges().split(" ", false)
	var i := 0
	while i < words.size() and out.size() < max_lines:
		var w: String = words[i]
		var candidate := w if line.is_empty() else line + " " + w
		if TyrianText.width(candidate, TyrianText.FontKind.SMALL) <= width:
			line = candidate
			i += 1
			continue
		if line.is_empty():
			# Single word too wide: split at the letter.
			var n := w.length()
			while n > 1 and TyrianText.width(w.left(n), TyrianText.FontKind.SMALL) > width:
				n -= 1
			out.append(w.left(n))
			words[i] = w.substr(n)
			continue
		out.append(line)
		line = ""
	if out.size() < max_lines and not line.is_empty():
		out.append(line)
		line = ""
	if (i < words.size() or not line.is_empty()) and not out.is_empty():
		var last: String = out[-1]
		while not last.is_empty() and TyrianText.width(last + "...", TyrianText.FontKind.SMALL) > width:
			last = last.left(last.length() - 1)
		out[-1] = last.strip_edges() + "..."
	return out


## JE_textShade(VGAScreen, x, y, s, hue, brightness, FULL_SHADE), x taken
## as the center of the text if `align` is CENTER.
func _text(s: String, x: int, y: int, hue: int, brightness: int, align := TyrianText.Align.LEFT) -> void:
	if align == TyrianText.Align.CENTER:
		x -= TyrianText.width(s, TyrianText.FontKind.SMALL) / 2
	TyrianText.text_shade(screen, x, y, s, hue, brightness, TyrianText.FULL_SHADE)


func _unhandled_input(event: InputEvent) -> void:
	# mod_manager.c has no mouse access: no hover, no click, no cursor.
	if event is InputEventMouse:
		return
	# "Install a mod" dialog open: the screen no longer reacts. On Windows,
	# the native dialog cannot be closed programmatically (hide() leaves it displayed);
	# leaving the screen therefore left it open behind the game. The player
	# closes it themselves (Open or Cancel), then the screen takes over again.
	if dialog_open():
		get_viewport().set_input_as_handled()
		return
	# mod_manager.c: R forces a full reload without toggling a mod.
	if event is InputEventKey and event.pressed and not event.echo and event.physical_keycode == KEY_R:
		reload_now()
		return
	var action := TyrianControls.menu_action(event)
	if action != &"":
		handle_action(action)


## Menu action (shared InputMap, ADR 0120), handled by the shared engine.
## Without GDExtension, only exiting remains possible.
func handle_action(action: StringName) -> void:
	if not Mods.available() and action != TyrianControls.BACK:
		return
	menus.handle_action(action)


## mod_manager.c, R key: data reload without waiting for the menu
## exit (its S_ITEM sound). The mods read are recounted and the list redrawn.
func reload_now() -> void:
	if not Mods.available():
		return
	Mods.reload()
	_changed = false
	last_sfx = Sfx.ITEM
	Sfx.play(Sfx.ITEM)
	_open(index)


## Enter / Space on a mod: activation, S_CLICK sound (mod_manager.c: a
## mod in error can be toggled too, it stays in error as long as its mod.json is).
func toggle() -> void:
	if index >= mods.size():
		return
	last_sfx = Sfx.CLICK
	Sfx.play(Sfx.CLICK)
	var m: Dictionary = mods[index]
	m["enabled"] = not m["enabled"]
	Mods.set_enabled(m["folder"], m["enabled"])
	_changed = true
	# Card right away: the counters come from the resolution of the
	# active mods, to be redone after the toggle (without it: "ships 0" for a
	# mod that has just been activated, until the menu is reopened).
	Mods.reload()
	mods = Mods.list()


## Opens the player's mods folder in the system's file manager
## (created if missing).
func open_folder() -> void:
	Sfx.play(Sfx.SELECT)
	var dir := ProjectSettings.globalize_path(Mods.USER_MODS)
	DirAccess.make_dir_recursive_absolute(dir)
	OS.shell_open(dir)


## Dialog box for choosing a .zip (native when the system has one).
func choose_zip() -> void:
	Sfx.play(Sfx.SELECT)
	_close_dialog()
	var dialog := FileDialog.new()
	_dialog = dialog
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.filters = PackedStringArray(["*.zip"])
	dialog.use_native_dialog = true
	dialog.title = TyrianText.L("gd_mods_install")
	dialog.current_dir = OS.get_system_dir(OS.SYSTEM_DIR_DOWNLOADS)
	dialog.file_selected.connect(_on_zip_chosen)
	dialog.canceled.connect(_on_dialog_done.bind(dialog))
	dialog.file_selected.connect(func(_p: String) -> void: _on_dialog_done(dialog))
	dialog.visibility_changed.connect(
		func() -> void:
			if not dialog.visible:
				_on_dialog_done(dialog)
	)
	add_child(dialog)
	dialog.popup_centered_ratio(0.9)


## True as long as the "Install a mod" dialog is waiting for the player's choice.
func dialog_open() -> bool:
	return _dialog != null and is_instance_valid(_dialog)


## The dialog has handed control back (choice, cancel or close): we free it.
func _on_dialog_done(dialog: FileDialog) -> void:
	if _dialog == dialog:
		_dialog = null
	if is_instance_valid(dialog) and not dialog.is_queued_for_deletion():
		dialog.queue_free()


## Closes and frees the .zip choice dialog (leaving the Mods screen, new call).
## The system's native dialog is a separate window: it is closed explicitly.
## On Windows, hide() does not close the native dialog (the system keeps it): we
## therefore also cut the link with the screen, so that a choice made later in
## that still-open dialog installs nothing outside the Mods screen.
func _close_dialog() -> void:
	var dialog := _dialog
	_dialog = null
	if dialog != null and is_instance_valid(dialog):
		if dialog.file_selected.is_connected(_on_zip_chosen):
			dialog.file_selected.disconnect(_on_zip_chosen)
		dialog.hide()
		if not dialog.is_queued_for_deletion():
			dialog.queue_free()


## The dialog hides before emitting file_selected: _dialog is already cleared by
## then. The link cut by _close_dialog is enough to ignore a late choice.
func _on_zip_chosen(path: String) -> void:
	if is_inside_tree():
		install(path)


func _on_files_dropped(files: PackedStringArray) -> void:
	for f in files:
		install(f)


## Installs a .zip into userdata/mods, reloads the mods and displays the
## result on the (selected) install row.
func install(zip_path: String) -> void:
	var r: Dictionary = Mods.install_zip(zip_path, ProjectSettings.globalize_path(Mods.USER_MODS))
	install_failed = not String(r["error"]).is_empty()
	if install_failed:
		var args: Array = r.get("args", [])
		install_message = TyrianText.L(r["error"]) % args if not args.is_empty() else TyrianText.L(r["error"])
	else:
		install_message = TyrianText.L("gd_mods_installed") % r["folder"]
		Mods.reload()
		_changed = false
	_open(Mods.list().size() + 1)


## Hot reload when leaving the menu (mod_manager.c: "hot-reloads
## all item data when you leave the menu").
const MODS_ROW := 5


func leave() -> void:
	_close_dialog()
	if _changed:
		Mods.reload()
	# setup_menu.c:action_mods hands control back to the Setup menu, on its Mods row.
	preload("res://scripts/setup.gd").entry_row = MODS_ROW
	Game.goto_scene(Game.SCENE_SETUP)
