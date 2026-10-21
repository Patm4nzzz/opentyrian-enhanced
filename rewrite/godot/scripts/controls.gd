## Configurable controls (blocks 5, 14 and 15, ADR 0051).
##
## All game inputs go through shared InputMap actions: a
## screen (keyboard, gamepad, Steam Deck, future block 20.2) only has to bind
## events to these actions. The bindings are saved in
## `userdata/controls.cfg` (ConfigFile) and reloaded at startup.
##
## Correspondence with the C version:
## - player 1: `config.c:defaultKeySettings` (arrows, Space, Enter,
##   Left Ctrl, Left Alt), keys remappable as in
##   `keybindings_menu.c`;
## - gamepad: `player_movement.c:JE_playerMovement` reads `joystick[j].action[0]`
##   (fire), `action_pressed[1]` (weapon switch), `action[2]` / `action[3]`
##   (sidekicks), `action_pressed[4]` (menu), `action_pressed[5]` (pause),
##   directions via the d-pad or analog stick (`joystick.c`);
## - mouse: `config.c:defaultMouseSettings` = {1, 4, 5}: left = fire,
##   right = both sidekicks, middle = weapon switch (`keyboard.c`).
## Player 2 has no keys of its own in the C (same `keySettings`):
## its default keys here (physical ZQSD / WASD) are a choice of the
## rewrite, documented by ADR 0051.
class_name TyrianControls
extends RefCounted

static var CONFIG_PATH: String = UserData.path("controls.cfg")
## Format 2: LB / RB are menu and pause, not sidekicks (like the C).
const CONTROLS_FORMAT := 2
const LEGACY_SIDEKICK_SHOULDER := {
	"left_sidekick": JOY_BUTTON_LEFT_SHOULDER, "right_sidekick": JOY_BUTTON_RIGHT_SHOULDER
}
## Steam Deck profile (block 15), applied if SteamDeck=1 and there is no controls.cfg.
const STEAM_DECK_PROFILE := "res://config/steam_deck_controls.cfg"

## Bits of the input mask (src/golden_master.c, tyrian-sim PlayerInput).
const MASK_UP := 1 << 0
const MASK_DOWN := 1 << 1
const MASK_LEFT := 1 << 2
const MASK_RIGHT := 1 << 3
const MASK_FIRE := 1 << 4
const MASK_CHANGE_FIRE := 1 << 5
const MASK_LEFT_SIDEKICK := 1 << 6
const MASK_RIGHT_SIDEKICK := 1 << 7

## Display order and bit of each game command.
const COMMANDS := [
	["up", MASK_UP, "joym_up"],
	["down", MASK_DOWN, "joym_down"],
	["left", MASK_LEFT, "joym_left"],
	["right", MASK_RIGHT, "joym_right"],
	["fire", MASK_FIRE, "joym_fire"],
	["change_fire", MASK_CHANGE_FIRE, "joym_change_fire"],
	["left_sidekick", MASK_LEFT_SIDEKICK, "joym_left_sidekick"],
	["right_sidekick", MASK_RIGHT_SIDEKICK, "joym_right_sidekick"],
]

## Default keyboard keys (physical keycodes) per player.
const DEFAULT_KEYS := {
	1:
	{
		"up": [KEY_UP],
		"down": [KEY_DOWN],
		"left": [KEY_LEFT],
		"right": [KEY_RIGHT],
		"fire": [KEY_SPACE],
		"change_fire": [KEY_ENTER],
		"left_sidekick": [KEY_CTRL],
		"right_sidekick": [KEY_ALT]
	},
	2:
	{
		"up": [KEY_W],
		"down": [KEY_S],
		"left": [KEY_A],
		"right": [KEY_D],
		"fire": [KEY_F],
		"change_fire": [KEY_G],
		"left_sidekick": [KEY_R],
		"right_sidekick": [KEY_T]
	},
}

## Default gamepad buttons (Xbox / Steam Deck layout):
## A fire (action[0]), B weapon switch (action[1]), X / Y: sidekicks
## (action[2], action[3]). Like joystick.c:reset_joystick_assignments (first
## six buttons), LB (button 4) = in-game menu (action[4]) and RB (button 5)
## = pause (action[5]): the shoulder buttons are therefore no longer sidekicks.
const DEFAULT_PAD_BUTTONS := {
	"up": [JOY_BUTTON_DPAD_UP],
	"down": [JOY_BUTTON_DPAD_DOWN],
	"left": [JOY_BUTTON_DPAD_LEFT],
	"right": [JOY_BUTTON_DPAD_RIGHT],
	"fire": [JOY_BUTTON_A],
	"change_fire": [JOY_BUTTON_B],
	"left_sidekick": [JOY_BUTTON_X],
	"right_sidekick": [JOY_BUTTON_Y],
}
## Left stick: analog movement (separate "tyrian_stick_*" actions,
## read by read_analog), like the C's `joystick[j].analog`: speed follows
## the tilt instead of the all-or-nothing directions of the d-pad.
const DEFAULT_PAD_AXES := {
	"up": [JOY_AXIS_LEFT_Y, -1.0],
	"down": [JOY_AXIS_LEFT_Y, 1.0],
	"left": [JOY_AXIS_LEFT_X, -1.0],
	"right": [JOY_AXIS_LEFT_X, 1.0],
}
## Lower triggers: fire (right); the left trigger is bullet time (see BULLET_TIME_PAD_AXIS), handy on the Steam Deck.
const DEFAULT_PAD_TRIGGERS := {"fire": JOY_AXIS_TRIGGER_RIGHT}
## Trigger dead zone (all-or-nothing actions).
const STICK_DEADZONE := 0.35
## joystick.c: default values of joystick_axis_threshold and
## joystick_axis_reduce ("threshold" 5 and "sensitivity" 5, config.c), on
## the SDL axis scale (joystick_analog_max = 32767). Dead zone
## 5000 / 32767 ≈ 15%; beyond it, 1 unit of mouseXC per 2000.
const ANALOG_MAX := 32767
const ANALOG_THRESHOLD := 5
const ANALOG_SENSITIVITY := 5
## joystick_menu.c: sensitivity and threshold adjustable from 0 to 10.
const ANALOG_SETTING_MAX := 10
## Gamepad settings of each player (joystick_menu.c, Setup >
## Joystick; [joystick] section of controls.cfg): `joystick[j].analog`
## (proportional stick; when off, the stick gives all-or-nothing directions
## beyond mid-travel, joystick.c:poll_joystick), `sensitivity` and
## `threshold` (joystick_axis_reduce / joystick_axis_threshold). Analog
## off by default, like the C (joystick.c:507 and 533); a controls.cfg that
## enables it keeps it (ADR 0051).
static var analog_enabled := {1: false, 2: false}
static var analog_sensitivity := {1: ANALOG_SENSITIVITY, 2: ANALOG_SENSITIVITY}
static var analog_threshold := {1: ANALOG_THRESHOLD, 2: ANALOG_THRESHOLD}

