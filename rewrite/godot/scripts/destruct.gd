## Destruct mini-game (destruct.c, ADR 0132): two sides bombard each other in the
## manner of Scorched Earth, single player (against the computer) or two on
## the same keyboard.
##
## The logic (terrain, units, shots, explosions, AI, end of round) is in
## tyrian-sim (destruct.rs), exposed by TyrianDestruct; this screen reproduces
## the flow of destruct.c:JE_destructGame / JE_destructMain frame by frame
## in the shared 8-bit framebuffer (each `present()` = one JE_showVGA of the
## C): introduction, mode selection (shared menu engine), rounds,
## pause (P), help (F1), human / computer toggle (left F10, right Gamma, F11 by default),
## new map (Backspace), return to mode selection (Escape).
##
## Hidden configuration: userdata/destruct.cfg, same format and same keys as
## the C's destruct.cfg (destruct.c:load_destruct_config, CONFIG.md), read and
## rewritten on every entry. The game keys of this file become
## InputMap actions "destruct_p1_*" / "destruct_p2_*".
extends TyrianScreen

const SCENE_TITLE := "res://scenes/title.tscn"
enum { STATE_INIT, STATE_RELOAD, STATE_CONTINUE }
## destruct.c:key_names (order of de_keys_t / de_move_t).
const COMMANDS := ["left", "right", "up", "down", "change", "fire", "previous_weapon", "next_weapon"]
## Gamepad: TyrianControls command (player 1 and 2) reused by each
## Destruct action (d-pad, stick, A fire, B unit switch, X/Y and
## upper triggers: previous / next weapon).
const PAD_COMMANDS := ["left", "right", "up", "down", "change_fire", "fire", "left_sidekick", "right_sidekick"]
const MOVE_CHANGE := 4
const MOVE_CYUP := 6
const MOVE_CYDN := 7
static var CONFIG_FILE: String = UserData.path("destruct.cfg")
## destruct.c: DESTRUCT_MODES modes from the data, + "Custom" (allow_custom).
const DESTRUCT_MODES := 5
## Frame-by-frame capture (--keys): an injected key stays held as many
## ticks as a 0.1 s xdotool press on the C side (visual_capture_c.sh), i.e.
## 7 ticks of 14.4 ms.
const CAPTURE_HOLD_TICKS := 7
## Capture: screen at rest after this number of identical frames in a row.
const CAPTURE_SETTLE_TICKS := 8
## Gamepad: key names accepted by the script (SDL_GetScancodeFromName).
const SCRIPT_KEY_NAMES := {
	"Return": KEY_ENTER,
	"Enter": KEY_ENTER,
	"Escape": KEY_ESCAPE,
	"Backspace": KEY_BACKSPACE,
	"Tab": KEY_TAB,
	"Space": KEY_SPACE,
	"Left Shift": KEY_SHIFT,
	"Left Ctrl": KEY_CTRL,
	"Left Alt": KEY_ALT,
	"Right Shift": KEY_SHIFT,
	"Right Ctrl": KEY_CTRL,
	"Right Alt": KEY_ALT,
	"Left": KEY_LEFT,
	"Right": KEY_RIGHT,
	"Up": KEY_UP,
	"Down": KEY_DOWN,
	"Insert": KEY_INSERT,
	"PageUp": KEY_PAGEUP,
	"PageDown": KEY_PAGEDOWN,
	"Keypad 0": KEY_KP_0,
	"Keypad 1": KEY_KP_1,
	"Keypad 2": KEY_KP_2,
	"Keypad 3": KEY_KP_3,
	"Keypad 4": KEY_KP_4,
	"Keypad 5": KEY_KP_5,
	"Keypad 6": KEY_KP_6,
	"Keypad 7": KEY_KP_7,
	"Keypad 8": KEY_KP_8,
	"Keypad 9": KEY_KP_9,
	"Keypad Enter": KEY_KP_ENTER,
}

