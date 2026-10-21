## In-game Godot frames, one per tick, for the side-by-side video
## (rewrite/scripts/side_by_side_video.sh, docs/video.md).
##
##   godot --rendering-driver opengl3 --path rewrite/godot res://tools/video_frames.tscn -- \
##       --video-replay=<file.gm> --video-from=<C tick> --video-to=<C tick> --video-out=<folder>
##
## Replays the inputs of a golden master of the C version exactly like
## tools/capture.gd (--replay: header, equipment, seed, helpers, masks and
## mouse read by Capture._load_replay), then saves the window image
## (320x200 scaled down from the window x3) after each tick, from --video-from to
## --video-to inclusive. Tick mapping: the image that the instrumented
## C build writes at tick T (GOLDEN_FRAMES, index.txt) is that of the
## simulation after T + 1 Godot steps (Sim::new plays the top of tick 0; same
## rule as capture.gd in visual_capture_level.sh). Measured on e1 TYRIAN
## tick 4221: T / T + 1 / T + 2 steps give 31,369 / 5 / 26,687 different
## pixels over the play area. The file <folder>/<T as 6 digits>.png
## is the state after T + 1 steps.
## Read-only tool: modifies neither the simulation nor capture.gd.
extends Node

var _opts := {}


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--video-"):
			var kv := arg.trim_prefix("--video-").split("=", true, 1)
			_opts[kv[0]] = kv[1] if kv.size() > 1 else "1"
	_run.call_deferred()


func _run() -> void:
	# Effects moves the main scene into its game viewport, which is emptied on
	# every goto_scene: reattach to the root (see test/campaign_test.gd).
	if get_parent() != get_tree().root:
		reparent(get_tree().root)
		await get_tree().process_frame
	for k in ["replay", "from", "to", "out"]:
		if not _opts.has(k):
			printerr("video_frames: --video-%s option missing" % k)
			Game.quit_cleanly(2)
			return
	var from := int(_opts["from"])
	var to := int(_opts["to"])
	var out := String(_opts["out"])
	DirAccess.make_dir_recursive_absolute(out)
	# Capture mode: the level scene only advances through advance().
	Capture.active = true
	if not Capture._load_replay(String(_opts["replay"])):
		Game.quit_cleanly(1)
		return
	var scene: Node = Game.goto_scene(Capture.SCENES["level"])
	await get_tree().process_frame
	if scene.sim == null:
		printerr("video_frames: simulation missing (GDExtension?)")
		Game.quit_cleanly(1)
		return
	# Fast-forward by `from` + 1 steps, without frames; then one frame per step.
	var steps := from + 1
	scene.advance(steps, func(i): return Capture._mask_at(i), func(i): return Capture._mouse_at(i))
	var written := 0
	for t in range(from, to + 1):
		if t > from:
			var base := steps
			scene.advance(1, func(i): return Capture._mask_at(base + i), func(i): return Capture._mouse_at(base + i))
			steps += 1
		await RenderingServer.frame_post_draw
		await RenderingServer.frame_post_draw
		var img := get_viewport().get_texture().get_image()
		if img.get_width() != 320:
			img.resize(320, 200, Image.INTERPOLATE_NEAREST)
		if img.save_png(out.path_join("%06d.png" % t)) != OK:
			printerr("video_frames: cannot write ", out)
			Game.quit_cleanly(1)
			return
		written += 1
		if scene.sim.level_ended():
			print("video_frames: level end at C tick %d" % t)
			break
	var p: PackedInt32Array = scene.sim.player(0)
	var row: PackedStringArray = (
		Capture._replay_rows[steps - 1] if steps - 1 < Capture._replay_rows.size() else PackedStringArray()
	)
	var same: bool = (
		row.size() > 7 and int(row[7]) == scene.sim.cur_loc() and int(row[2]) == p[0] and int(row[3]) == p[1]
	)
	print(
		(
			"video_frames: %d frames (C ticks %d..%d) -> %s; final state cur_loc=%d x=%d y=%d: %s"
			% [
				written,
				from,
				from + written - 1,
				out,
				scene.sim.cur_loc(),
				p[0],
				p[1],
				"IDENTICAL to the C" if same else "DIFFERENT from the C"
			]
		)
	)
	Game.quit_cleanly(0 if same else 1)