## Shared menu actions (all scenes): keyboard + gamepad.
## The first seven mirror input_map.h (INPUT_MENU_UP ... CANCEL, DELETE,
## PAGE_UP/DOWN, YES, NO) and their default keys (input_map.c:
## input_map_init). The following ones name keys that the screens inherited
## from the C test directly in `lastkey_scan`: Space (= Enter in
## title_screen.c, JE_loadScreen, JE_highScoreScreen, JE_helpSystem, shop),
## Tab (game_menu.c:JE_quitRequestMsg), Delete (controls screen, specific to
## the rewrite), M / E / 2 (access to the side modes from the title, ADR
## 0050 and 0071), R (endless.c:endless_leaderboard_show, end of an Endless game).
const MENU_ACTIONS := {
	"tyrian_menu_up": [[KEY_UP], [JOY_BUTTON_DPAD_UP], [JOY_AXIS_LEFT_Y, -1.0]],
	"tyrian_menu_down": [[KEY_DOWN], [JOY_BUTTON_DPAD_DOWN], [JOY_AXIS_LEFT_Y, 1.0]],
	"tyrian_menu_left": [[KEY_LEFT], [JOY_BUTTON_DPAD_LEFT], [JOY_AXIS_LEFT_X, -1.0]],
	"tyrian_menu_right": [[KEY_RIGHT], [JOY_BUTTON_DPAD_RIGHT], [JOY_AXIS_LEFT_X, 1.0]],
	# joystick.c:push_joysticks_as_keyboard: buttons 0 and 4 (A, LB) = Enter,
	# 1 and 5 (B, RB) = Escape; Start has no role in a C menu.
	"tyrian_menu_accept": [[KEY_ENTER, KEY_KP_ENTER], [JOY_BUTTON_A, JOY_BUTTON_LEFT_SHOULDER], []],
	"tyrian_menu_back": [[KEY_ESCAPE], [JOY_BUTTON_B, JOY_BUTTON_RIGHT_SHOULDER], []],
	"tyrian_menu_delete": [[KEY_BACKSPACE], [], []],
	"tyrian_menu_page_up": [[KEY_PAGEUP], [], []],
	"tyrian_menu_page_down": [[KEY_PAGEDOWN], [], []],
	"tyrian_menu_yes": [[KEY_Y], [], []],
	"tyrian_menu_no": [[KEY_N], [], []],
	"tyrian_menu_select": [[KEY_SPACE], [], []],
	"tyrian_menu_toggle": [[KEY_TAB], [JOY_BUTTON_Y], []],
	"tyrian_menu_reset": [[KEY_DELETE], [JOY_BUTTON_X], []],
	"tyrian_menu_modes": [[KEY_M], [], []],
	"tyrian_menu_endless": [[KEY_E], [], []],
	"tyrian_menu_endless_2p": [[KEY_2], [], []],
	"tyrian_menu_retry": [[KEY_R], [], []],
	# In a level: in-game menu (Escape, Back/Select, LB = the C's action[4]; B is used
	# for the weapon switch, Start and RB for pause).
	"tyrian_level_exit": [[KEY_ESCAPE], [JOY_BUTTON_BACK, JOY_BUTTON_LEFT_SHOULDER], []],
}
## Actions returned by menu_action (tyrian_level_exit excluded: it
## duplicates Escape / Back and only makes sense in a level).
const MENU_ACTION_NAMES: Array[StringName] = [
	&"tyrian_menu_up",
	&"tyrian_menu_down",
	&"tyrian_menu_left",
	&"tyrian_menu_right",
	&"tyrian_menu_accept",
	&"tyrian_menu_back",
	&"tyrian_menu_delete",
	&"tyrian_menu_page_up",
	&"tyrian_menu_page_down",
	&"tyrian_menu_yes",
	&"tyrian_menu_no",
	&"tyrian_menu_select",
	&"tyrian_menu_toggle",
	&"tyrian_menu_reset",
	&"tyrian_menu_modes",
	&"tyrian_menu_endless",
	&"tyrian_menu_endless_2p",
	&"tyrian_menu_retry",
]
## Shortcuts for the screens' `match` statements.
const UP := &"tyrian_menu_up"
const DOWN := &"tyrian_menu_down"
const LEFT := &"tyrian_menu_left"
const RIGHT := &"tyrian_menu_right"
const ACCEPT := &"tyrian_menu_accept"
const BACK := &"tyrian_menu_back"
const DELETE := &"tyrian_menu_delete"
const YES := &"tyrian_menu_yes"
const NO := &"tyrian_menu_no"
const SELECT := &"tyrian_menu_select"
const TOGGLE := &"tyrian_menu_toggle"
const RESET := &"tyrian_menu_reset"
const MODES := &"tyrian_menu_modes"
const ENDLESS_2P := &"tyrian_menu_endless_2p"
const RETRY := &"tyrian_menu_retry"
## Actions repeated while the key is held (keyboard echo, MenuRepeat for
## the gamepad): directions and pages.
const REPEATABLE: Array[StringName] = [
	&"tyrian_menu_up",
	&"tyrian_menu_down",
	&"tyrian_menu_left",
	&"tyrian_menu_right",
	&"tyrian_menu_page_up",
	&"tyrian_menu_page_down",
]
## Key equivalent of a menu action: existing screens (handle_key,
## _unhandled_input on KEY_*) thus receive the gamepad without rewriting.
const MENU_KEY := {
	"tyrian_menu_up": KEY_UP,
	"tyrian_menu_down": KEY_DOWN,
	"tyrian_menu_left": KEY_LEFT,
	"tyrian_menu_right": KEY_RIGHT,
	"tyrian_menu_accept": KEY_ENTER,
	"tyrian_menu_back": KEY_ESCAPE,
}

