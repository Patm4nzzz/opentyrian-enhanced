## Development keys (dev_console.c), active only with
## --devmode (params.c): Ctrl+Shift+letter during a level.
##   G: god mode (constantPlay: permanent fire, ship drift)
##   H: player 1 armor to 28
##   C: +10000 credits
##   N: successful level end (debriefing, progression)
##   U: all achievements unlocked and saved
## The effects go through TyrianSim (tyrian-sim, dev module) and are played
## on the next tick, where the C calls dev_console_poll. Without --devmode,
## nothing is read: simulation identical to the C.
extends RefCounted

## dev_console.c: "player[0].cash += 10000".
const CASH := 10000

## dev_console_available (--devmode), -1: command line not read yet.
static var _available := -1
## params.c:constantPlay: C global, kept from one level to the next.
static var god_mode := false


static func available() -> bool:
	if _available < 0:
		_available = 1 if OS.get_cmdline_user_args().has("--devmode") else 0
	return _available == 1


## Tests: enables or disables the keys like --devmode.
static func set_available(on: bool) -> void:
	_available = 1 if on else 0


## dev_console.c:dev_console_poll: Ctrl and Shift held, rising edge of the
## letter (event without repeat). True if a cheat was applied.
static func handle(event: InputEvent, level: Node) -> bool:
	if not available() or not (event is InputEventKey) or not event.pressed or event.echo:
		return false
	if not (event.ctrl_pressed and event.shift_pressed):
		return false
	var sim: Object = level.get("sim")
	if sim == null or not sim.has_method("dev_toggle_god_mode"):
		return false
	match event.physical_keycode:
		KEY_G:
			god_mode = sim.dev_toggle_god_mode()
			print("dev: god mode %s" % ("ON" if god_mode else "OFF"))
		KEY_H:
			sim.dev_restore_armor()
			print("dev: armor restored to 28")
		KEY_C:
			sim.dev_add_cash(CASH)
			print("dev: +10000 credits")
		KEY_N:
			sim.dev_complete_level()
			print("dev: level-end signal sent")
		KEY_U:
			sim.dev_unlock_all_achievements(level.call("_ach_dat_path"))
			print("dev: unlocked all achievements")
		_:
			return false
	return true
