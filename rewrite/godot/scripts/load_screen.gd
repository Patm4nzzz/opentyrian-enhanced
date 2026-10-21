## "Load Game" screen of the title (mainint.c:JE_loadScreen), rendered frame by
## frame on TyrianScreen (ADR 0020).
##
## Two pages (one player, two players) of 11 slots + "Exit to Main
## Menu". Slots of the native save (Game.campaign_slots, ADR
## 0129: indices 0..10 = 1 player, 11..21 = 2 players, like config.c).
## Without a save (screenshot case), all "EMPTY SLOT".
## Unreadable slot: shown as "Unreadable" and refused. Enter on a
## full slot: Game.load_campaign_slot.
extends TyrianScreen

const SCENE_TITLE := "res://scenes/title.tscn"

const X_CENTER := 160
const Y_MENU_HEADER := 5
const X_NAME := 10
const X_LAST_LEVEL := 120
const X_EPISODE := 250
const Y_ITEMS := 30
const DY_ITEMS := 13
const X_LEFT_CONTROL := 83
const X_RIGHT_CONTROL := 213
const Y_CONTROLS := 179
const ITEMS := 12

var players_index := 0
var selected := 0
var idle := false
var _busy := true
## Slot list (shared engine, ADR 0121).
var menus := MenuEngine.new(self)
## 22 slots (playersIndex * 11 + i): Dictionary or null.
var _slots: Array = []


func _ready() -> void:
	_slots.resize(22)
	_slots.fill(null)
	# campaign_slots() returns 22 slots (0..10 = 1 player, 11..21 = 2 players).
	var all_slots: Array = Game.campaign_slots()
	for i in mini(all_slots.size(), 22):
		_slots[i] = all_slots[i]
	_run.call_deferred()


func _run() -> void:
	# `if (restart)`: background = image 2, bottom band cleared, in VGAScreen2.
	vga.load_pic(VGA_SCREEN2, 2, false)
	vga.fill_rectangle_xy(VGA_SCREEN2, 0, 192, 319, 199, 0)
	_draw()
	await fade_palette(10, 0, 255)
	present()
	idle = true
	_busy = false


## Rows and options of the list (shared engine, ROWS style, ADR 0121).
func _open_menu() -> void:
	var rows := []
	for i in ITEMS:
		rows.append(MenuEngine.item("", "", MenuEngine.Kind.CUSTOM, {"index": i, "draw": _draw_row}))
	menus.open("", rows, {
		"style": MenuEngine.Style.ROWS, "draw": false, "sel": selected,
		"row_x": X_NAME, "row_y": Y_ITEMS, "row_step": DY_ITEMS,
		# mainint.c:JE_loadScreen: wMenuItem = 300, hMenuItem = 8.
		"hit_w": 300, "row_h": 8,
		"move_sfx": Sfx.CURSOR, "redraw_on_other": true,
		"accept": [TyrianControls.SELECT, TyrianControls.ACCEPT],
		"header_draw": func() -> void:
			TyrianText.draw_font_hv_shadow(self, X_CENTER, Y_MENU_HEADER,
					TyrianText.L("miscText_%d" % (38 + players_index)), LARGE_FONT, CENTERED, 15, -3, false, 2),
		"decorate": _draw_controls,
		"shortcuts": {
			TyrianControls.LEFT: _switch_players,
			TyrianControls.RIGHT: _switch_players,
		},
		"on_activate": _choose,
		"on_back": func() -> void:
			Sfx.play(Sfx.SPRING)
			_leave(),
	})


## Body of the JE_loadScreen loop (without present()).
func _draw() -> void:
	if menus.stack.is_empty():
		_open_menu()
	menus.current().sel = selected
	menus.render()