## Actions shared by both players, reconfigurable on the keyboard in the
## controls screen: in-level help (F1), cancel of the capture of a new
## key (Escape), pause (F2: input_map.c:INPUT_PAUSE; P always works,
## mainint.c: "keysactive[SDL_SCANCODE_P] || input_held(INPUT_PAUSE)"),
## gamma (F11: INPUT_GAMMA) and the C's level shortcuts
## (input_map.c:enhanced_keys[]); slow motion is on Left Shift instead of Tab
## (deliberate divergence, divergence no. 31). [action name, locale key].
const HELP := &"tyrian_help"
const CAPTURE_CANCEL := &"tyrian_capture_cancel"
const PAUSE := &"tyrian_pause"
const GAMMA := &"tyrian_gamma"
const KEY_ACHIEVEMENTS := &"tyrian_key_achievements"
const KEY_STATS := &"tyrian_key_stats"
const KEY_QUICK_SAVE := &"tyrian_key_quick_save"
const KEY_DPS := &"tyrian_key_dps"
const KEY_DAMAGE_NUMBERS := &"tyrian_key_damage_numbers"
const KEY_QUICK_LOAD := &"tyrian_key_quick_load"
const KEY_REPLAY := &"tyrian_key_replay"
const KEY_SCREENSHOT := &"tyrian_key_screenshot"
const KEY_BULLET_TIME := &"tyrian_key_bullet_time"
const KEY_MUTE := &"tyrian_key_mute"
const GLOBAL_COMMANDS := [
	[HELP, "gd_controls_help"],
	[CAPTURE_CANCEL, "gd_controls_cancel"],
	[PAUSE, "gd_controls_pause"],
	[GAMMA, "mk_gamma"],
	[KEY_ACHIEVEMENTS, "kb_achievements"],
	[KEY_STATS, "kb_stats"],
	[KEY_QUICK_SAVE, "kb_quick_save"],
	[KEY_DPS, "kb_dps"],
	[KEY_DAMAGE_NUMBERS, "kb_damage_numbers"],
	[KEY_QUICK_LOAD, "kb_quick_load"],
	[KEY_REPLAY, "kb_replay"],
	[KEY_SCREENSHOT, "kb_screenshot"],
	[KEY_BULLET_TIME, "kb_bullet_time"],
	[KEY_MUTE, "kb_mute"],
]
const DEFAULT_GLOBAL_KEYS := {
	HELP: KEY_F1,
	CAPTURE_CANCEL: KEY_ESCAPE,
	PAUSE: KEY_F2,
	GAMMA: KEY_F11,
	KEY_ACHIEVEMENTS: KEY_F3,
	KEY_STATS: KEY_F4,
	KEY_QUICK_SAVE: KEY_F5,
	KEY_DPS: KEY_F6,
	KEY_DAMAGE_NUMBERS: KEY_F7,
	KEY_QUICK_LOAD: KEY_F9,
	KEY_REPLAY: KEY_F10,
	KEY_SCREENSHOT: KEY_F12,
	KEY_BULLET_TIME: KEY_SHIFT,
	KEY_MUTE: KEY_F8,
}
## Side of the default key: Left Shift only (Right Shift does nothing).
const DEFAULT_GLOBAL_LOCATION := {KEY_BULLET_TIME: KEY_LOCATION_LEFT}
## Left gamepad trigger: slow motion (alternate fire is no longer there).
const BULLET_TIME_PAD_AXIS := JOY_AXIS_TRIGGER_LEFT
## Fixed gamepad buttons of a shared action (not changed by the new
## keyboard key): Start and RB pause (the C's joystick[j].action_pressed[5]:
## button 5 = RB, player_movement.c).
const DEFAULT_GLOBAL_BUTTONS := {PAUSE: [JOY_BUTTON_START, JOY_BUTTON_RIGHT_SHOULDER]}
## input_map.c:input_scancode_reserved: Escape (menus, capture, exit box),
## F1 (in-game help) and P (pause) cannot be given to any other shortcut.
const RESERVED_KEYS := [KEY_ESCAPE, KEY_F1, KEY_P]
## Controls-screen keys that cannot become the cancel key:
## a capture started by Enter would end immediately,
## Up/Down/Tab/Delete could no longer be chosen for a command.
const CAPTURE_CANCEL_FORBIDDEN := [KEY_ENTER, KEY_KP_ENTER, KEY_UP, KEY_DOWN, KEY_TAB, KEY_DELETE]

## Godot device of each player's gamepad (-1: all).
const PAD_DEVICE := {1: 0, 2: 1}


static func action_name(player: int, command: String) -> String:
	# Player 1: historical names (game.gd ACTIONS) so as not to break anything.
	return "tyrian_%s" % command if player == 1 else "tyrian_p2_%s" % command


## Analog stick action of a player in a direction (up, down, left, right).
static func stick_action(player: int, direction: String) -> String:
	return action_name(player, "stick_" + direction)


