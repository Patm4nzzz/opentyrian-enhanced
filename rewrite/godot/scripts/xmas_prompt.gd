## Christmas prompt (xmas.c:xmas_prompt), set by the title at launch in
## December (see Christmas.needs_prompt). Plain blue background (0x8F), 80 snowflakes
## falling (two thirds behind the text, the last third in front),
## two lines of text and "Yes" / "No". Choice: left / right
## move, Enter or Space confirm, Esc cancels; with the mouse, hover
## selects, left click confirms, right click cancels. Yes enables Christmas
## mode (Christmas.set_active), No and Esc disable it.
##
## During the prompt the mode is already active, as in the C code: the file
## tyrianc.shp is loaded before the question (opentyr.c) and it carries
## the snowflakes (sheet 7, sprites 225 and 226).
extends TyrianScreen

## xmas.c: geometry of the 320×200 screen.
const X_CENTER := 160
const Y_PROMPT := 85
const DY_PROMPT := 15
const W_CHOICE := 40
const Y_CHOICE := 120
const H_CHOICE := 13
const BACKGROUND := 0x8F
const FLAKE_COUNT := 80
const BACK_FLAKE_COUNT := FLAKE_COUNT * 2 / 3
## Snowflakes of the "main:7" sheet (spriteSheet8): background, foreground.
const SHEET := "main:7"
const FLAKE_BACK := 225
const FLAKE_FRONT := 226
const DY_DEN := 128
## SDL_Delay(16) of the C loop.
const FRAME_SECONDS := 0.016
const SCENE_TITLE := "res://scenes/title.tscn"

## True when the screen is waiting for a key (capture tool).
var idle := false
## Selected choice (0: Yes, 1: No) and answer: -1 pending, 0 no, 1 yes.
var selected := 0
var answer := -1
var _flakes_x := PackedInt32Array()
var _flakes_y := PackedInt32Array()
var _flakes_acc := PackedInt32Array()
var _flakes_add := PackedInt32Array()
var _rng := RandomNumberGenerator.new()
var _prompt: Array[String] = []
var _choices: Array[String] = []


func _ready() -> void:
	if Capture.active:
		_rng.seed = 1
	else:
		_rng.randomize()
	_run.call_deferred()


func _run() -> void:
	# opentyr.c: tyrianc.shp is loaded before the question.
	Christmas.set_active(true)
	_prompt = [TyrianText.L("xmas_detected"), TyrianText.L("xmas_activate")]
	_choices = [TyrianText.L("xmas_yes"), TyrianText.L("xmas_no")]
	mouse_pointer = MouseCursor.NORMAL
	_reset_flakes()
	vga.load_palette(0)
	var restart := true
	while answer < 0:
		_step_flakes()
		_draw_frame()
		if restart:
			await fade_palette(10, 0, 255)
			restart = false
			idle = true
		present()
		await _frame_wait()
		if not is_inside_tree():
			return
	idle = false
	# xmas.c: fade_black(10) before returning the answer.
	await fade_black(10)
	var yes := answer == 1
	if not Capture.active:
		Christmas.decided = true
		Christmas.set_active(yes)
		Effects.show_scene(SCENE_TITLE)


func _frame_wait() -> void:
	if Capture.active:
		await get_tree().process_frame
	else:
		await get_tree().create_timer(FRAME_SECONDS, true, false, true).timeout


## xmas.c, `restart`: the snowflakes restart above the screen, each at its own speed.
func _reset_flakes() -> void:
	_flakes_x.resize(FLAKE_COUNT)
	_flakes_y.resize(FLAKE_COUNT)
	_flakes_acc.resize(FLAKE_COUNT)
	_flakes_add.resize(FLAKE_COUNT)
	for i in FLAKE_COUNT:
		_flakes_y[i] = 200 + _rng.randi() % 200
		_flakes_add[i] = DY_DEN / 2 + i * DY_DEN / FLAKE_COUNT


## xmas.c: one fall step (snowflake gone off the bottom: back above at a drawn x,
## drifts one pixel one time in eight, fractional speed by accumulation).
func _step_flakes() -> void:
	for i in FLAKE_COUNT:
		if _flakes_y[i] >= 200:
			_flakes_x[i] = _rng.randi() % 320
			_flakes_y[i] = 200 - 14 - _flakes_y[i]
			_flakes_acc[i] = _flakes_add[i]
		var temp := _rng.randi() & 0xF
		if (temp & 0xE) == 0:
			_flakes_x[i] += (temp & 1) * 2 - 1
		var dy_num := _flakes_acc[i] + _flakes_add[i]
		_flakes_acc[i] = dy_num % DY_DEN
		_flakes_y[i] += dy_num / DY_DEN


func _draw_frame() -> void:
	vga.fill_rectangle_xy(VGA_SCREEN, 0, 0, 319, 199, BACKGROUND)
	for i in BACK_FLAKE_COUNT:
		vga.blit_sprite2(VGA_SCREEN, _flakes_x[i], _flakes_y[i], SHEET, FLAKE_BACK, 1, 0, false)
	for i in _prompt.size():
		TyrianText.draw_font_hv_full_shadow(
			self, X_CENTER, Y_PROMPT + DY_PROMPT * i, _prompt[i], NORMAL_FONT, CENTERED, 2 if i % 2 else 4, -2, true, 1
		)
	for i in _choices.size():
		var x := X_CENTER - W_CHOICE / 2 + W_CHOICE * i
		TyrianText.draw_font_hv_full_shadow(
			self, x, Y_CHOICE, _choices[i], NORMAL_FONT, CENTERED, 15, -2 if selected == i else -4, true, 1
		)
	for i in range(BACK_FLAKE_COUNT, FLAKE_COUNT):
		vga.blit_sprite2(VGA_SCREEN, _flakes_x[i], _flakes_y[i], SHEET, FLAKE_FRONT, 1, 0, false)


## Rank of the choice under point `p` (40 px zones centered on the words), -1 otherwise.
static func choice_at(p: Vector2) -> int:
	if p.y < Y_CHOICE or p.y >= Y_CHOICE + H_CHOICE:
		return -1
	for i in 2:
		var x_choice := X_CENTER - W_CHOICE + W_CHOICE * i
		if p.x >= x_choice and p.x < x_choice + W_CHOICE:
			return i
	return -1


## Yes = rank 0: the C answer is `!cancel && selectedIndex == 0`.
func _finish(cancel: bool) -> void:
	if answer >= 0:
		return
	answer = 1 if (not cancel and selected == 0) else 0


func handle_action(action: StringName) -> void:
	if answer >= 0:
		return
	match action:
		TyrianControls.LEFT, TyrianControls.RIGHT:
			selected = 1 - selected
		TyrianControls.ACCEPT, TyrianControls.SELECT:
			_finish(false)
		TyrianControls.BACK:
			_finish(true)


## xmas.c, mouse: hovering a word selects it; left click on a word = confirm;
## right click anywhere = cancel. A wheel button does not count.
func handle_mouse(event: InputEvent) -> void:
	if answer >= 0 or TyrianMouse.is_wheel(event):
		return
	if event is InputEventMouseMotion:
		var hovered := choice_at(event.position)
		if hovered >= 0 and event.relative != Vector2.ZERO:
			selected = hovered
		return
	if not TyrianMouse.is_button_press(event):
		return
	var clicked := choice_at(event.position)
	if clicked >= 0:
		selected = clicked
	if event.button_index == MOUSE_BUTTON_RIGHT:
		_finish(true)
	elif event.button_index == MOUSE_BUTTON_LEFT and clicked >= 0:
		_finish(false)