## SDL_Scancode -> Godot physical key (+ side for modifiers):
## names accepted by destruct.cfg (SDL_GetScancodeFromName).
const SCANCODE_KEYS := {
	40: KEY_ENTER,
	41: KEY_ESCAPE,
	42: KEY_BACKSPACE,
	43: KEY_TAB,
	44: KEY_SPACE,
	45: KEY_MINUS,
	46: KEY_EQUAL,
	47: KEY_BRACKETLEFT,
	48: KEY_BRACKETRIGHT,
	49: KEY_BACKSLASH,
	51: KEY_SEMICOLON,
	52: KEY_APOSTROPHE,
	53: KEY_QUOTELEFT,
	54: KEY_COMMA,
	55: KEY_PERIOD,
	56: KEY_SLASH,
	57: KEY_CAPSLOCK,
	70: KEY_PRINT,
	71: KEY_SCROLLLOCK,
	72: KEY_PAUSE,
	73: KEY_INSERT,
	74: KEY_HOME,
	75: KEY_PAGEUP,
	76: KEY_DELETE,
	77: KEY_END,
	78: KEY_PAGEDOWN,
	79: KEY_RIGHT,
	80: KEY_LEFT,
	81: KEY_DOWN,
	82: KEY_UP,
	83: KEY_NUMLOCK,
	84: KEY_KP_DIVIDE,
	85: KEY_KP_MULTIPLY,
	86: KEY_KP_SUBTRACT,
	87: KEY_KP_ADD,
	88: KEY_KP_ENTER,
	89: KEY_KP_1,
	90: KEY_KP_2,
	91: KEY_KP_3,
	92: KEY_KP_4,
	93: KEY_KP_5,
	94: KEY_KP_6,
	95: KEY_KP_7,
	96: KEY_KP_8,
	97: KEY_KP_9,
	98: KEY_KP_0,
	99: KEY_KP_PERIOD,
	101: KEY_MENU,
	224: [KEY_CTRL, KEY_LOCATION_LEFT],
	225: [KEY_SHIFT, KEY_LOCATION_LEFT],
	226: [KEY_ALT, KEY_LOCATION_LEFT],
	227: [KEY_META, KEY_LOCATION_LEFT],
	228: [KEY_CTRL, KEY_LOCATION_RIGHT],
	229: [KEY_SHIFT, KEY_LOCATION_RIGHT],
	230: [KEY_ALT, KEY_LOCATION_RIGHT],
	231: [KEY_META, KEY_LOCATION_RIGHT],
}

## Effects forwards these keys to the screen instead of handling them (F10:
## display options, Shift: bullet time): Destruct uses them in play.
var reserved_keys := [KEY_F10, KEY_SHIFT]
## True when the screen is waiting for a key (capture tool).
var idle := false
var game: RefCounted
var menus := MenuEngine.new(self)
## Current phase: "intro", "menu", "play", "wait" (pause, help), "" otherwise.
var phase := ""
## keyboard.c:keysactive: held key (physical_keycode | side << 24),
## reset to false when the game "consumes" it.
var _keys := {}
## keyboard.c:newkey and lastkey_scan.
var _newkey := false
var _last_key := 0
var _mode_result := -2
var _tick := 0
var _capture_release := {}
var _same_frames := 0
var _last_hash := 0
var _next_tick_ms := 0.0
var _channels: Array[AudioStreamPlayer2D] = []
## Gamepad buttons already seen pressed (rising edge of consumed actions).
var _pad_prev := {}
## Gamepad exit button pressed during the round (read as Escape).
var _pad_back := false
## Capture script (--destruct-script): [[tick, duration, [code...]], ...], like
## the C's GOLDEN_DESTRUCT_KEYS (descriptor below).
var _script: Array = []
## Keys held by the script at the previous tick (released at the next).
var _script_held := {}
## Capture: number of ticks played, then exit (--destruct-ticks).
var _done := false


func _ready() -> void:
	mouse_cursor = false
	game = ClassDB.instantiate("TyrianDestruct") if ClassDB.class_exists("TyrianDestruct") else null
	for i in 8:
		# Sound placed at the x position of the event (soundQueueX, panning).
		var p := AudioStreamPlayer2D.new()
		add_child(p)
		_channels.append(p)
	_run.call_deferred()


## destruct.c has no mouse access (neither JE_mouseStart nor mouse_x): no effect.
func handle_mouse(_event: InputEvent) -> void:
	pass