## Creates all actions with the default bindings, then applies
## `userdata/controls.cfg` if it exists.
static func install() -> void:
	for player in [1, 2]:
		for c in COMMANDS:
			var action := action_name(player, c[0])
			if not InputMap.has_action(action):
				InputMap.add_action(action, STICK_DEADZONE)
			_set_defaults(player, c[0])
		# Zero dead zone: the C one (ANALOG_THRESHOLD) is applied by read_analog.
		for direction in DEFAULT_PAD_AXES:
			var stick := stick_action(player, direction)
			if not InputMap.has_action(stick):
				InputMap.add_action(stick, 0.0)
				InputMap.action_add_event(
					stick, _axis(DEFAULT_PAD_AXES[direction][0], DEFAULT_PAD_AXES[direction][1], PAD_DEVICE[player])
				)
	for action in MENU_ACTIONS:
		if InputMap.has_action(action):
			continue
		InputMap.add_action(action, 0.5)
		for k in MENU_ACTIONS[action][0]:
			InputMap.action_add_event(action, _key(k))
		for b in MENU_ACTIONS[action][1]:
			InputMap.action_add_event(action, _button(b, -1))
		if not MENU_ACTIONS[action][2].is_empty():
			InputMap.action_add_event(action, _axis(MENU_ACTIONS[action][2][0], MENU_ACTIONS[action][2][1], -1))
	for g in GLOBAL_COMMANDS:
		if not InputMap.has_action(g[0]):
			InputMap.add_action(g[0], 0.5)
		_set_global_default(g[0])
	if not load_config() and OS.get_environment("SteamDeck") == "1":
		# Block 15: first launch on Steam Deck, provided profile.
		load_config(STEAM_DECK_PROFILE)


static func _set_defaults(player: int, command: String) -> void:
	var action := action_name(player, command)
	InputMap.action_erase_events(action)
	for k in DEFAULT_KEYS[player].get(command, []):
		InputMap.action_add_event(action, _key(k))
	var dev: int = PAD_DEVICE[player]
	for b in DEFAULT_PAD_BUTTONS.get(command, []):
		InputMap.action_add_event(action, _button(b, dev))
	if DEFAULT_PAD_TRIGGERS.has(command):
		InputMap.action_add_event(action, _axis(DEFAULT_PAD_TRIGGERS[command], 1.0, dev))


static func _set_global_default(action: StringName) -> void:
	InputMap.action_erase_events(action)
	var ev := _key(DEFAULT_GLOBAL_KEYS[action])
	ev.location = DEFAULT_GLOBAL_LOCATION.get(action, KEY_LOCATION_UNSPECIFIED)
	InputMap.action_add_event(action, ev)
	if DEFAULT_GLOBAL_BUTTONS.has(action):
		for b in DEFAULT_GLOBAL_BUTTONS[action]:
			InputMap.action_add_event(action, _button(b, -1))
	if action == KEY_BULLET_TIME:
		InputMap.action_add_event(action, _axis(BULLET_TIME_PAD_AXIS, 1.0, -1))
		# Mouse side button 4.
		var mouse_button := InputEventMouseButton.new()
		mouse_button.button_index = MOUSE_BUTTON_XBUTTON1
		InputMap.action_add_event(action, mouse_button)


## Replaces the keyboard key of a shared action; keeps its gamepad button.
static func _replace_global_key(action: StringName, keycode: int) -> void:
	for ev in InputMap.action_get_events(action):
		if ev is InputEventKey:
			InputMap.action_erase_event(action, ev)
	var ev := _key(keycode)
	# A modifier key is the left one: the other does nothing.
	if keycode in [KEY_SHIFT, KEY_CTRL, KEY_ALT, KEY_META]:
		ev.location = KEY_LOCATION_LEFT
	InputMap.action_add_event(action, ev)


## True if `code` is already taken by a shortcut: a game key cannot
## take it (input_map.c:input_scancode_in_use). Deliberate exception: the slow-motion key (Left
## Shift) can become a game key, which then takes precedence over slow motion.
static func shortcut_key_in_use(code: int) -> bool:
	# P always pauses (mainint.c); Escape stays free as long as the capture
	# cancel key is elsewhere (it is then in GLOBAL_COMMANDS).
	if code == KEY_P:
		return true
	for g in GLOBAL_COMMANDS:
		if g[0] != KEY_BULLET_TIME and global_key(g[0]) == code:
			return true
	return false


## True if `code` is a player's game key (slow motion on Left Shift
## then gives way to the game command).
static func is_game_key(code: int) -> bool:
	for player in [1, 2]:
		for c in COMMANDS:
			for ev in InputMap.action_get_events(action_name(player, c[0])):
				if ev is InputEventKey and ev.physical_keycode == code:
					return true
	return false


## Press (edge) on the gamepad trigger of a shared action:
## `down` remembers the previous state, returns true when crossing the threshold.
static func pad_axis_edge(event: InputEvent, action: StringName, down: bool) -> Array:
	if not (event is InputEventJoypadMotion) or not InputMap.has_action(action) or not event.is_action(action):
		return [down, false]
	var now: bool = event.is_action_pressed(action)
	return [now, now and not down]


static func reset_defaults() -> void:
	for player in [1, 2]:
		for c in COMMANDS:
			_set_defaults(player, c[0])
		_reset_pad_settings(player)
	for g in GLOBAL_COMMANDS:
		_set_global_default(g[0])
	_set_menu_pad_default()


## joystick.c:reset_joystick_assignments for a player's gamepad:
## default buttons, triggers and stick, analog, sensitivity and threshold
## defaults. Keyboard keys and shared commands do not change.
static func reset_pad(player: int) -> void:
	for c in COMMANDS:
		var action := action_name(player, c[0])
		for ev in InputMap.action_get_events(action):
			if ev is InputEventJoypadButton or ev is InputEventJoypadMotion:
				InputMap.action_erase_event(action, ev)
		var dev: int = PAD_DEVICE[player]
		for b in DEFAULT_PAD_BUTTONS.get(c[0], []):
			InputMap.action_add_event(action, _button(b, dev))
		if DEFAULT_PAD_TRIGGERS.has(c[0]):
			InputMap.action_add_event(action, _axis(DEFAULT_PAD_TRIGGERS[c[0]], 1.0, dev))
	_reset_pad_settings(player)


static func _reset_pad_settings(player: int) -> void:
	for direction in DEFAULT_PAD_AXES:
		var stick := stick_action(player, direction)
		if InputMap.has_action(stick):
			InputMap.action_erase_events(stick)
			InputMap.action_add_event(
				stick, _axis(DEFAULT_PAD_AXES[direction][0], DEFAULT_PAD_AXES[direction][1], PAD_DEVICE[player])
			)
	analog_enabled[player] = false
	analog_sensitivity[player] = ANALOG_SENSITIVITY
	analog_threshold[player] = ANALOG_THRESHOLD


