## In-game pause menu (mainint.c:JE_inGameSetup), opened by Esc (or
## gamepad Back) like INPUT_MENU_CANCEL in tyrian2.c. Rendered by the
## shared menu engine (MENU_TREE style with the C geometry and no_shadow)
## on the frozen image of the level, darkened by JE_barShade.
##
## Rows: music and effects volume (same settings as Setup >
## Sound), visual effects (Visual FX preset), game speed, back, quit
## (JE_quitRequest confirmation). Changed settings are applied right
## away and saved on close (save_json_save). The simulation does not
## run while the menu is open.
extends "res://scripts/frozen_level_screen.gd"

const Kind := MenuEngine.Kind
const SETUP := preload("res://scripts/setup.gd")
const GAME_SPEED := preload("res://scripts/game_speed.gd")

## mainint.c: geometry of the pause menu.
const PAUSE_X := 10
const PAUSE_Y_START := 20
const PAUSE_STEP := 20
const PAUSE_ROW_W := 200
const PAUSE_NAME_W := 110
const PAUSE_VALUE_X := PAUSE_X + PAUSE_NAME_W
const PAUSE_VALUE_W := 90
## mainint.c:VOLUME_BAR_W (scale of JE_barDrawShadow).
const VOLUME_BAR_W := (255 + 6) / 12 * (3 + 1) - 1
## visual_fx.c:visual_fx_preset_label (Off, Basic, Extended).
const VISUAL_FX_LABELS := ["fx_preset_off", "fx_preset_basic", "fx_preset_extended"]
## game_menu.c:JE_quitRequestMsg: click areas of "Yes" and "No".
const QUIT_BOX_Y := Vector2i(124, 148)
const QUIT_BOX_YES_X := Vector2i(57, 141)
const QUIT_BOX_NO_X := Vector2i(152, 236)

## Close: `quit` true if "Quit" was confirmed.
signal closed(quit: bool)

var menus := MenuEngine.new(self)
var _closing := false


## JE_inGameSetup: darkened frozen image (four JE_barShade passes),
## kept in VGAScreen2 and copied back every frame (pause_predraw).
func open_menu(lvl: Node) -> bool:
	if not freeze(lvl):
		return false
	vga.bar_shade(VGA_SCREEN, 3, 13, 217, 137)
	vga.bar_shade(VGA_SCREEN, 5, 15, 215, 135)
	vga.bar_shade(VGA_SCREEN, 5, 15, 215, 135)
	vga.bar_shade(VGA_SCREEN, 5, 15, 215, 135)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	(
		menus
		. open(
			"",
			_items(),
			{
				"x": PAUSE_X,
				"y_start": PAUSE_Y_START,
				"step": PAUSE_STEP,
				"row_width": PAUSE_ROW_W,
				"no_shadow": true,
				"background": _restore_frozen_level,
				"on_back": _close.bind(false),
			}
		)
	)
	return true


## Menu background: the frozen level image as is. The default menu engine
## puts the black band of leftover scenery text from the setup screens on it
## (x 204..319, y 191..199), which would cut off the bottom of the dashboard
## and the message bar here.
func _restore_frozen_level() -> void:
	vga.copy(VGA_SCREEN, VGA_SCREEN2)


func _items() -> Array:
	return [
		(
			MenuEngine
			. item(
				"ig_music_vol",
				"",
				Kind.CUSTOM,
				{
					"volume": "music_volume",
					"disabled": "music_disabled",
					"draw": _draw_volume,
					"activate": _volume_toggle,
					"adjust": _volume_adjust,
					"click": _volume_click,
				}
			)
		),
		(
			MenuEngine
			. item(
				"ig_sfx_vol",
				"",
				Kind.CUSTOM,
				{
					"volume": "fx_volume",
					"disabled": "samples_disabled",
					"draw": _draw_volume,
					"activate": _volume_toggle,
					"adjust": _volume_adjust,
					"click": _volume_click,
				}
			)
		),
		(
			MenuEngine
			. item(
				"ig_detail",
				"",
				Kind.CUSTOM,
				{
					"draw": _draw_vfx,
					"activate": _vfx_adjust.bind(1),
					"adjust": _vfx_adjust,
					"click": _vfx_click,
				}
			)
		),
		(
			MenuEngine
			. item(
				"ig_game_speed",
				"",
				Kind.CUSTOM,
				{
					"draw": _draw_speed,
					"activate": _speed_adjust.bind(1),
					"adjust": _speed_adjust,
					"click": _speed_click,
				}
			)
		),
		MenuEngine.item("ig_return", "", Kind.BACK),
		MenuEngine.item("ig_quit", "", Kind.ACTION, {"action": _quit_action}),
	]