## destruct.c:JE_destructGame.
func _run() -> void:
	vga.clear(VGA_SCREEN, 0)
	present()
	if game == null:
		push_error("GDExtension tyrian-godot missing: TyrianDestruct unavailable")
		Game.goto_scene(SCENE_TITLE)
		return
	_parse_script()
	_load_config()
	await fade_black(1)
	await _destruct_main()
	if _done:
		# Capture (--destruct-ticks): the scene stops, the tool records.
		idle = true
		await get_tree().create_timer(0.05).timeout
		# Frees the audio streams to avoid "resources still in use" on exit.
		for p in _channels:
			p.stream = null
		Sfx.release_streams()
		Game.quit_cleanly(0)
		return
	if is_inside_tree():
		Game.goto_scene(SCENE_TITLE)


## Reading then rewriting destruct.cfg (destruct.c:JE_destructGame) and
## HUD texts. --destruct-cfg=<file>: starting file (captures).
func _load_config() -> void:
	var src := CONFIG_FILE
	if Capture.has_option("destruct-cfg"):
		src = String(Capture._options["destruct-cfg"])
	var data := FileAccess.get_file_as_bytes(src) if FileAccess.file_exists(src) else PackedByteArray()
	var r: Dictionary = game.load_config(data, _rng_seed(), _rng_skip())
	for w in r.warnings:
		push_warning(w)
	var f := FileAccess.open(CONFIG_FILE, FileAccess.WRITE)
	if f != null:
		f.store_buffer(r.file)
		f.close()
	var names := PackedStringArray()
	for i in 17:
		names.append(TyrianText.L("destructWeapon_%d" % i))
	game.set_texts(names, TyrianText.L("destruct_hud_dmg"), TyrianText.L("destruct_hud_pts"))
	install_actions(game.key_config())


## --destruct-no-intro[=MODE]: skips the intro and the menu (given mode, 0 by
## default) and starts the round at tick 0, like the instrumented C version
## (GOLDEN_DESTRUCT_NO_INTRO / GOLDEN_DESTRUCT_MODE). The C's wait loops
## last a number of frames that depends on real time: without this shortcut,
## the two sequences do not line up.
func _no_intro() -> bool:
	return Capture.has_option("destruct-no-intro")


## --destruct-ticks=N: plays N round ticks then exits cleanly. The
## C capture stops the same way (golden_master.c, GOLDEN_DESTRUCT_TICKS):
## without it, the C version never stops and the sequences are not
## comparable.
func _tick_limit() -> int:
	return int(Capture._options.get("destruct-ticks", 0))


func _forced_mode() -> int:
	var v := String(Capture._options.get("destruct-no-intro", "")).to_int()
	return v if v >= 0 and v <= DESTRUCT_MODES else 0


## --destruct-script="t1:Name[+Name][/duration],...": same keys, at the same round
## tick as the instrumented C version (GOLDEN_DESTRUCT_KEYS,
## visual_capture_c.sh) — the C never stops in play, so xdotool keys
## would land at a tick that depends on real time. The intro / menu loops
## read the key set at tick 0 as a real press.
func _parse_script() -> void:
	if not Capture.has_option("destruct-script"):
		return
	for entry in String(Capture._options["destruct-script"]).split(",", false):
		var colon := entry.find(":")
		if colon < 0:
			continue
		var tick := int(entry.substr(0, colon))
		var rest := entry.substr(colon + 1)
		var hold := 1
		var slash := rest.find("/")
		if slash >= 0:
			hold = maxi(int(rest.substr(slash + 1)), 1)
			rest = rest.substr(0, slash)
		var codes := PackedInt32Array()
		for name in rest.split("+", false):
			var code: int = SCRIPT_KEY_NAMES.get(name, KEY_NONE)
			if code == KEY_NONE:
				code = OS.find_keycode_from_string(name)
			if code != KEY_NONE:
				codes.append(code)
		if codes.size() > 0:
			_script.append([tick, hold, codes])


