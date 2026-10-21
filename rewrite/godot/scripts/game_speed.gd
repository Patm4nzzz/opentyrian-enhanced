## Game speed (mainint.c:JE_inGameSetup, "Game Speed" row;
## config.c:JE_initProcessorType and JE_setNewGameSpeed; params.c --speed).
##
## The C sets the delay between two iterations of the level loop
## (tyrian2.c: "setDelay(frameCountMax)" in PIT timer units,
## nortsong.c:setDelaySpeed). The rewrite advances the simulation by one tick
## per physics frame (ADR 0003): speed becomes a number of ticks
## per second, relative to Normal (35, unchanged). The simulation itself does not
## change: only the real-time pace of the ticks changes, as in the C.
##
## Setting saved in userdata/setup.cfg (`game_speed`, 1..5, Normal = 4:
## save_json.c "game_speed"), applied during levels only
## (tyrian2.c resets Normal speed outside levels, JE_setNewGameSpeed at
## level start).
extends RefCounted

## helptext.c:gameSpeedText (gameSpeed 1..5).
const LABELS := ["spd_slug", "spd_slower", "spd_slow", "spd_normal", "spd_turbo"]
const NORMAL := 4
## config.c:JE_initProcessorType: gameSpeed 1..5 -> fastPlay.
const FAST_PLAY := [3, 4, 5, 0, 1]
## config.c:JE_setNewGameSpeed: [speed (setDelaySpeed), frameCountMax] per
## fastPlay. fastPlay 5 (Slow): pentiumMode, tyrian2.c alternates frameCountMax
## between 2 and 3 on each iteration (2.5 on average). fastPlay 2 (--speed=6,
## "hidden turbo"): smoothScroll false, no waiting (display pace).
const DELAY := [[0x4300, 2.0], [0x3000, 2.0], [0x2000, 0.0], [0x5300, 4.0], [0x4300, 3.0], [0x4300, 2.5]]
## Ticks per second of Normal in the rewrite (project.godot, ADR 0003).
const NORMAL_TICKS := 35

## params.c:speed_override (--speed=1..6), -1: command line not read yet.
static var speed_override := -1


## --speed=N: 1 slug ... 4 normal, 5 turbo, 6 hidden turbo. Debug
## override: never saved (params.c).
static func override() -> int:
	if speed_override < 0:
		speed_override = 0
		for arg in OS.get_cmdline_user_args():
			if arg.begins_with("--speed="):
				var v := int(arg.trim_prefix("--speed="))
				if v >= 1 and v <= 6:
					speed_override = v
				else:
					push_warning("--speed=%s: expected 1..6" % arg.trim_prefix("--speed="))
	return speed_override


## config.c:JE_initProcessorType: fastPlay from `game_speed`, --speed applied afterwards.
static func fast_play(game_speed: int) -> int:
	var o := override()
	if o >= 1 and o <= 5:
		return FAST_PLAY[o - 1]
	if o == 6:
		return 2
	return FAST_PLAY[clampi(game_speed, 1, 5) - 1]


## Ticks per second of a fastPlay: Normal × (Normal period / period).
## Turbo 49, Slow 28, Slower 23, Slug 14; fastPlay 2: screen refresh rate.
static func ticks_per_second(fast: int) -> int:
	var d: Array = DELAY[clampi(fast, 0, DELAY.size() - 1)]
	if d[1] == 0.0:
		var hz := DisplayServer.screen_get_refresh_rate()
		return roundi(hz) if hz > 0.0 else 60
	return roundi(NORMAL_TICKS * (0x4300 * 2.0) / (d[0] * d[1]))


## JE_setNewGameSpeed at level start or after a change.
static func apply(game_speed: int) -> void:
	Engine.physics_ticks_per_second = ticks_per_second(fast_play(game_speed))


## Outside a level: Normal pace (tyrian2.c, "Normal speed" on leaving a level).
static func restore() -> void:
	Engine.physics_ticks_per_second = int(
		ProjectSettings.get_setting("physics/common/physics_ticks_per_second", NORMAL_TICKS)
	)
