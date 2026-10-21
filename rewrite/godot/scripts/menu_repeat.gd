## Repeat of directions held on the gamepad in menus
## (joystick.c:poll_joystick and push_joysticks_as_keyboard).
##
## The C code: `joystick_repeat_delay = 300` ms. At each poll, a held
## direction is "pressed" if it has just been pushed or if the
## delay has elapsed (`joystick_delay < SDL_GetTicks()`); any new input
## re-arms the delay (`joystick_delay = SDL_GetTicks() + 300`). Action
## buttons do not repeat (edges only). Hence, for a held direction:
## an immediate press, then one every 300 ms.
##
## Here: the first press is the gamepad event itself (forwarded by
## Effects as before); `tick` returns the repeats that are due. The clock is
## passed as a parameter (milliseconds): tests simulate it.
class_name MenuRepeat
extends RefCounted

## joystick.c:joystick_repeat_delay.
const DELAY_MS := 300
## Menu directions (d-pad and left stick, TyrianControls.MENU_ACTIONS).
const DIRECTIONS: Array[StringName] = [
	&"tyrian_menu_up", &"tyrian_menu_down", &"tyrian_menu_left", &"tyrian_menu_right"
]

## Action -> held sources ({"b<dev>:<button>" or "a<dev>:<axis>": true}).
var _sources := {}
## joystick_delay: instant (ms) of the next repeat.
var _next_ms := 0


## Records a gamepad event at instant `now` (ms). Returns true if
## the event pushes a direction that was not already pushed (edge: the stick
## sends an event at every axis change, only crossing the threshold
## is a press, like direction_pressed in the C code).
func feed(event: InputEvent, now: int) -> bool:
	if not (event is InputEventJoypadButton or event is InputEventJoypadMotion):
		return false
	var new_press: bool = event is InputEventJoypadButton and (event as InputEventJoypadButton).pressed
	var new_direction := false
	for action in DIRECTIONS:
		var held: Variant = _held_by(event, action)
		if held == null:
			continue
		var sources: Dictionary = _sources.get(action, {})
		var was := not sources.is_empty()
		var key := _source_key(event)
		if held:
			sources[key] = true
		else:
			sources.erase(key)
		_sources[action] = sources
		if held and not was:
			new_press = true
			new_direction = true
	# "if new input, reset press-repeat delay"
	if new_press:
		_next_ms = now + DELAY_MS
	return new_direction


## True if the gamepad event merely extends a direction that is already
## held (stick moving beyond the threshold): it is not a new press.
func is_held_motion(event: InputEvent) -> bool:
	if not (event is InputEventJoypadMotion):
		return false
	for action in DIRECTIONS:
		if _held_by(event, action) == true and (_sources.get(action, {}) as Dictionary).has(_source_key(event)):
			return true
	return false


## Directions repeated at instant `now` (empty if the delay has not elapsed).
## The C code compares strictly (`joystick_delay < SDL_GetTicks()`), i.e. at most
## 1 ms more, under the duration of a frame: the repeat lands here at
## exactly 300 ms.
func tick(now: int) -> Array[StringName]:
	var out: Array[StringName] = []
	if now < _next_ms:
		return out
	for action in DIRECTIONS:
		if held(action):
			out.append(action)
	if not out.is_empty():
		_next_ms = now + DELAY_MS
	return out


func held(action: StringName) -> bool:
	return not (_sources.get(action, {}) as Dictionary).is_empty()


## Nothing held any more (screen change, tests).
func clear() -> void:
	_sources.clear()


## True / false if the event carries a gamepad binding for `action`, null otherwise.
static func _held_by(event: InputEvent, action: StringName) -> Variant:
	if not InputMap.has_action(action):
		return null
	for e in InputMap.action_get_events(action):
		if event is InputEventJoypadButton and e is InputEventJoypadButton:
			if e.button_index == event.button_index and (e.device == -1 or e.device == event.device):
				return event.pressed
		elif event is InputEventJoypadMotion and e is InputEventJoypadMotion:
			if e.axis == event.axis and (e.device == -1 or e.device == event.device):
				return signf(e.axis_value) * event.axis_value >= 0.5
	return null


static func _source_key(event: InputEvent) -> String:
	if event is InputEventJoypadButton:
		return "b%d:%d" % [event.device, event.button_index]
	return "a%d:%d" % [event.device, event.axis]
