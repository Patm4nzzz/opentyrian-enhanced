## Level hotkeys from the C code (input_map.c:enhanced_keys[], mainint.c): F2 pause
## (handled by level.gd), F3 achievements, F4 statistics, F5 quick save,
## F6 DPS counter, F7 damage numbers, F9 quick load, F10
## replay recording, F11 gamma, F12 screenshot, and slow motion
## (left Shift instead of the C's Tab: intentional deviation, divergence no. 31).
## The keys are rebindable (TyrianControls.GLOBAL_COMMANDS).
class_name LevelHotkeys
extends RefCounted

const SetupScreen := preload("res://scripts/setup.gd")
const C := preload("res://scripts/controls.gd")
const QUICK_SLOT_ONE_PLAYER := 11
const QUICK_SLOT_TWO_PLAYERS := 22

var level: Node
var overlays := LevelOverlays.new()
## input_map.c:quickload_confirm_pending.
var quick_load_pending := false
## Left trigger edge: true while it is held.
var _bullet_axis_down := false
## Number of the next screenshot (video.c:JE_saveScreenshot), -1 = to be looked up.
static var _screenshot_next := -1

const DEV_CONSOLE := preload("res://scripts/dev_console.gd")


func _init(owner_level: Node) -> void:
	level = owner_level


## Handles an event; true if it was consumed.
func handle(event: InputEvent) -> bool:
	if quick_load_pending and _answer_quick_load(event):
		return true
	if overlays.achievements_active:
		# Mouse wheel: one line per notch, like Up / Down (maintainer, 2026-10-10).
		var step := TyrianMouse.wheel_step(event)
		if step != 0:
			overlays.scroll(step)
			return true
		_navigate_achievements(event)
	# Left gamepad trigger: slow motion (rising edge).
	var axis := C.pad_axis_edge(event, C.KEY_BULLET_TIME, _bullet_axis_down)
	if event is InputEventJoypadMotion and event.is_action(C.KEY_BULLET_TIME):
		_bullet_axis_down = axis[0]
		if axis[1]:
			bullet_time()
			return true
		return false
	# Mouse side button 4: slow motion.
	if TyrianMouse.is_button_press(event) and event.is_action(C.KEY_BULLET_TIME):
		bullet_time()
		return true
	if not _pressed(event):
		return false
	if _is(event, C.KEY_ACHIEVEMENTS):
		# input_map.c:input_action_toggle_achievements: achievements_check first.
		if level.sim != null and level.sim.has_method("ach_check_now"):
			level._ach_set_context()
			level.ach_unlocked_many(level.sim.ach_check_now())
		overlays.toggle_achievements()
	elif _is(event, C.KEY_STATS):
		overlays.toggle_stats()
	elif _is(event, C.KEY_DPS):
		overlays.toggle_dps()
	elif _is(event, C.KEY_DAMAGE_NUMBERS):
		SetupScreen.toggle_damage_numbers()
		_message("msg_damage_numbers_on" if SetupScreen.settings.damage_numbers_enabled else "msg_damage_numbers_off")
	elif _is(event, C.KEY_QUICK_SAVE):
		quick_save()
	elif _is(event, C.KEY_QUICK_LOAD):
		quick_load_press()
	elif _is(event, C.KEY_REPLAY):
		if level.interactive and not level.replaying:
			level.toggle_recording()
	elif _is(event, C.KEY_SCREENSHOT):
		take_screenshot()
	elif _is(event, C.GAMMA):
		cycle_gamma()
	elif _is(event, C.KEY_BULLET_TIME) and event is InputEventKey:
		# A player's game key (left Shift reassigned to fire, for
		# example) keeps its game function.
		if C.is_game_key(event.physical_keycode):
			return false
		# With --devmode, Ctrl+Shift+letter is a cheat (dev_console.c): in the C code
		# slow motion is on Tab, so this prefix never triggers it.
		if event.ctrl_pressed and DEV_CONSOLE.available():
			return false
		bullet_time()
	else:
		return false
	return true


## Non-repeating press of a keyboard key (the C level hotkeys
## are keys, input_map.c).
func _pressed(event: InputEvent) -> bool:
	return event is InputEventKey and event.pressed and not event.echo


func _is(event: InputEvent, action: StringName) -> bool:
	return event.is_action_pressed(action, false, false)


# --- Achievements: navigation (mainint.c, achievements_scroll_*) ---------------


func _navigate_achievements(event: InputEvent) -> void:
	var action := C.menu_action(event)
	match action:
		C.UP:
			overlays.scroll(-1)
		C.DOWN:
			overlays.scroll(1)
		C.LEFT:
			overlays.change_category(-1)
		C.RIGHT:
			overlays.change_category(1)


# --- Slow motion (input_map.c:input_action_bullet_time) -----------------------


func bullet_time() -> void:
	if (
		not level.interactive
		or level.demo
		or level.replaying
		or not Effects.bullet_time_allowed
		or not Effects._level_running()
	):
		return
	if Effects.bullet_time_active():
		_message("msg_bullet_time_active")
	elif Effects.bullet_time_cooldown_seconds() > 0:
		_message_text(TyrianText.L("msg_cooldown").replace("%u", str(Effects.bullet_time_cooldown_seconds())))
	else:
		var sim: Node = level.sim
		if sim != null and sim.has_method("ach_counter"):
			Effects.bullet_time_uses = maxi(Effects.bullet_time_uses, int(sim.ach_counter("bullet_time_uses")))
		if not Effects.bullet_time_activate():
			return
		_message("msg_bullet_time")
		if sim != null and sim.has_method("ach_note_bullet_time"):
			sim.ach_set_bullet_time_uses(Effects.bullet_time_uses)
			level.ach_unlocked_many(sim.ach_note_bullet_time())


