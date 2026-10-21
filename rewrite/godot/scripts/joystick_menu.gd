## Setup > Joystick (joystick_menu.c:joystick_menu_run): settings of each
## player's gamepad, rendered in the Setup menu (MenuEngine, MENU_TREE
## style) as in the C: gamepad choice, analog, sensitivity,
## threshold, the ten assignments (two slots each, captured by
## joystick.c:detect_joystick_assignment), reset.
##
## Gamepads: that of player 1 (device 0) and that of player 2
## (device 1), TyrianControls.PAD_DEVICE; they can be configured even when
## unplugged (the C only showed "no gamepad"; the settings
## apply here as soon as it is plugged in). Menu and Pause are shared by
## both gamepads (actions tyrian_level_exit and tyrian_pause).
## Everything is saved in userdata/controls.cfg (TyrianControls.save_config).
class_name JoystickMenu
extends RefCounted

const Kind := MenuEngine.Kind

## Setup screen (drawing of the rows and of the capture prompt).
var screen: TyrianScreen
## Player whose gamepad is being configured (joystick_menu.c:s_cur + 1).
var player := 1
## Assignment row being captured (index of TyrianControls.PAD_ROWS), -1 otherwise.
var capture_row := -1


func _init(setup_screen: TyrianScreen) -> void:
	screen = setup_screen


func capturing() -> bool:
	return capture_row >= 0


## joystick_menu.c:joystick_items.
func items() -> Array:
	var out := [
		MenuEngine.item("joym_device", "joym_device_desc", Kind.PICKER, {
			"get": func() -> int: return player - 1,
			"set": func(i: int) -> void: player = i + 1,
			"count": TyrianControls.PAD_DEVICE.size(),
			"labels": [device_label(1), device_label(2)],
		}),
		MenuEngine.item("joym_analog", "joym_analog_desc", Kind.TOGGLE, {
			"get": func() -> bool: return TyrianControls.analog_enabled[player],
			"set": func(v: bool) -> void:
				TyrianControls.analog_enabled[player] = v
				TyrianControls.save_config(),
		}),
		_range("joym_sensitivity", "joym_sens_desc", TyrianControls.analog_sensitivity),
		_range("joym_threshold", "joym_thresh_desc", TyrianControls.analog_threshold),
	]
	for row in TyrianControls.PAD_ROWS.size():
		out.append(MenuEngine.item("joym_" + TyrianControls.PAD_ROWS[row], "joym_assign_desc", Kind.CUSTOM, {
			"row": row,
			"draw": _draw_assign,
			"activate": func(it: Dictionary) -> void: capture_row = it.row,
			"adjust": func(_it: Dictionary, _dir: int) -> void: pass,
		}))
	out.append(MenuEngine.item("joym_reset", "joym_reset_desc", Kind.ACTION, {
		"action": func() -> void:
			TyrianControls.reset_pad(player)
			TyrianControls.save_config(),
	}))
	out.append(MenuEngine.item("setup_done", "setup_done_back", Kind.BACK))
	return out


## joystick_menu.c:dev_label: "n - name" (name truncated to 24), "n" without a name.
static func device_label(p: int) -> String:
	var device: int = TyrianControls.PAD_DEVICE[p]
	if device in Input.get_connected_joypads():
		return "%d - %s" % [p, Input.get_joy_name(device).left(24)]
	return str(p)


## MENU_ITEM_RANGE (menu_tree.c) 0..10 step 1: Left / Right clamped,
## S_CLICK; value on the right like render_default.
func _range(label: String, help: String, values: Dictionary) -> Dictionary:
	return MenuEngine.item(label, help, Kind.CUSTOM, {
		"draw": func(it: Dictionary, x: int, y: int, selected: bool) -> void:
			_draw_row(it.label, str(values[player]), x, y, selected),
		"activate": func(_it: Dictionary) -> void: pass,
		"adjust": func(_it: Dictionary, dir: int) -> void:
			values[player] = clampi(int(values[player]) + dir, 0, TyrianControls.ANALOG_SETTING_MAX)
			Sfx.play(Sfx.CLICK)
			TyrianControls.save_config(),
	})


## menu_tree.c:render_default: label and value at x + 160, -3 / +2 selected.
func _draw_row(label: String, value: String, x: int, y: int, selected: bool) -> void:
	var v := -3 + (2 if selected else 0)
	TyrianText.draw_font_hv_shadow(screen, x, y, TyrianText.L(label), screen.NORMAL_FONT, screen.LEFT, 15, v, false, 2)
	if value != "":
		TyrianText.draw_font_hv_shadow(screen, x + MenuEngine.VALUE_X_OFFSET, y, value, screen.NORMAL_FONT, screen.LEFT,
				15, v, false, 2)


## joystick_menu.c:assign_draw.
func _draw_assign(it: Dictionary, x: int, y: int, selected: bool) -> void:
	_draw_row(it.label, TyrianControls.pad_row_text(player, it.row), x, y, selected)


## Maximum width of the prompt text, in the 260 px box.
const CAPTURE_TEXT_WIDTH := 244
const CAPTURE_LINE_HEIGHT := 12


## Cuts the text at spaces so that each line fits within max_width.
static func wrap_lines(text: String, max_width: int, font: int) -> PackedStringArray:
	var lines := PackedStringArray()
	var current := ""
	for word in text.split(" ", false):
		var candidate: String = word if current == "" else current + " " + word
		if current != "" and TyrianText.width(candidate, font) > max_width:
			lines.append(current)
			current = word
		else:
			current = candidate
	if current != "":
		lines.append(current)
	return lines


## joystick_menu.c:assign_act: prompt over the menu during the capture.
## The localized text may exceed the box: it then wraps onto several
## lines and the box grows, centered on the same point.
func decorate() -> void:
	if not capturing():
		return
	var lines := wrap_lines(TyrianText.L("joym_capture"), CAPTURE_TEXT_WIDTH, screen.NORMAL_FONT)
	var extra := (lines.size() - 1) * CAPTURE_LINE_HEIGHT
	var top := 82 - extra / 2
	screen.vga.fill_rectangle_xy(screen.VGA_SCREEN, 30, top, 30 + 260 - 1, top + 26 + extra - 1, 224)
	screen.vga.rectangle(screen.VGA_SCREEN, 28, top - 2, 291, top + 27 + extra, 248)
	for i in lines.size():
		TyrianText.draw_font_hv_shadow(screen, 160, top + 8 + i * CAPTURE_LINE_HEIGHT, lines[i], screen.NORMAL_FONT,
				screen.CENTERED, 15, -2, false, 2)


## Event received during the capture: a button or an axis pushed beyond
## two thirds is assigned (joystick_assign_toggle, S_CLICK); a key
## or a click cancels (joystick_menu.c:assign_act, keystroke purged). True if the
## capture is over (menu to redraw).
func capture(event: InputEvent) -> bool:
	var detected := TyrianControls.pad_capture_event(event)
	if detected != null:
		TyrianControls.pad_assign_toggle(player, capture_row, detected)
		TyrianControls.save_config()
		Sfx.play(Sfx.CLICK)
		capture_row = -1
		return true
	if (event is InputEventKey and event.pressed and not event.echo) or TyrianMouse.is_button_press(event):
		capture_row = -1
		return true
	return false
