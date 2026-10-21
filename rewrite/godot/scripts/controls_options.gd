## Controls screen (block 14, ADR 0051): keys and gamepad buttons
## of players 1 and 2, saved in userdata/controls.cfg.
##
## Equivalent of `keybindings_menu.c` (C): Up/Down picks a command,
## Enter waits for the new key (or gamepad button; the cancel
## key, Escape by default, cancels), Tab switches player. The last
## two rows (F1 help, capture cancel) are shared by both
## players and only take a keyboard key, Delete restores the defaults (config.c:defaultKeySettings),
## Escape saves and returns to the title. Opened by F9 (effects.gd); during
## a level, the screen is laid over the suspended game, which resumes
## as is on closing (Effects.open_options / close_options).
class_name ControlsOptions
extends Node2D

const SCENE_PATH := "res://scenes/controls_options.tscn"
## Screen returned to by Escape outside the overlay: title (F9), or Setup when
## the screen is opened by Setup > Key Bindings (back to the title on exit).
static var return_scene := "res://scenes/title.tscn"

## First row and row spacing: eleven rows (eight commands, three shared
## actions) fit between the title and the refusal message (y 154).
const ROW_Y := 30
const ROW_STEP := 11
## Visible rows: eight player commands and thirteen shared keys do not
## fit together between the title and the refusal message; the window
## follows the selected row.
const ROW_VISIBLE := 10

var selected := 0
## First displayed row.
var top := 0
var player := 1
## True: the next key / button replaces the binding.
var waiting := false
## Key refused for a shared action (message until the next selection).
var refused := false
## Backdrop and 8-bit framebuffer of the screen (TyrianScreen.menu_backdrop).
var screen: TyrianScreen


func _ready() -> void:
	# Rewrite cursor (screen absent from the 8-bit C).
	add_child(MouseCursor.new())
	screen = TyrianScreen.menu_backdrop(self)
	if Effects.level_paused():
		screen.modulate.a = 0.8  # suspended game visible underneath
	# menu_tree.c:run_menu (keybindings_menu.c): with the mouse active, the row under the
	# pointer is selected on opening, without movement, with the hover click.
	var i := _row_under(TyrianMouse.position)
	if i >= 0 and not TyrianMouse.inactive:
		selected = i
		Sfx.play(Sfx.CLICK)
	_redraw()


## Row under point `p` (320x200 pixels) among the visible rows, -1 otherwise.
func _row_under(p: Vector2i) -> int:
	var i := (p.y - ROW_Y) / ROW_STEP + top
	if p.x >= 20 and p.x < 300 and p.y >= ROW_Y and i >= top and i < mini(top + ROW_VISIBLE, _row_count()):
		return i
	return -1


## Drawing into the 8-bit framebuffer on image 2 and its palette, C
## colors: header like menu_tree.c:run_menu (draw_font_hv_shadow 15, -4, shadow
## 2); rows in small font like the C lists in that font
## (mod_manager.c:1549: JE_textShade 15, 4 on the selected row, 2 otherwise,
## FULL_SHADE); refusal in tint 7 (menu_tree.c:set_status); help
## JE_textShade(15, 4, PART_SHADE). Rewrite layout (small
## font, 11 px spacing, two players).
func _redraw() -> void:
	if screen == null:
		return
	screen.begin_menu_frame()
	TyrianText.draw_font_hv_shadow(
		screen,
		160,
		12,
		TyrianText.L("gd_controls_title") % player,
		TyrianScreen.NORMAL_FONT,
		TyrianScreen.CENTERED,
		15,
		-4,
		false,
		2
	)
	var cmds := TyrianControls.COMMANDS
	_scroll_to_selection()
	for i in range(top, mini(top + ROW_VISIBLE, _row_count())):
		var bright := 4 if i == selected else 2
		var label: String
		var value: String
		if i < cmds.size():
			label = cmds[i][2]
			value = TyrianControls.describe(player, cmds[i][0])
		else:
			var g: Array = TyrianControls.GLOBAL_COMMANDS[i - cmds.size()]
			label = g[1]
			value = TyrianControls.key_name(TyrianControls.global_key(g[0]))
		if waiting and i == selected:
			value = "..."
		var y := ROW_Y + (i - top) * ROW_STEP
		TyrianText.text_shade(screen, 20, y, TyrianText.L(label), 15, bright, TyrianText.FULL_SHADE)
		TyrianText.text_shade(screen, 140, y, value, 15, bright, TyrianText.FULL_SHADE)
	if refused:
		TyrianText.text_shade(screen, 20, 154, TyrianText.L("gd_controls_in_use"), 7, 4, TyrianText.PART_SHADE)
	TyrianText.text_shade(screen, 20, 165, TyrianText.L("gd_controls_keys"), 15, 4, TyrianText.PART_SHADE)
	TyrianText.text_shade(screen, 20, 180, TyrianText.L("gd_controls_exit"), 15, 4, TyrianText.PART_SHADE)
	screen.present()


