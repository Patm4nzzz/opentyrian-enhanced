## In-game help (mainint.c:JE_inGameHelp, F1 by default: "Help" action
## of the controls screen): help panels over the frozen level image,
## closed by any key, click or button. The simulation does not
## run during help.
extends "res://scripts/frozen_level_screen.gd"

signal closed

const KEYS_X := 14
const KEYS_Y := 13
const KEYS_STEP := 10

var _closing := false
## Panel displayed: 1 = the C's help, 2 = keys and mouse (divergence no. 57).
var _panel := 1


func open_help(lvl: Node) -> bool:
	if not freeze(lvl):
		return false
	_draw_frame()
	if lvl.call("_two_players"):
		_hbox(20, 4, 36, 50, 3)
		vga.blit_sprite(VGA_SCREEN, 2, 21, OPTION_SHAPES, 43)  # weapons help
		_hbox(55, 20, 37, 40)
		vga.blit_sprite(VGA_SCREEN, 5, 36, OPTION_SHAPES, 41)  # sidekicks help
		_hbox(40, 43, 34, 44)
		vga.blit_sprite(VGA_SCREEN, 2, 79, OPTION_SHAPES, 42)  # shield / armor
		_hbox(54, 84, 35, 40)
		_hbox(5, 126, 38, 55)
		_hbox(5, 160, 39, 55)
	else:
		vga.blit_sprite(VGA_SCREEN, 15, 5, OPTION_SHAPES, 40)  # energy bar
		_hbox(40, 10, 31, 45)
		vga.blit_sprite(VGA_SCREEN, 5, 37, OPTION_SHAPES, 39)  # weapons
		_hbox(40, 40, 32, 44)
		_hbox(40, 60, 33, 44)
		vga.blit_sprite(VGA_SCREEN, 5, 98, OPTION_SHAPES, 41)  # sidekicks
		_hbox(40, 103, 34, 44)
		vga.blit_sprite(VGA_SCREEN, 2, 138, OPTION_SHAPES, 42)  # shield / armor
		_hbox(54, 143, 35, 40)
	_draw_press_key()
	present()
	idle = true
	return true


## Frame (Main Box) of the C's help.
func _draw_frame() -> void:
	vga.bar_shade(VGA_SCREEN, 1, 1, 262, 182)
	vga.bar_shade(VGA_SCREEN, 3, 3, 260, 180)
	vga.bar_shade(VGA_SCREEN, 5, 5, 258, 178)
	vga.bar_shade(VGA_SCREEN, 7, 7, 256, 176)
	vga.fill_rectangle_xy(VGA_SCREEN, 9, 9, 254, 174, 0)


## "press a key" in the dashboard text area.
func _draw_press_key() -> void:
	vga.blit_sprite(VGA_SCREEN, 16, 189, OPTION_SHAPES, 36)
	var text := TyrianText.L("miscText_4")
	TyrianText.out_text(self, 120 - TyrianText.width(text, SMALL_FONT) / 2 + 20, 190, text, 0, 4)


## Second panel: the keys and the mouse, one row each.
func _open_keys_panel() -> void:
	_panel = 2
	if not freeze(level):
		_closing = true
		closed.emit()
		return
	_draw_frame()
	var saved_height := TyrianText.vertical_height
	TyrianText.vertical_height = 8
	for i in TyrianControls.HELP_KEYS.size():
		var line: Array = TyrianControls.HELP_KEYS[i]
		TyrianText.help_box(self, KEYS_X, KEYS_Y + i * KEYS_STEP, TyrianControls.help_line(line[0], line[1]), 60, 5, 3)
	TyrianText.vertical_height = saved_height
	_draw_press_key()
	present()


## helptext.c:JE_HBox: helpTxt[messagenum - 1] (help_N keys, N starting from
## 0), color helpBoxColor (5, or 3 for the two-player header) and
## brightness helpBoxBrightness 3.
func _hbox(x: int, y: int, messagenum: int, boxwidth: int, color := 5) -> void:
	TyrianText.help_box(self, x, y, TyrianText.L("help_%d" % (messagenum - 1)), boxwidth, color, 3)


## "while (!(newkey || newmouse))": any key, any click, any button.
func _unhandled_input(event: InputEvent) -> void:
	if (
		(event is InputEventKey and event.pressed and not event.echo)
		or TyrianMouse.is_button_press(event)
		or (event is InputEventJoypadButton and event.pressed)
	):
		get_viewport().set_input_as_handled()
		handle_action(&"")


## Keys of the capture tool (handle_event) and of the keyboard.
func handle_action(_action: StringName) -> void:
	if _closing:
		return
	if _panel == 1:
		_open_keys_panel()
		return
	_closing = true
	closed.emit()