## destruct.c:DE_RunTickGetInput: applies the script at the current tick; keys
## set at the previous tick and absent from this one are released
## (keysactive reset to false, like the instrumented C version).
func _script_tick() -> void:
	if _script.is_empty():
		return
	var held := {}
	for entry in _script:
		var at: int = entry[0]
		var hold: int = entry[1]
		if _tick >= at and _tick < at + hold:
			for code in entry[2]:
				var id := _key_id(scancode_event_for(code))
				_keys[id] = true
				held[id] = true
			_newkey = true
	for id in _script_held:
		if not held.has(id):
			_keys[id] = false
	_script_held = held


## Keyboard event of a physical key (script).
static func scancode_event_for(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.physical_keycode = code as Key
	return ev


## rng.c seed: that of opentyr.c:main (rng_seed(0)), or --rng-seed.
static func _rng_seed() -> int:
	if Capture.has_option("rng-seed"):
		return int(Capture._options["rng-seed"])
	return int(Time.get_unix_time_from_system()) ^ (OS.get_process_id() << 16)


## rng.c draws made by the C before entering Destruct via --goto:
## default score tables (config.c:JE_loadConfiguration, without a
## save, see high_scores.gd) then secretHint (mainint.c:JE_initPlayerData).
## Only with --rng-seed: in play, the seed is already unpredictable.
static func _rng_skip() -> int:
	if not Capture.has_option("rng-seed"):
		return 0
	var draws := 0
	for z in 22:
		draws += 3 if z % 6 > 2 else 2
	return draws + 120 + 1


## InputMap actions "destruct_p<player>_<command>" from the keys of
## destruct.cfg (key_config: 2 × 8 × 4 SDL_Scancode codes, 0 = empty).
static func install_actions(keys: PackedInt32Array) -> void:
	for p in 2:
		for k in COMMANDS.size():
			var action := action_name(p, k)
			if not InputMap.has_action(action):
				InputMap.add_action(action)
			InputMap.action_erase_events(action)
			for slot in 4:
				var i := (p * COMMANDS.size() + k) * 4 + slot
				if i >= keys.size() or keys[i] == 0:
					break
				var ev := scancode_event(keys[i])
				if ev != null:
					InputMap.action_add_event(action, ev)
			if not InputMap.has_action(TyrianControls.action_name(1, "fire")):
				TyrianControls.install()
			for ev in InputMap.action_get_events(TyrianControls.action_name(p + 1, PAD_COMMANDS[k])):
				if ev is InputEventJoypadButton or ev is InputEventJoypadMotion:
					InputMap.action_add_event(action, ev)
			if k < 4:
				var stick := TyrianControls.stick_action(p + 1, PAD_COMMANDS[k])
				if InputMap.has_action(stick):
					for ev in InputMap.action_get_events(stick):
						InputMap.action_add_event(action, ev)


static func action_name(player: int, command: int) -> StringName:
	return StringName("destruct_p%d_%s" % [player + 1, COMMANDS[command]])


## Godot key of an SDL_Scancode code (letters, digits, F1-F24 computed).
static func scancode_event(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	if code >= 4 and code <= 29:
		ev.physical_keycode = (KEY_A + code - 4) as Key
	elif code >= 30 and code <= 38:
		ev.physical_keycode = (KEY_1 + code - 30) as Key
	elif code == 39:
		ev.physical_keycode = KEY_0
	elif code >= 58 and code <= 69:
		ev.physical_keycode = (KEY_F1 + code - 58) as Key
	elif code >= 104 and code <= 115:
		ev.physical_keycode = (KEY_F13 + code - 104) as Key
	elif SCANCODE_KEYS.has(code):
		var k = SCANCODE_KEYS[code]
		if k is Array:
			ev.physical_keycode = k[0]
			ev.location = k[1]
		else:
			ev.physical_keycode = k
	else:
		return null
	return ev


static func _key_id(ev: InputEventKey) -> int:
	return int(ev.physical_keycode) | (int(ev.location) << 24)


# --- Inputs --------------------------------------------------------------------


## Every keyboard event goes through here (Effects, capture tool):
## keyboard.c:service_SDL_events (keysactive, newkey, lastkey_scan).
func handle_event(event: InputEvent) -> void:
	if not (event is InputEventKey):
		# keyboard.c: the gamepad becomes menu keys (newkey).
		if event is InputEventJoypadButton and event.pressed:
			_newkey = true
			# The C leaves the round via the gamepad (button changed to Escape,
			# joystick.c:push_joysticks_as_keyboard, destruct.c:1772). Here the
			# gamepad drives the left player as in a level (B = switch
			# unit): the exit takes the level-exit button (Back, LB).
			if phase == "play" and event.is_action_pressed(&"tyrian_level_exit", false, true):
				_pad_back = true
		if phase == "menu":
			var a := TyrianControls.menu_action(event)
			if a != &"":
				_menu_action(a)
		return
	var id := _key_id(event)
	if event.pressed:
		_keys[id] = true
		_newkey = true
		_last_key = id
		if phase == "menu":
			# JE_modeSelect re-reads the held state on every key, repeats included.
			_menu_action(TyrianControls.menu_action(event))
		if Capture.active:
			# --keys only sends the press: released like a 0.1 s xdotool press
			# (about 7 ticks in play, before the next tick elsewhere).
			if phase == "play":
				_capture_release[id] = _tick + CAPTURE_HOLD_TICKS
				_same_frames = 0
				idle = false
			else:
				_keys[id] = false
	else:
		_keys[id] = false


func handle_action(_action: StringName) -> void:
	pass  # keys go through handle_event


## Key of action event `ev` held (side unspecified: either one).
func _key_held(ev: InputEventKey, consume := false) -> bool:
	var ids := [_key_id(ev)]
	if ev.location == KEY_LOCATION_UNSPECIFIED:
		ids.append(int(ev.physical_keycode) | (KEY_LOCATION_LEFT << 24))
		ids.append(int(ev.physical_keycode) | (KEY_LOCATION_RIGHT << 24))
	for id in ids:
		if _keys.get(id, false):
			if consume:
				_keys[id] = false
			return true
	return false


## Fixed C key (F1, F10, P, Backspace): keysactive[...] read
## then reset to false.
func _consume_key(code: int) -> bool:
	var ev := InputEventKey.new()
	ev.physical_keycode = code as Key
	return _key_held(ev, true)


## input_held(INPUT_GAMMA) + input_consume: keyboard key of the Gamma action.
func _consume_gamma() -> bool:
	for ev in InputMap.action_get_events(TyrianControls.GAMMA):
		if ev is InputEventKey and _key_held(ev, true):
			return true
	return false


## input_held(INPUT_MENU_CANCEL) + input_consume: keyboard keys of the menu
## "back" action (Escape by default).
func _consume_back() -> bool:
	if _pad_back:
		_pad_back = false
		return true
	for ev in InputMap.action_get_events(TyrianControls.BACK):
		if ev is InputEventKey and _key_held(ev, true):
			return true
	return false


## destruct.c:DE_RunTickGetInput: moves[player * 8 + action]; the unit
## and weapon switch keys are consumed (one press = one notch).
func _read_moves() -> PackedInt32Array:
	var moves := PackedInt32Array()
	moves.resize(16)
	for p in 2:
		for k in COMMANDS.size():
			var consume := k == MOVE_CHANGE or k == MOVE_CYUP or k == MOVE_CYDN
			for ev in InputMap.action_get_events(action_name(p, k)):
				if ev is InputEventKey and _key_held(ev, consume):
					moves[p * COMMANDS.size() + k] = 1
					break
			if moves[p * COMMANDS.size() + k] == 0 and _pad_held(action_name(p, k), consume):
				moves[p * COMMANDS.size() + k] = 1
	return moves


## Gamepad: action held (or, for consumed actions, rising edge).
func _pad_held(action: StringName, edge: bool) -> bool:
	var held := false
	for ev in InputMap.action_get_events(action):
		if ev is InputEventJoypadButton:
			held = held or Input.is_joy_button_pressed(ev.device, ev.button_index)
		elif ev is InputEventJoypadMotion:
			var v := Input.get_joy_axis(ev.device, ev.axis)
			held = held or v * signf(ev.axis_value) > TyrianControls.STICK_DEADZONE
	if not edge:
		return held
	var was: bool = _pad_prev.get(action, false)
	_pad_prev[action] = held
	return held and not was


## Waiting for a key ("do service_SDL_events ... while (!newkey)" loops).
func _wait_newkey() -> void:
	_newkey = false
	idle = true
	while not _newkey and is_inside_tree():
		await get_tree().process_frame
	idle = false


# --- Screens -------------------------------------------------------------------


## destruct.c:JE_destructMain.
func _destruct_main() -> void:
	vga.load_pic(VGA_SCREEN, 11, false)
	if not _no_intro():
		await _intro_screen()
	game.reset_players()
	while is_inside_tree() and not _done:
		vga.load_pic(VGA_SCREEN, 11, false)
		var mode: int
		if _no_intro():
			mode = _forced_mode()
		else:
			mode = await _mode_select()
		if mode < 0:
			break
		var state := STATE_RELOAD
		while state == STATE_RELOAD and is_inside_tree():
			vga.load_pic(VGA_SCREEN, 11, false)
			Music.play_song(game.start_round(vga, mode))
			present()  # JE_generateTerrain : JE_showVGA
			_next_tick_ms = Time.get_ticks_usec() / 1000.0
			state = STATE_CONTINUE
			while state == STATE_CONTINUE and is_inside_tree():
				state = await _run_tick()
				if _tick_limit() > 0 and _tick >= _tick_limit():
					state = STATE_CONTINUE
					_done = true
					break
			if _done:
				break
			await fade_black(25)


## destruct.c:JE_introScreen.
func _intro_screen() -> void:
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	_out_centered(TyrianText.L("specialName_9"), 90, 12, 5)
	_out_centered(TyrianText.L("miscText_64"), 180, 15, 2)
	_out_centered(TyrianText.L("miscText_65"), 190, 15, 2)
	present()
	await fade_palette(15)
	phase = "intro"
	await _wait_newkey()
	phase = ""
	await fade_black(15)
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	present()


## JE_outText centered by JE_fontCenter (TINY_FONT).
func _out_centered(text: String, y: int, colorbank: int, brightness: int) -> void:
	vga.out_text(VGA_SCREEN, 160 - vga.text_width(text, SMALL_FONT) / 2, y, text, colorbank, brightness)


## destruct.c:JE_modeSelect on the shared menu engine: one row per
## mode (destructModeName[], "Custom" if allow_custom); Enter picks,
## Escape returns MODE_NONE (-1).
func _mode_select() -> int:
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	var items := []
	for i in DESTRUCT_MODES:
		items.append(MenuEngine.item(TyrianText.L("destructMode_%d" % i), "", MenuEngine.Kind.ACTION))
	if game.allow_custom():
		items.append(MenuEngine.item(TyrianText.L("destruct_mode_custom"), "", MenuEngine.Kind.ACTION))
	menus.stack.clear()
	_mode_result = -2
	await menus.open(
		"",
		items,
		{
			"render": _draw_mode_menu,
			"activate_sfx": -1,
			"draw": false,
			"on_activate": func(sel: int) -> void: _mode_result = sel,
			"on_back": func() -> void: _mode_result = -1,
			"redraw_on_other": true,
		}
	)
	_draw_mode_menu()
	present()
	await fade_palette(15)
	# Loop: menu redrawn and presented, then a key.
	phase = "menu"
	while _mode_result == -2 and is_inside_tree():
		idle = true
		await get_tree().process_frame
	idle = false
	phase = ""
	await fade_black(15)
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	present()
	return _mode_result


func _menu_action(action: StringName) -> void:
	# Any key re-presents the menu (JE_modeSelect loop).
	menus.handle_action(action)


## destruct.c:DrawModeSelectMenu (drawn over the image, no background).
func _draw_mode_menu() -> void:
	var m: Dictionary = menus.current()
	for i in m.items.size():
		var text: String = m.items[i].label
		vga.text_shade(
			VGA_SCREEN,
			160 - vga.text_width(text, SMALL_FONT) / 2,
			82 + i * 12,
			text,
			12,
			4 if i == m.sel else 0,
			TyrianText.FULL_SHADE
		)


## destruct.c:DE_RunTick.
func _run_tick() -> int:
	_tick += 1
	_script_tick()
	for id in _capture_release.keys():
		if _tick >= _capture_release[id]:
			_keys[id] = false
			_capture_release.erase(id)
	game.tick_draw(vga)
	present()
	if game.take_first_time():
		await fade_palette(25)
	phase = "play"
	if game.tick_input(_read_moves()) == 1:
		return STATE_RELOAD
	_play_sounds(game.take_sounds())
	if _consume_key(KEY_F10):
		game.toggle_cpu(0)
	# destruct.c:1755: input_held(INPUT_GAMMA), reconfigurable Gamma key (F11 by default).
	if _consume_gamma():
		game.toggle_cpu(1)
	if _consume_key(KEY_P):
		await _pause_screen()
		_keys[_last_key] = false
	if _consume_key(KEY_F1):
		await _help_screen()
		_keys[_last_key] = false
	await _tick_delay()
	if _consume_back():
		phase = ""
		return STATE_INIT
	if _consume_key(KEY_BACKSPACE):
		phase = ""
		return STATE_RELOAD
	return STATE_CONTINUE


## setDelay(1) ... wait_delay(): one tick every 14.4 ms (nortsong.c). In
## capture, one tick per frame, and "at rest" when the image stops changing.
func _tick_delay() -> void:
	if Capture.active:
		var h := hash(vga.indices(VGA_SCREEN))
		_same_frames = _same_frames + 1 if h == _last_hash else 0
		_last_hash = h
		# --destruct-ticks: the sequence stops at the requested tick, not at rest
		# (otherwise the capture tool would record the screen far too early).
		idle = false if _tick_limit() > 0 else _same_frames >= CAPTURE_SETTLE_TICKS
		await get_tree().process_frame
		idle = false
		return
	_next_tick_ms += DELAY_PERIOD * 1000.0
	var now := Time.get_ticks_usec() / 1000.0
	if _next_tick_ms < now - 100.0:
		_next_tick_ms = now  # catch-up abandoned after a long pause
	while Time.get_ticks_usec() / 1000.0 < _next_tick_ms and is_inside_tree():
		await get_tree().process_frame


## destruct.c:JE_pauseScreen.
func _pause_screen() -> void:
	var volume: int = Music.music_volume
	Music.set_volume(volume / 2)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	_out_centered(TyrianText.L("miscText_22"), 90, 12, 5)
	present()
	phase = "wait"
	await _wait_newkey()
	phase = "play"
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	present()
	Music.set_volume(volume)


## destruct.c:JE_helpScreen.
func _help_screen() -> void:
	await fade_black(15)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	vga.clear(VGA_SCREEN, 0)
	for i in 2:
		vga.out_text(VGA_SCREEN, 100, 5 + i * 90, TyrianText.L("destructHelp_%d" % (i * 12)), 2, 4)
		vga.out_text(VGA_SCREEN, 100, 15 + i * 90, TyrianText.L("destructHelp_%d" % (i * 12 + 1)), 2, 1)
		for j in range(3, 13):
			vga.out_text(
				VGA_SCREEN,
				((j - 1) % 2) * 160 + 10,
				15 + ((j - 1) / 2) * 12 + i * 90,
				TyrianText.L("destructHelp_%d" % (i * 12 + j - 1)),
				1,
				3
			)
	vga.out_text(VGA_SCREEN, 30, 190, TyrianText.L("destructHelp_24"), 3, 4)
	present()
	await fade_palette(15)
	phase = "wait"
	await _wait_newkey()
	phase = "play"
	await fade_black(15)
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	present()
	await fade_palette(15)


## destruct.c:DE_RunTickPlaySounds: one channel per soundQueue entry (a
## new sound cuts the previous one on the same channel), channel 7 at full volume,
## the others at half volume, with no pitch variation.
func _play_sounds(queue: PackedInt32Array) -> void:
	if not Sfx.enabled or Sfx._released or DisplayServer.get_name() == "headless":
		return
	for i in range(0, queue.size(), 3):
		var stream: AudioStream = Sfx._stream(queue[i + 1])
		if stream == null:
			continue
		var p := _channels[queue[i]]
		p.stream = stream
		p.position = Vector2(queue[i + 2], 100)
		p.volume_db = Sfx._volume_db() + (0.0 if queue[i] == 7 else -6.0)
		p.play()