## One row: save slot or "Exit to Main Menu".
func _draw_row(it: Dictionary, x: int, y: int, sel: bool) -> void:
	var i: int = it.index
	if i == ITEMS - 1:
		TyrianText.text_shade(self, x, y, TyrianText.L("miscText_33"), 13, 6 if sel else 2, TyrianText.FULL_SHADE)
		return
	var s = _slots[players_index * 11 + i]
	if s == null:
		# miscText[2] overridden by "misc_empty" (helptext.c).
		TyrianText.text_shade(self, x, y, TyrianText.L("misc_empty"), 13, 6 if sel else 0, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, X_LAST_LEVEL, y, "%s -----" % TyrianText.L("miscTextB_2"), 5, 6 if sel else 0,
				TyrianText.FULL_SHADE)
	elif str(s.get("error", "")) != "":
		TyrianText.text_shade(self, x, y, TyrianText.L("save_unreadable"), 13, 6 if sel else 0, TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, X_LAST_LEVEL, y, "%s -----" % TyrianText.L("miscTextB_2"), 5, 6 if sel else 0,
				TyrianText.FULL_SHADE)
	else:
		var b := 6 if sel else 2
		TyrianText.text_shade(self, x, y, str(s["name"]), 13, b, TyrianText.FULL_SHADE)
		var ln := str(s["last_level"])
		if ln == "Completed":
			ln = TyrianText.L("save_completed")
		TyrianText.text_shade(self, X_LAST_LEVEL, y, ("%s %s" % [TyrianText.L("miscTextB_2"), ln]).left(21), 5, b,
				TyrianText.FULL_SHADE)
		TyrianText.text_shade(self, X_EPISODE, y, "%s %d" % [TyrianText.L("miscTextB_1"), int(s["episode"])], 5, b,
				TyrianText.FULL_SHADE)


## Page arrows and help below the list.
func _draw_controls() -> void:
	if players_index > 0:
		vga.blit_sprite2(VGA_SCREEN, X_LEFT_CONTROL, Y_CONTROLS, "newsh1.shp", 279, 0, 0, true)
	if players_index < 1:
		vga.blit_sprite2(VGA_SCREEN, X_RIGHT_CONTROL, Y_CONTROLS, "newsh1.shp", 281, 0, 0, true)
	TyrianText.help_box(self, 103, 182, TyrianText.L("miscText_55"), 25, 15)


## Left / Right: one-player / two-player page.
func _switch_players() -> bool:
	Sfx.play(Sfx.CURSOR)
	players_index = 1 - players_index
	return true


## Mouse (mainint.c:JE_loadScreen): page arrows (x 83 / 213, width
## 24, y >= 179) on left click; otherwise hovering a row = selection, left
## click = Enter; right click = exit (S_SPRING).
func handle_mouse(event: InputEvent) -> void:
	if _busy or not idle:
		return
	TyrianMouse.note(event)
	var p: Vector2 = event.position
	if p.y >= Y_CONTROLS and not TyrianMouse.is_wheel(event):
		var left := players_index > 0 and p.x >= X_LEFT_CONTROL and p.x < X_LEFT_CONTROL + 24
		var right := players_index < 1 and p.x >= X_RIGHT_CONTROL and p.x < X_RIGHT_CONTROL + 24
		if left or right:
			if TyrianMouse.is_button_press(event) and event.button_index == MOUSE_BUTTON_LEFT:
				handle_action(TyrianControls.LEFT if left else TyrianControls.RIGHT)
			elif TyrianMouse.is_button_press(event) and event.button_index == MOUSE_BUTTON_RIGHT:
				menus.back()
			return
	menus.handle_mouse(event)
	if not menus.stack.is_empty():
		selected = menus.selection()


## `switch (lastkey_scan)` of JE_loadScreen: navigation by the engine.
func handle_action(key: StringName) -> void:
	if _busy or not idle:
		return
	menus.handle_action(key)
	selected = menus.selection()


## Enter on a row: exit, empty slot (click) or load.
func _choose(sel: int) -> void:
	selected = sel
	if selected == ITEMS - 1:
		Sfx.play(Sfx.SELECT)
		_leave()
		return
	var s = _slots[players_index * 11 + selected]
	if s == null or str(s.get("error", "")) != "":
		Sfx.play(23)  # S_CLINK
		if s != null:
			push_warning("Slot %d unreadable: %s" % [selected + 1, s["error"]])
	else:
		# players_index 0 = 1 player, 1 = 2 players (config.c:(slot-1) > 10).
		Sfx.play(Sfx.SELECT)
		idle = false
		_busy = true
		await fade_black(15)
		Game.load_campaign_slot(s)
		return
	menus.draw()


func _leave() -> void:
	idle = false
	_busy = true
	await fade_black(15)
	Game.goto_scene(SCENE_TITLE)  # `restart = true` in titleScreen