# --- Rows (mainint.c: volume_row_*, vfx_row_*, speed_row_*) -----------------


## mainint.c:volume_row_draw: JE_barDrawShadow bar (color 12 if muted).
func _draw_volume(it: Dictionary, x: int, y: int, selected: bool) -> void:
	menus.draw_label_no_shadow(x, y, TyrianText.L(it.label), selected)
	var muted: bool = SETUP.settings[it.disabled]
	bar_draw_shadow(PAUSE_VALUE_X, y, 1, 12 if muted else 16, (int(SETUP.settings[it.volume]) + 6) / 12, 3, 13)


## Enter: mutes or restores the sound (S_SELECT).
func _volume_toggle(it: Dictionary) -> void:
	Sfx.play(Sfx.SELECT)
	SETUP.settings[it.disabled] = not SETUP.settings[it.disabled]
	SETUP.apply_settings()


## Left / Right: ±12 (one notch of the bar), nortsong.c:JE_changeVolume
## (clamped to 0..255, S_CLINK at the limit).
func _volume_adjust(it: Dictionary, dir: int) -> void:
	Sfx.play(Sfx.CURSOR)
	var v := int(SETUP.settings[it.volume]) + dir * 12
	if v > 255 or v < 0:
		v = clampi(v, 0, 255)
		Sfx.play(Sfx.CLINK)
	SETUP.settings[it.volume] = v
	SETUP.apply_settings()


## Click: name column = mute; value column = volume at the point.
func _volume_click(it: Dictionary, rel_x: int) -> void:
	if rel_x < PAUSE_NAME_W:
		_volume_toggle(it)
		return
	var in_value := rel_x - PAUSE_NAME_W
	if in_value < 0 or in_value >= PAUSE_VALUE_W:
		return
	SETUP.settings[it.volume] = clampi(in_value * 255 / (VOLUME_BAR_W - 1), 0, 255)
	SETUP.apply_settings()
	Sfx.play(Sfx.CURSOR)


## mainint.c:vfx_row_draw: value aligned on the bars column.
func _draw_vfx(it: Dictionary, x: int, y: int, selected: bool) -> void:
	menus.draw_label_no_shadow(x, y, TyrianText.L(it.label), selected)
	var i := clampi(int(SETUP.settings.visual_fx), 0, VISUAL_FX_LABELS.size() - 1)
	menus.draw_value_no_shadow(PAUSE_VALUE_X, y, TyrianText.L(VISUAL_FX_LABELS[i]), selected)


## mainint.c:vfx_row_act: next preset (Right, Enter, click) or previous.
func _vfx_adjust(_it: Dictionary, dir: int) -> void:
	Sfx.play(Sfx.CURSOR)
	var cur := int(SETUP.settings.visual_fx)
	SETUP.settings.visual_fx = (cur + 1) % 3 if dir >= 0 else (cur + 2) % 3
	SETUP.apply_settings()


func _vfx_click(it: Dictionary, _rel_x: int) -> void:
	_vfx_adjust(it, 1)


## mainint.c:speed_row_draw: gameSpeedText[gameSpeed - 1].
func _draw_speed(it: Dictionary, x: int, y: int, selected: bool) -> void:
	menus.draw_label_no_shadow(x, y, TyrianText.L(it.label), selected)
	var label: String = GAME_SPEED.LABELS[clampi(int(SETUP.settings.game_speed), 1, 5) - 1]
	menus.draw_value_no_shadow(PAUSE_VALUE_X, y, TyrianText.L(label), selected)


