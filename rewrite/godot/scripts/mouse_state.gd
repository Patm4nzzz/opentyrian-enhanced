## Global state of the menu mouse (keyboard.c:service_SDL_events_ex,
## mouse.c): pointer position in game pixels (mouse_x / mouse_y,
## 320×200), held buttons (mousedown) and `mouseInactive` toggle.
##
## keyboard.c: any key (SDL_KEYDOWN, OS repeats included, and
## the synthetic keys of push_joysticks_as_keyboard) makes the mouse
## inactive; a real movement (non-zero xrel or yrel) or a click makes it
## active. Menus only apply hover when the mouse is active (menu_tree.c:
## run_menu), and the cursor is only drawn when the mouse is active (mouse.c:
## JE_mouseStart). The wheel does not exist in the C code (no SDL_MOUSEWHEEL
## in src/): it does not change the toggle.
##
## Fed by Effects (autoload) for every event, before forwarding it to the
## game viewport.
class_name TyrianMouse
extends RefCounted

## mouse.c:mouseInactive (true at launch).
static var inactive := true
## mouse_x, mouse_y (game pixels); (-1, -1) until the mouse has moved.
static var position := Vector2i(-1, -1)
## Mask of held buttons (bit = MouseButton - 1), wheel excluded.
static var buttons := 0


## Wheel: Godot buttons 4 to 7 (up, down, left, right).
static func is_wheel(event: InputEvent) -> bool:
	return (
		event is InputEventMouseButton
		and (
			event.button_index
			in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN, MOUSE_BUTTON_WHEEL_LEFT, MOUSE_BUTTON_WHEEL_RIGHT]
		)
	)


## Vertical wheel notch: -1 (up), 1 (down), 0 otherwise. Only used
## by scrolling texts (cubes, achievements); never a click nor a menu step.
static func wheel_step(event: InputEvent) -> int:
	if not (event is InputEventMouseButton and event.pressed):
		return 0
	if event.button_index == MOUSE_BUTTON_WHEEL_UP:
		return -1
	if event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
		return 1
	return 0


## Real button (left, right, middle...) pressed: SDL_MOUSEBUTTONDOWN.
static func is_button_press(event: InputEvent) -> bool:
	return event is InputEventMouseButton and event.pressed and not is_wheel(event)


## A real button is held (the C's mousedown).
static func any_button_down() -> bool:
	return buttons != 0


## Updates the state with an event (idempotent: the same event can
## be noted twice with no additional effect).
static func note(event: InputEvent) -> void:
	if event is InputEventKey:
		if event.pressed:
			inactive = true  # keyboard.c, SDL_KEYDOWN
	elif event is InputEventJoypadButton:
		if event.pressed:
			inactive = true  # push_joysticks_as_keyboard -> SDL_KEYDOWN
	elif event is InputEventJoypadMotion:
		if absf(event.axis_value) >= 0.5:
			inactive = true
	elif event is InputEventAction:
		if event.pressed:
			inactive = true  # gamepad repeat (MenuRepeat): synthetic key
	elif event is InputEventMouseMotion:
		position = Vector2i(event.position.floor())
		# keyboard.c, SDL_MOUSEMOTION: "if (ev.motion.xrel != 0 || ev.motion.yrel != 0) mouseInactive = false".
		if event.relative != Vector2.ZERO:
			inactive = false
	elif event is InputEventMouseButton:
		position = Vector2i(event.position.floor())
		if is_wheel(event):
			return
		var bit := 1 << (int(event.button_index) - 1)
		if event.pressed:
			inactive = false  # keyboard.c, SDL_MOUSEBUTTONDOWN
			buttons |= bit
		else:
			buttons &= ~bit


## Pointer within the 320×200 image (keyboard.c: SDL_ShowCursor hidden inside).
static func inside_screen(p: Vector2) -> bool:
	return p.x >= 0 and p.y >= 0 and p.x < 320 and p.y < 200


## True if the known pointer is within the image (MouseCursor).
static func in_range() -> bool:
	return position.x >= 0 and position.y >= 0 and position.x < 320 and position.y < 200


## Pointer jump caused by a relative mode change (capture on entering a
## level, release on leaving): the C code never sees it (menus.c:115-127,
## "mouseMoved": only a real movement counts), Godot emits a real
## movement that menus would take for a hover. Ignored for 250 ms.
static var synthetic_until_ms := 0
const SYNTHETIC_WINDOW_MS := 250


## To be called just before changing the mouse mode.
static func expect_synthetic_motion() -> void:
	synthetic_until_ms = Time.get_ticks_msec() + SYNTHETIC_WINDOW_MS


static func is_synthetic_motion(event: InputEvent) -> bool:
	return event is InputEventMouseMotion and Time.get_ticks_msec() < synthetic_until_ms


## Initial state (tests: each suite restarts with the mouse inactive).
static func reset() -> void:
	inactive = true
	position = Vector2i(-1, -1)
	buttons = 0
	synthetic_until_ms = 0