## Gamepad "menu" button (joystick_menu.c: joym_menu, action[4]):
## tyrian_level_exit, Back by default.
static func _set_menu_pad_default() -> void:
	if not InputMap.has_action(MENU_PAD_ACTION):
		return
	for ev in InputMap.action_get_events(MENU_PAD_ACTION):
		if ev is InputEventJoypadButton or ev is InputEventJoypadMotion:
			InputMap.action_erase_event(MENU_PAD_ACTION, ev)
	for b in MENU_ACTIONS[MENU_PAD_ACTION][1]:
		InputMap.action_add_event(MENU_PAD_ACTION, _button(b, -1))


## Key help (divergence no. 57): suffix of the keyhelp_<suffix> key and
## action whose key is written in place of "%s" (empty: fixed text).
const HELP_KEYS := [
	["pause", PAUSE],
	["bullet_time", KEY_BULLET_TIME],
	["quick_save", KEY_QUICK_SAVE],
	["quick_load", KEY_QUICK_LOAD],
	["replay", KEY_REPLAY],
	["screenshot", KEY_SCREENSHOT],
	["achievements", KEY_ACHIEVEMENTS],
	["stats", KEY_STATS],
	["dps", KEY_DPS],
	["damage_numbers", KEY_DAMAGE_NUMBERS],
	["gamma", GAMMA],
	["mute", KEY_MUTE],
	["fullscreen", &""],
	["mouse", &""],
	["wheel", &""],
	["endless", &""],
]


## Help line for a key, with the key actually bound (never hardcoded).
static func help_line(suffix: String, action: StringName) -> String:
	var text := TyrianText.L("keyhelp_" + suffix)
	if action == &"":
		return text
	var code := global_key(action)
	# Slow motion is on Left Shift (the other Shift does nothing).
	var name := TyrianText.L("key_lshift") if code == KEY_SHIFT else key_name(code)
	return text.replace("%s", name)


## Keyboard key of a shared action (help, capture cancel).
static func global_key(action: StringName) -> int:
	for ev in InputMap.action_get_events(action):
		if ev is InputEventKey:
			return ev.physical_keycode
	return KEY_NONE


## True if `event` is a press (without repeat) on the shared action
## `action`: its keyboard key, or its gamepad button (Start for pause).
static func global_pressed(event: InputEvent, action: StringName) -> bool:
	if not InputMap.has_action(action):
		install()
	if event is InputEventJoypadButton:
		return event.pressed and event.is_action_pressed(action, false, true)
	return event is InputEventKey and event.pressed and not event.echo and event.is_action_pressed(action, false, false)


## New key of a shared action. Refused (false) if it would make
## a command unreachable: for the capture cancel, a game key of
## either player or a controls-screen key
## (CAPTURE_CANCEL_FORBIDDEN); for help, a game key (it
## would toggle help while playing). The two actions do not share a key.
static func rebind_global(action: StringName, keycode: int) -> bool:
	if global_key_conflict(action, keycode):
		return false
	_replace_global_key(action, keycode)
	return true


static func global_key_conflict(action: StringName, keycode: int) -> bool:
	if action == CAPTURE_CANCEL and keycode in CAPTURE_CANCEL_FORBIDDEN:
		return true
	# input_scancode_reserved: valid for all except help (F1) and cancel (Escape).
	if action != HELP and action != CAPTURE_CANCEL and keycode in RESERVED_KEYS:
		return true
	for g in GLOBAL_COMMANDS:
		if g[0] != action and global_key(g[0]) == keycode:
			return true
	for player in [1, 2]:
		for c in COMMANDS:
			for ev in InputMap.action_get_events(action_name(player, c[0])):
				if ev is InputEventKey and ev.physical_keycode == keycode:
					return true
	return false