## Keeps the selected row inside the ROW_VISIBLE-row window.
func _scroll_to_selection() -> void:
	top = clampi(top, maxi(selected - ROW_VISIBLE + 1, 0), selected)
	top = clampi(top, 0, maxi(_row_count() - ROW_VISIBLE, 0))


func _row_count() -> int:
	return TyrianControls.COMMANDS.size() + TyrianControls.GLOBAL_COMMANDS.size()


## Screen pointer (MouseCursor): hidden while a key is being captured,
## presented without JE_mouseStart (menu_tree.c:828-862).
func pointer_shown() -> bool:
	return not waiting


func _command() -> String:
	return TyrianControls.COMMANDS[selected][0]


## Shared action of the selected row, &"" on a player command.
func _global_action() -> StringName:
	var i := selected - TyrianControls.COMMANDS.size()
	return TyrianControls.GLOBAL_COMMANDS[i][0] if i >= 0 else &""


func _unhandled_input(event: InputEvent) -> void:
	if waiting:
		# menu_tree_capture_scancode only listens to keys: the mouse is
		# ignored while waiting (M01), except the right click which cancels like
		# Escape (the gamepad capture help says "a click cancels").
		if event is InputEventMouse:
			if TyrianMouse.is_button_press(event) and event.button_index == MOUSE_BUTTON_RIGHT:
				waiting = false
				_redraw()
		else:
			capture(event)
		return
	if event is InputEventMouse:
		handle_mouse(event)
		return
	var action := TyrianControls.menu_action(event)
	if action != &"":
		handle_action(action)


## Mouse (keybindings_menu.c went through menu_tree: hover = selected row,
## left click = Enter (waiting for a key), right click = Escape, wheel =
## one row like the arrows).
func handle_mouse(event: InputEvent) -> void:
	if TyrianMouse.is_wheel(event):
		return
	var i := _row_under(Vector2i(event.position.floor()))
	var inside := i >= 0
	if event is InputEventMouseMotion:
		if inside and not TyrianMouse.inactive and event.relative != Vector2.ZERO and i != selected:
			selected = i
			refused = false
			Sfx.play(Sfx.CLICK)  # menu_tree.c:580-584: hover = S_CLICK
			_redraw()
		return
	if not TyrianMouse.is_button_press(event):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		handle_action(TyrianControls.BACK)
		return
	if not inside:
		return
	selected = i
	handle_action(TyrianControls.ACCEPT)


## Capture of the new binding (raw event: the key itself is
## requested). Any key or button ends the capture: it cannot
## stay stuck. The cancel key cancels; a gamepad button on a
## shared row (keyboard only) also cancels.
func capture(event: InputEvent) -> void:
	var g := _global_action()
	if event is InputEventKey and event.pressed and not event.echo:
		if TyrianControls.global_pressed(event, TyrianControls.CAPTURE_CANCEL):
			pass
		elif g != &"":
			refused = not TyrianControls.rebind_global(g, event.physical_keycode)
		elif TyrianControls.shortcut_key_in_use(event.physical_keycode):
			refused = true  # key of a shortcut (help, pause, achievements, capture...) or reserved
		else:
			TyrianControls.rebind_key(player, _command(), event.physical_keycode)
	elif event is InputEventJoypadButton and event.pressed:
		if g == &"":
			TyrianControls.rebind_button(player, _command(), event.button_index)
	else:
		return
	waiting = false
	_redraw()


## Menu action (ADR 0120). The capture of a new binding, above,
## reads the raw event: the key itself is requested.
func handle_action(key: StringName) -> void:
	var n := _row_count()
	refused = false
	match key:
		TyrianControls.UP:
			# menu_tree.c:488: Up / Down silent in MENU_TREE menus.
			selected = (selected + n - 1) % n
		TyrianControls.DOWN:
			selected = (selected + 1) % n
		TyrianControls.TOGGLE, TyrianControls.LEFT, TyrianControls.RIGHT:
			player = 3 - player
			Sfx.play(Sfx.CURSOR)
		TyrianControls.ACCEPT:
			waiting = true
			Sfx.play(Sfx.SELECT)
		TyrianControls.RESET:
			TyrianControls.reset_defaults()
		TyrianControls.BACK:
			TyrianControls.save_config()
			# Opened over a level (F9): the game resumes; otherwise title.
			if not Effects.close_options():
				var back := return_scene
				return_scene = Game.SCENE_TITLE
				Game.goto_scene(back)
			return
		_:
			return
	_redraw()
