## Records real `.rep` replays through the game path (F8 key armed,
## level loaded by the level scene, tyrian-sim: ReplayRecorder) to
## prove them against the C binary's replay (c-final:utils/golden/record_godot_replay.sh).
##
##   godot --path rewrite/godot res://tools/record_replays.tscn -- --rec-out=<folder> [--rec-ticks=N]
##
## Writes <folder>/godot-campaign.rep, godot-arcade.rep and godot-2p.rep: one
## episode 1 level in a normal game, in one-player Arcade and in two-player
## (same starts as the modes screen, modes_menu.gd), with a
## scripted sweep (fire held, left, right, up, down).
extends Node

const SCENARIOS := [
	# [name, modes, equipment, difficulty, section/level (episode, ]L[)]
	["godot-campaign", [], [1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 10000], 2, 1, 9],
	["godot-arcade", [0, 0, 1, 0, 0, 0, 0, 0, 1, 1], [8, 1, 3, 0, 1, 4, 2, 0, 0, 0, 0], 2, 1, 9],
	["godot-2p", [1, 0, 0, 0, 0, 0, 0, 0, 1, 1], [1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0], 2, 1, 9],
]

var _opts := {}


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--rec-"):
			var kv := arg.trim_prefix("--rec-").split("=", true, 1)
			_opts[kv[0]] = kv[1] if kv.size() > 1 else "1"
	_run.call_deferred()


## Screen sweep: fire held, directions in turn (game mask bits
## 1 up, 2 down, 4 left, 8 right, 16 fire).
func _mask(tick: int) -> int:
	var phases := [0, 4, 8, 1, 2, 4, 8, 2, 1]
	return 16 | phases[(tick / 60) % phases.size()]


func _run() -> void:
	if get_parent() != get_tree().root:
		reparent(get_tree().root)
		await get_tree().process_frame
	if not _opts.has("out"):
		printerr("record_replays: --rec-out option missing")
		Game.quit_cleanly(2)
		return
	var out := String(_opts["out"])
	DirAccess.make_dir_recursive_absolute(out)
	Capture._options["replay-dir"] = out
	var ticks := int(_opts.get("ticks", "700"))
	for s in SCENARIOS:
		if not await _record(s, out, ticks):
			Game.quit_cleanly(1)
			return
	Game.quit_cleanly(0)


func _record(s: Array, out: String, ticks: int) -> bool:
	Game.campaign_active = false
	EndlessSession.stop()
	Game.modes = PackedInt32Array(s[1])
	Game.loadout = PackedInt32Array(s[2])
	Game.difficulty = s[3]
	Game.episode = s[4]
	Game.lvl_file_num = s[5]
	Game.start_cur_loc = 0
	Game.rng_seed = 1
	ReplayRecording.armed = true
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	level.set_physics_process(false)
	if level.sim == null or not level.sim.is_recording():
		printerr(
			(
				"record_replays: %s: recording not started (%s)"
				% [s[0], level.sim.last_error() if level.sim != null else "no simulation"]
			)
		)
		return false
	var two: bool = level._two_players()
	for t in ticks:
		if level.sim.level_ended():
			break
		level.step_sim(_mask(t), _mask(t + 30) if two else 0)
	level.stop_recording()
	var path: String = level.last_recording_path
	if path.is_empty():
		printerr("record_replays: %s: no file written" % s[0])
		return false
	var target := out.path_join(String(s[0]) + ".rep")
	DirAccess.rename_absolute(path, target)
	print("record_replays: %s -> %s (%d frames)" % [s[0], target, TyrianVga.replay_header(target)[4]])
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	return true