static func _key(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.physical_keycode = code
	return ev


static func _button(b: int, device: int) -> InputEventJoypadButton:
	var ev := InputEventJoypadButton.new()
	ev.button_index = b
	ev.device = device
	return ev


static func _axis(axis: int, value: float, device: int) -> InputEventJoypadMotion:
	var ev := InputEventJoypadMotion.new()
	ev.axis = axis
	ev.axis_value = value
	ev.device = device
	return ev


## Mask of a player (1 or 2) from its actions. With analog off
## (joystick[j].analog false), the stick sets the direction bits beyond
## mid-travel (joystick.c:poll_joystick, analog_direction > max / 2).
static func read_mask(player: int) -> int:
	var mask := 0
	var analog: bool = analog_enabled.get(player, false)
	for c in COMMANDS:
		var action := action_name(player, c[0])
		if not Input.is_action_pressed(action):
			continue
		# Analog active: the gamepad d-pad goes through read_analog (stick
		# fully pushed, joystick.c:check_assigned), not through the keyboard's fixed step.
		if analog and DEFAULT_PAD_AXES.has(c[0]) and _pad_buttons_held(action, player) and not _keys_held(action):
			continue
		mask |= c[1]
	if not analog_enabled.get(player, false):
		for c in COMMANDS:
			if DEFAULT_PAD_AXES.has(c[0]) and Input.get_action_raw_strength(stick_action(player, c[0])) > 0.5:
				mask |= c[1]
	return mask


## joystick.c:joystick_axis_reduce (with joystick_axis_threshold): SDL axis
## value (-32767..32767) -> displacement added to mouseXC / mouseYC, according to
## the player's gamepad threshold and sensitivity.
## Integer division truncated toward zero, as in C.
static func analog_axis_reduce(value: int, player := 1) -> int:
	var threshold: int = analog_threshold.get(player, ANALOG_THRESHOLD)
	var sensitivity: int = analog_sensitivity.get(player, ANALOG_SENSITIVITY)
	var magnitude := absi(value)
	if magnitude <= threshold * 1000:
		return 0
	magnitude -= threshold * 1000
	return signi(value) * (magnitude / (3000 - 200 * sensitivity))


## Analog stick of a player for the tick (player_movement.c, branch
## `joystick[j].analog`): x = right - left, y = down - up, reduced;
## zero when analog is off (the stick then goes through read_mask).
static func read_analog(player: int) -> Vector2i:
	if not analog_enabled.get(player, false):
		return Vector2i.ZERO
	var x := _analog_direction(player, "right") - _analog_direction(player, "left")
	var y := _analog_direction(player, "down") - _analog_direction(player, "up")
	return Vector2i(analog_axis_reduce(int(x * ANALOG_MAX), player), analog_axis_reduce(int(y * ANALOG_MAX), player))


## joystick.c:poll_joystick, analog_direction[d] = check_assigned(...): the
## strongest of the direction's assignments, the axis at its value, a button or the
## d-pad fully pushed (joystick_analog_max).
static func _analog_direction(player: int, direction: String) -> float:
	var strength := Input.get_action_raw_strength(stick_action(player, direction))
	if _pad_buttons_held(action_name(player, direction), player):
		strength = 1.0
	return strength


## A gamepad button (d-pad included) bound to the action is held down.
static func _pad_buttons_held(action: String, player: int) -> bool:
	if not InputMap.has_action(action):
		return false
	for ev in InputMap.action_get_events(action):
		if ev is InputEventJoypadButton:
			var device: int = ev.device if ev.device >= 0 else PAD_DEVICE[player]
			if Input.is_joy_button_pressed(device, ev.button_index):
				return true
	return false


## A keyboard key bound to the action is held down.
static func _keys_held(action: String) -> bool:
	for ev in InputMap.action_get_events(action):
		if ev is InputEventKey:
			if (
				(ev.physical_keycode != KEY_NONE and Input.is_physical_key_pressed(ev.physical_keycode))
				or (ev.keycode != KEY_NONE and Input.is_key_pressed(ev.keycode))
			):
				return true
	return false


# --- Gamepad assignments (joystick_menu.c, joystick.c) -----------------

## Gamepad "menu" action (joystick_menu.c: joym_menu).
const MENU_PAD_ACTION := "tyrian_level_exit"
## joystick_menu.c:joystick_items: ten assignment rows, in the order of
## joystick[j].assignment (up, right, down, left, then the six actions).
const PAD_ROWS := [
	"up", "right", "down", "left", "fire", "change_fire", "left_sidekick", "right_sidekick", "menu", "pause"
]
## Slots per row (COUNTOF(joystick[j].assignment[a]) = 2).
const PAD_SLOTS := 2
## joystick.c:detect_joystick_assignment: axis displacement accepted beyond
## two thirds of the travel.
const PAD_AXIS_DETECT := 2.0 / 3.0


## Actions that carry the assignments of a row: [buttons, axes].
## Directions: buttons on the command, axes on the stick; menu and pause:
## shared actions (any gamepad).
static func _pad_row_actions(player: int, row: int) -> Array:
	var name: String = PAD_ROWS[row]
	if name == "menu":
		return [MENU_PAD_ACTION, MENU_PAD_ACTION]
	if name == "pause":
		return [PAUSE, PAUSE]
	if DEFAULT_PAD_AXES.has(name):
		return [action_name(player, name), stick_action(player, name)]
	return [action_name(player, name), action_name(player, name)]


static func _pad_row_device(player: int, row: int) -> int:
	return -1 if PAD_ROWS[row] in ["menu", "pause"] else PAD_DEVICE[player]


## Gamepad assignments of a row, axes then buttons.
static func pad_bindings(player: int, row: int) -> Array:
	var acts := _pad_row_actions(player, row)
	var out: Array = []
	if InputMap.has_action(acts[1]):
		for ev in InputMap.action_get_events(acts[1]):
			if ev is InputEventJoypadMotion:
				out.append(ev)
	if InputMap.has_action(acts[0]):
		for ev in InputMap.action_get_events(acts[0]):
			if ev is InputEventJoypadButton:
				out.append(ev)
	return out


## joystick.c:assignment_to_code ("AX 2-", "BTN 1"), 1-based numbers.
static func pad_code(ev: InputEvent) -> String:
	if ev is InputEventJoypadMotion:
		# gdlint: ignore=max-line-length
		return "AX %d%s" % [ev.axis + 1, "-" if ev.axis_value < 0.0 else "+"]  # locale-exempt: codes of joystick.c:assignment_to_code
	if ev is InputEventJoypadButton:
		return "BTN %d" % (ev.button_index + 1)  # locale-exempt: codes of joystick.c:assignment_to_code
	return ""


## joystick.c:joystick_assignments_to_string.
static func pad_row_text(player: int, row: int) -> String:
	return ", ".join(pad_bindings(player, row).map(func(ev: InputEvent) -> String: return pad_code(ev)))


## Gamepad input accepted by the assignment capture
## (joystick.c:detect_joystick_assignment): button pressed or axis pushed
## beyond two thirds; null otherwise.
static func pad_capture_event(event: InputEvent) -> InputEvent:
	if event is InputEventJoypadButton and event.pressed:
		return _button(event.button_index, 0)
	if event is InputEventJoypadMotion and absf(event.axis_value) > PAD_AXIS_DETECT:
		return _axis(event.axis, -1.0 if event.axis_value < 0.0 else 1.0, 0)
	return null


## joystick.c:joystick_assign_toggle: an assignment already present is
## removed; otherwise added in a free slot, or in place of the
## oldest one when both are taken.
static func pad_assign_toggle(player: int, row: int, detected: InputEvent) -> void:
	var acts := _pad_row_actions(player, row)
	var device := _pad_row_device(player, row)
	var is_axis := detected is InputEventJoypadMotion
	var target: String = acts[1] if is_axis else acts[0]
	for ev in pad_bindings(player, row):
		var same: bool = (
			(
				ev is InputEventJoypadMotion
				and is_axis
				and ev.axis == detected.axis
				and signf(ev.axis_value) == signf(detected.axis_value)
			)
			or (ev is InputEventJoypadButton and not is_axis and ev.button_index == detected.button_index)
		)
		if same:
			InputMap.action_erase_event(acts[1] if ev is InputEventJoypadMotion else acts[0], ev)
			return
	var current := pad_bindings(player, row)
	if current.size() >= PAD_SLOTS:
		var oldest: InputEvent = current[0]
		InputMap.action_erase_event(acts[1] if oldest is InputEventJoypadMotion else acts[0], oldest)
	if is_axis:
		InputMap.action_add_event(target, _axis(detected.axis, detected.axis_value, device))
	else:
		InputMap.action_add_event(target, _button(detected.button_index, device))


## Replaces the keyboard key of a command (options screen); keeps the gamepad bindings.
static func rebind_key(player: int, command: String, keycode: int) -> void:
	var action := action_name(player, command)
	for ev in InputMap.action_get_events(action):
		if ev is InputEventKey:
			InputMap.action_erase_event(action, ev)
	InputMap.action_add_event(action, _key(keycode))


## Replaces the gamepad button of a command; keeps keyboard and axes.
static func rebind_button(player: int, command: String, button: int) -> void:
	var action := action_name(player, command)
	for ev in InputMap.action_get_events(action):
		if ev is InputEventJoypadButton:
			InputMap.action_erase_event(action, ev)
	InputMap.action_add_event(action, _button(button, PAD_DEVICE[player]))


## input_map.c:key_locale_names: keys whose name is translated (locale keys
## key_*). Shift, Ctrl, Alt and Windows are not there: their keys
## name a side ("Left Ctrl") while a Godot binding applies to
## both; their short Godot name is displayed.
const KEY_LOCALE_NAMES := {
	KEY_UP: "key_up",
	KEY_DOWN: "key_down",
	KEY_LEFT: "key_left",
	KEY_RIGHT: "key_right",
	KEY_ENTER: "key_return",
	KEY_ESCAPE: "key_escape",
	KEY_SPACE: "key_space",
	KEY_BACKSPACE: "key_backspace",
	KEY_TAB: "key_tab",
	KEY_MENU: "key_application",
	KEY_DELETE: "key_delete",
	KEY_INSERT: "key_insert",
	KEY_HOME: "key_home",
	KEY_END: "key_end",
	KEY_PAGEUP: "key_pageup",
	KEY_PAGEDOWN: "key_pagedown",
	KEY_CAPSLOCK: "key_capslock",
	KEY_NUMLOCK: "key_numlock",
	KEY_SCROLLLOCK: "key_scrolllock",
	KEY_PRINT: "key_printscreen",
	KEY_PAUSE: "key_pause",
	KEY_KP_ENTER: "key_kp_enter",
	KEY_KP_ADD: "key_kp_plus",
	KEY_KP_SUBTRACT: "key_kp_minus",
	KEY_KP_MULTIPLY: "key_kp_multiply",
	KEY_KP_DIVIDE: "key_kp_divide",
	KEY_KP_PERIOD: "key_kp_period",
	KEY_KP_0: "key_kp_0",
	KEY_KP_1: "key_kp_1",
	KEY_KP_2: "key_kp_2",
	KEY_KP_3: "key_kp_3",
	KEY_KP_4: "key_kp_4",
	KEY_KP_5: "key_kp_5",
	KEY_KP_6: "key_kp_6",
	KEY_KP_7: "key_kp_7",
	KEY_KP_8: "key_kp_8",
	KEY_KP_9: "key_kp_9",
}


## input_map.c:input_key_name_localized: key name in the current
## language, Godot's English name for the others (letters, F1...).
static func key_name(keycode: int) -> String:
	var k: String = KEY_LOCALE_NAMES.get(keycode, "")
	if k != "":
		var s := TyrianText.L(k)
		if s != k and s != "":
			return s
	# gdlint: ignore=max-line-length
	return OS.get_keycode_string(keycode)  # locale-exempt: letters, digits, F1... without a translated name (fallback of input_key_name_localized)


## Short text of a command's bindings ("Space / A").
static func describe(player: int, command: String) -> String:
	var keys: PackedStringArray = []
	var pads: PackedStringArray = []
	for ev in InputMap.action_get_events(action_name(player, command)):
		if ev is InputEventKey:
			keys.append(key_name(ev.physical_keycode))
		elif ev is InputEventJoypadButton:
			pads.append(pad_code(ev))
	return (
		"%s / %s" % [", ".join(keys) if not keys.is_empty() else "-", ", ".join(pads) if not pads.is_empty() else "-"]
	)


static func save_config(path := CONFIG_PATH) -> int:
	var cfg := ConfigFile.new()
	cfg.set_value("format", "controls", CONTROLS_FORMAT)
	for player in [1, 2]:
		for c in COMMANDS:
			var keys: Array = []
			var buttons: Array = []
			for ev in InputMap.action_get_events(action_name(player, c[0])):
				if ev is InputEventKey:
					keys.append(ev.physical_keycode)
				elif ev is InputEventJoypadButton:
					buttons.append(ev.button_index)
			cfg.set_value("player%d" % player, c[0] + "_keys", keys)
			cfg.set_value("player%d" % player, c[0] + "_buttons", buttons)
			cfg.set_value("player%d" % player, c[0] + "_axes", _axes_of(action_name(player, c[0])))
			if DEFAULT_PAD_AXES.has(c[0]):
				cfg.set_value("player%d" % player, "stick_" + c[0] + "_axes", _axes_of(stick_action(player, c[0])))
		cfg.set_value("joystick", "analog_%d" % player, analog_enabled[player])
		cfg.set_value("joystick", "sensitivity_%d" % player, analog_sensitivity[player])
		cfg.set_value("joystick", "threshold_%d" % player, analog_threshold[player])
	for g in GLOBAL_COMMANDS:
		cfg.set_value("global", String(g[0]).trim_prefix("tyrian_") + "_key", global_key(g[0]))
	cfg.set_value("global", "pause_buttons", _buttons_of(PAUSE))
	cfg.set_value("global", "menu_buttons", _buttons_of(MENU_PAD_ACTION))
	return cfg.save(path)


## Axes of an action, flattened into [axis, direction] pairs.
static func _axes_of(action: String) -> Array:
	var out: Array = []
	if InputMap.has_action(action):
		for ev in InputMap.action_get_events(action):
			if ev is InputEventJoypadMotion:
				out.append_array([ev.axis, 1 if ev.axis_value > 0.0 else -1])
	return out


static func _buttons_of(action: String) -> Array:
	var out: Array = []
	if InputMap.has_action(action):
		for ev in InputMap.action_get_events(action):
			if ev is InputEventJoypadButton:
				out.append(ev.button_index)
	return out


## Replaces the axes (or buttons) of an action with those from the file.
static func _replace_pad_events(action: String, axes: bool, values: Array, device: int) -> void:
	if not InputMap.has_action(action):
		return
	for ev in InputMap.action_get_events(action):
		if (ev is InputEventJoypadMotion) if axes else (ev is InputEventJoypadButton):
			InputMap.action_erase_event(action, ev)
	if axes:
		for k in range(0, values.size() - 1, 2):
			InputMap.action_add_event(action, _axis(int(values[k]), float(values[k + 1]), device))
	else:
		for b in values:
			InputMap.action_add_event(action, _button(int(b), device))


## Applies a bindings file (userdata/controls.cfg or provided profile,
## e.g. `res://config/steam_deck_controls.cfg`). Axes stay at their defaults.
static func load_config(path := CONFIG_PATH) -> bool:
	var cfg := ConfigFile.new()
	if cfg.load(path) != OK:
		return false
	# joystick.c:499-503: buttons 4 and 5 are menu and pause, never a sidekick.
	# A file from before this format keeps LB / RB as sidekicks: remove them.
	var legacy_pad := int(cfg.get_value("format", "controls", 1)) < CONTROLS_FORMAT
	for player in [1, 2]:
		var section := "player%d" % player
		for c in COMMANDS:
			var action := action_name(player, c[0])
			if cfg.has_section_key(section, c[0] + "_keys"):
				for ev in InputMap.action_get_events(action):
					if ev is InputEventKey:
						InputMap.action_erase_event(action, ev)
				for k in cfg.get_value(section, c[0] + "_keys", []):
					InputMap.action_add_event(action, _key(int(k)))
			if cfg.has_section_key(section, c[0] + "_buttons"):
				for ev in InputMap.action_get_events(action):
					if ev is InputEventJoypadButton:
						InputMap.action_erase_event(action, ev)
				for b in cfg.get_value(section, c[0] + "_buttons", []):
					if legacy_pad and int(b) == LEGACY_SIDEKICK_SHOULDER.get(c[0], -1):
						continue
					InputMap.action_add_event(action, _button(int(b), PAD_DEVICE[player]))
			if cfg.has_section_key(section, c[0] + "_axes"):
				_replace_pad_events(action, true, cfg.get_value(section, c[0] + "_axes", []), PAD_DEVICE[player])
			if cfg.has_section_key(section, "stick_" + c[0] + "_axes"):
				_replace_pad_events(
					stick_action(player, c[0]),
					true,
					cfg.get_value(section, "stick_" + c[0] + "_axes", []),
					PAD_DEVICE[player]
				)
		# save_json.c:apply_joystick_entry: analog, sensitivity and threshold clamped to 0..10.
		analog_enabled[player] = bool(cfg.get_value("joystick", "analog_%d" % player, analog_enabled[player]))
		analog_sensitivity[player] = clampi(
			int(cfg.get_value("joystick", "sensitivity_%d" % player, analog_sensitivity[player])), 0, ANALOG_SETTING_MAX
		)
		analog_threshold[player] = clampi(
			int(cfg.get_value("joystick", "threshold_%d" % player, analog_threshold[player])), 0, ANALOG_SETTING_MAX
		)
	for g in GLOBAL_COMMANDS:
		var k := String(g[0]).trim_prefix("tyrian_") + "_key"
		if cfg.has_section_key("global", k):
			var code := int(cfg.get_value("global", k))
			# Old pause default (P): P always works, F2 becomes the key.
			if g[0] == PAUSE and code == KEY_P:
				code = KEY_F2
			_replace_global_key(g[0], code)
	if cfg.has_section_key("global", "pause_buttons"):
		var pause_buttons: Array = cfg.get_value("global", "pause_buttons", [])
		if legacy_pad and not pause_buttons.has(JOY_BUTTON_RIGHT_SHOULDER):
			pause_buttons = pause_buttons + [JOY_BUTTON_RIGHT_SHOULDER]
		_replace_pad_events(PAUSE, false, pause_buttons, -1)
	if cfg.has_section_key("global", "menu_buttons"):
		_replace_pad_events(MENU_PAD_ACTION, false, cfg.get_value("global", "menu_buttons", []), -1)
	return true


## Menu action of an event (keyboard or gamepad), &"" if none.
## Menu screens: `_unhandled_input` -> menu_action -> `handle_action`
## (block 20.2, ADR 0120); actions are created if needed (verification tools
## launched without the Game autoload).
static func menu_action(event: InputEvent) -> StringName:
	if not InputMap.has_action(MENU_ACTION_NAMES[0]):
		install()
	# Keyboard: modifiers ignored, like the old physical_keycode test.
	var exact := not (event is InputEventKey)
	for action in MENU_ACTION_NAMES:
		# OS repeat (echo) accepted for directions and pages
		# only: keyboard.c does not filter repeated SDL_KEYDOWN, but
		# a repeated Enter or Escape would open / close menus
		# in a cascade (joystick.c does not repeat confirm / cancel either).
		if event.is_action_pressed(action, action in REPEATABLE, exact):
			return action
	return &""


## Synthetic keyboard event (capture tool --keys, tests).
static func key_event(code: int) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.physical_keycode = code as Key
	ev.keycode = code as Key
	ev.pressed = true
	return ev


## Gamepad event -> equivalent key, for screens without `handle_action`
## (effects.gd passes it to them as a key); 0 if none.
static func menu_key_for(event: InputEvent) -> int:
	if not (event is InputEventJoypadButton or event is InputEventJoypadMotion):
		return 0
	for action in MENU_KEY:
		if event.is_action_pressed(action, false, true):
			return MENU_KEY[action]
	return 0