# --- Screenshot (input_map.c:input_action_screenshot) -------------------------


## video.c:JE_saveScreenshot: first free number, BMP file in the user
## folder; returns number + 1 (1 = screenshot_0000), 0 on failure.
static func save_screenshot(image: Image, dir: String) -> int:
	if image == null or image.is_empty():
		return 0
	if _screenshot_next < 0:
		_screenshot_next = 0
		while (
			_screenshot_next < 10000 and FileAccess.file_exists(dir.path_join("screenshot_%04d.bmp" % _screenshot_next))
		):
			_screenshot_next += 1
	var number := _screenshot_next
	var rgba := image.duplicate() as Image
	rgba.convert(Image.FORMAT_RGBA8)
	var path := dir.path_join("screenshot_%04d.bmp" % number)
	if not bool(
		ClassDB.class_call_static("TyrianVga", "save_bmp", path, rgba.get_width(), rgba.get_height(), rgba.get_data())
	):
		return 0
	_screenshot_next += 1
	return number + 1


func take_screenshot() -> void:
	var image: Image = null
	if DisplayServer.get_name() != "headless":
		image = level.get_viewport().get_texture().get_image()
	var dir := UserData.dir()
	var number := save_screenshot(image, dir)
	if number > 0:
		_message_text(TyrianText.L("msg_screenshot_saved").replace("%u", str(number - 1)))
		var sim: Node = level.sim
		if sim != null and sim.has_method("ach_note_screenshot"):
			level.ach_unlocked_many(sim.ach_note_screenshot())
	else:
		_message("msg_screenshot_fail")


# --- Gamma (mainint.c:JE_gammaCheck) -------------------------------------------


func cycle_gamma() -> void:
	DisplaySettings.cycle_gamma()
	Effects.apply_gamma()


# --- Quick save and quick load (input_map.c) ----------------------------------


## Neither demo, nor replay, nor Endless (mainint.c: !play_demo, !replay_is_spectating,
## !endlessMode).
func _quick_allowed() -> bool:
	return (
		level.interactive
		and not level.demo
		and not level.replaying
		and not EndlessSession.active()
		and Game.campaign_active
		and level.sim != null
	)


## Key ignored in a played level (side mode, Endless): a short message
## instead of silence. Demo and replay stay silent.
func _quick_refused() -> void:
	if level.interactive and not level.demo and not level.replaying:
		_message("msg_quick_campaign_only")


func _quick_slot() -> int:
	return QUICK_SLOT_TWO_PLAYERS if Game.two_player_game() else QUICK_SLOT_ONE_PLAYER


## F5: slot 11 (22 in two-player) named "QUICK SAVE", with
## the current equipment and money (JE_saveGame reads player[].items).
func quick_save() -> void:
	if not _quick_allowed():
		_quick_refused()
		return
	var stats: Dictionary = level.sim.level_stats()
	if Game.save_campaign(
		_quick_slot(),
		TyrianText.L("save_quick_save"),
		Game.last_level_section,
		stats.get("loadout", PackedInt32Array())
	):
		_message("msg_quick_saved")


## F9, first press: pinned question if a slot exists, otherwise a message.
func quick_load_press() -> void:
	if not _quick_allowed():
		_quick_refused()
		return
	var slot := _quick_slot()
	var slots := Game.campaign_slots()
	var s = slots[slot - 1] if slots.size() >= slot else null
	if s != null and str(s.get("error", "")) == "":
		quick_load_pending = true
		level.hud_pin_message(TyrianText.L("msg_quick_load_prompt"))
	else:
		_message("msg_quick_load_none")


## Answer to the question: Y or F9 loads, N or Esc cancels.
## mainint.c:locale_yes_scancode: the prompt announces the language's "yes" letter
## (O in French, J in German, S in Spanish, Italian and Portuguese);
## that letter also confirms. SDL_GetScancodeFromName gives the physical key
## of that name, hence physical_keycode.
static func is_locale_yes_letter(event: InputEvent) -> bool:
	if not (event is InputEventKey):
		return false
	var letter := TyrianText.L("key_yes_letter")
	if letter.is_empty() or letter == "key_yes_letter":
		return false
	var code := OS.find_keycode_from_string(letter)
	var k := event as InputEventKey
	return code != KEY_NONE and (k.physical_keycode == code or (k.physical_keycode == KEY_NONE and k.keycode == code))


func _answer_quick_load(event: InputEvent) -> bool:
	if not (event is InputEventKey and event.pressed and not event.echo):
		return false
	if event.is_action_pressed(C.YES, false, false) or _is(event, C.KEY_QUICK_LOAD) or is_locale_yes_letter(event):
		quick_load_pending = false
		level.hud_pin_message("")
		level.quick_load_confirmed()
		return true
	if event.is_action_pressed(C.NO, false, false) or event.is_action_pressed(C.BACK, false, false):
		quick_load_pending = false
		level.hud_pin_message("")
		_message("msg_cancelled")
		# mainint.c:3107-3110: the cancel is consumed (INPUT_MENU_CANCEL),
		# Esc must not also open the pause menu (level.gd reads it by
		# polling: we make it pass for a resume).
		level._resumed_frame = Engine.get_physics_frames()
		return true
	return false


func _message(key: String) -> void:
	_message_text(TyrianText.L(key))


func _message_text(text: String) -> void:
	level.hud_queue_message(text)