## mainint.c:speed_row_act: 1..5 in a loop, then JE_initProcessorType and
## JE_setNewGameSpeed (the tick rate changes on resume).
func _speed_adjust(_it: Dictionary, dir: int) -> void:
	Sfx.play(Sfx.CURSOR)
	var cur := int(SETUP.settings.game_speed)
	if dir >= 0:
		cur = cur + 1 if cur < 5 else 1
	else:
		cur = cur - 1 if cur > 1 else 5
	SETUP.settings.game_speed = cur
	GAME_SPEED.apply(cur)


func _speed_click(it: Dictionary, _rel_x: int) -> void:
	_speed_adjust(it, 1)


## mainint.c:pause_quit_action: the menu closes (one more frame, like
## run_menu) then JE_quitRequest puts its box on the last frame.
func _quit_action() -> void:
	Sfx.play(Sfx.SELECT)
	_quit_request.call_deferred()


func _quit_request() -> void:
	idle = false
	var yes: bool = await quit_request_msg(TyrianText.L("miscText_28"), TyrianText.L("miscText_30"))
	_close(yes)


func _close(quit: bool) -> void:
	if _closing:
		return
	_closing = true
	# save_json_save: settings changed during the pause are saved.
	SETUP.save_settings()
	closed.emit(quit)


# --- Input -------------------------------------------------------------------

## Keys received by the JE_quitRequestMsg box, in order: the C code reads them
## one per frame (newkey); a single slot lost Left followed by Enter
## in the same wait.
var _modal_queue: Array[StringName] = []


func modal_input(action: StringName) -> bool:
	if not modal_active:
		return false
	_modal_queue.append(action)
	_feed_modal()
	return true


func _feed_modal() -> void:
	if modal_active and _modal_key == null and not _modal_queue.is_empty():
		_modal_key = _modal_queue.pop_front()


func _process(_delta: float) -> void:
	_feed_modal()


func handle_action(action: StringName) -> void:
	if _closing or modal_input(action):
		return
	menus.handle_action(action)


## Keyboard and gamepad through menu actions (TyrianScreen.handle_event);
## mouse like menu_tree.c:run_menu (hover, left click, right click = exit).
func _unhandled_input(event: InputEvent) -> void:
	if _closing:
		return
	if event is InputEventMouseMotion or event is InputEventMouseButton:
		mouse_event(event)
	else:
		handle_event(event)
	get_viewport().set_input_as_handled()


func mouse_event(event: InputEvent) -> void:
	var p := Vector2i(make_input_local(event).position)
	if modal_active:
		# Hover over "Yes" / "No": moves the selection (modal_mouse).
		if event is InputEventMouseMotion:
			modal_mouse(event)
			return
		# JE_quitRequestMsg: click on "Yes" or "No".
		if TyrianMouse.is_button_press(event) and p.y > QUIT_BOX_Y.x - 1 and p.y < QUIT_BOX_Y.y + 1:
			if p.x > QUIT_BOX_YES_X.x - 1 and p.x < QUIT_BOX_YES_X.y + 1:
				modal_input(TyrianControls.YES)
			elif p.x > QUIT_BOX_NO_X.x - 1 and p.x < QUIT_BOX_NO_X.y + 1:
				modal_input(TyrianControls.NO)
		return
	var m: Dictionary = menus.current()
	var i := menus.hittest_row(p.x, p.y)
	if event is InputEventMouseMotion:
		# menu_tree.c:run_menu: hover only when the mouse is active (mouseInactive false).
		if not TyrianMouse.inactive and i >= 0 and i != m.sel:
			m.sel = i
			Sfx.play(Sfx.CLICK)
			menus.draw()
		return
	if not event.pressed:
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		menus.back()
		return
	if event.button_index != MOUSE_BUTTON_LEFT or i < 0:
		return
	if i != m.sel:
		m.sel = i
		Sfx.play(Sfx.CLICK)
	var it: Dictionary = m.items[i]
	if it.kind == Kind.BACK:
		menus.back()
		return
	if it.kind == Kind.CUSTOM:
		it.click.call(it, p.x - PAUSE_X)
	else:
		menus.activate(it)
	menus.draw()
