## Performance benchmark (autoload `Bench`, inactive without an argument).
##
##   godot --path rewrite/godot -- --bench=<output.json> [--bench-counts=0,500,2000]
##         [--bench-seconds=6] [--bench-warmup=1] [--bench-legacy-shots]
##
## For each enemy shot count N, plays the default level (episode 1,
## TYRIAN) at the original rate (35 ticks/s, _physics_process) with N shots
## kept alive by the tyrian-sim test API (`bench_fill_enemy_shots`), with
## vsync off and uncapped rendering. Measures on every frame:
##   - sim_us     : bench_fill + Rust core step (per simulation tick);
##   - refresh_us : reading the state and updating the nodes (level.gd:_refresh);
##   - shots_us   : of which projectiles only (level.gd:_update_shots);
##   - frame_ms   : actual interval between two frames (_process);
##   - render_cpu_ms / render_gpu_ms : RenderingServer, root viewport and
##     game viewport (engine measurement).
## --bench-legacy-shots : projectiles through the old GDScript path
## (level.gd:_update_shots_from, one instance at a time), for before/after.
## Writes a JSON (one entry per N) and one summary line per N to stdout.
extends Node

var active := false
## Autopilot helpers during the measurement (read by the level scene, ADR 0125).
var replay_assist := false
var _out := ""
var _counts: Array[int] = [0, 60, 500, 1000, 2000, 5000, 10000]
var _seconds := 6.0
var _warmup := 1.0
var _legacy_shots := false
var _probe := false

var _scene: Node
var _seed := 42
var _n := 0
var _phase_start := 0
var _measuring := false
var _sim_us: Array[int] = []
var _refresh_us: Array[int] = []
var _shots_us: Array[int] = []
var _frame_us: Array[int] = []
var _render_cpu: Array[float] = []
var _render_gpu: Array[float] = []
var _last_frame := 0
var _ticks := 0
var _frames := 0
var _shots_seen := 0
var _results: Array = []


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--bench="):
			_out = arg.trim_prefix("--bench=")
			active = true
		elif arg.begins_with("--bench-counts="):
			_counts.clear()
			for c in arg.trim_prefix("--bench-counts=").split(",", false):
				_counts.append(int(c))
		elif arg.begins_with("--bench-seconds="):
			_seconds = float(arg.trim_prefix("--bench-seconds="))
		elif arg == "--bench-probe":
			_probe = true
		elif arg == "--bench-legacy-shots":
			_legacy_shots = true
		elif arg.begins_with("--bench-warmup="):
			_warmup = float(arg.trim_prefix("--bench-warmup="))
	set_process(false)
	set_physics_process(false)
	if active:
		_run.call_deferred()


func _run() -> void:
	await get_tree().process_frame
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	Engine.max_fps = 0
	for vp in [get_viewport().get_viewport_rid(), Effects.game_viewport.get_viewport_rid()]:
		RenderingServer.viewport_set_measure_render_time(vp, true)
	if _probe:
		await _probe_shot_batch()
		Game.quit_cleanly(0)
		return
	print(
		(
			"bench: %s, driver %s, %s"
			% [
				RenderingServer.get_video_adapter_name(),
				RenderingServer.get_current_rendering_driver_name(),
				OS.get_name()
			]
		)
	)
	for n in _counts:
		await _measure(n)
	var f := FileAccess.open(_out, FileAccess.WRITE)
	(
		f
		. store_string(
			(
				(
					JSON
					. stringify(
						{
							"adapter": RenderingServer.get_video_adapter_name(),
							"driver": RenderingServer.get_current_rendering_driver_name(),
							"os": OS.get_name(),
							"legacy_shots": _legacy_shots,
							"window": DisplayServer.window_get_size(),
							"results": _results,
						},
						"  "
					)
				)
				+ "\n"
			)
		)
	)
	f.close()
	Game.quit_cleanly(0)


func _measure(n: int) -> void:
	replay_assist = true
	_scene = Game.goto_scene(Game.SCENE_LEVEL)
	_scene.interactive = false
	if _scene.sim == null:
		push_error("bench: GDExtension missing")
		Game.quit_cleanly(1)
		return
	_scene.sim.bench_set_enemy_shot_capacity(maxi(n, 60))
	_n = n
	_seed = 42
	_measuring = false
	for a in [_sim_us, _refresh_us, _shots_us, _frame_us, _render_cpu, _render_gpu]:
		a.clear()
	_ticks = 0
	_frames = 0
	_shots_seen = 0
	_phase_start = Time.get_ticks_usec()
	_last_frame = _phase_start
	set_process(true)
	set_physics_process(true)
	# Warm-up, then measurement.
	await get_tree().create_timer(_warmup, true, false, true).timeout
	_measuring = true
	var t0 := Time.get_ticks_usec()
	_last_frame = t0
	await get_tree().create_timer(_seconds, true, false, true).timeout
	var elapsed := (Time.get_ticks_usec() - t0) / 1e6
	set_process(false)
	set_physics_process(false)
	var r := {
		"shots": n,
		"seconds": elapsed,
		"frames": _frames,
		"fps": _frames / elapsed,
		"ticks": _ticks,
		"ticks_per_s": _ticks / elapsed,
		"shots_visible_mean": float(_shots_seen) / maxi(_ticks, 1),
		"sim_us": _stats(_sim_us),
		"refresh_us": _stats(_refresh_us),
		"shots_us": _stats(_shots_us),
		"frame_ms": _stats(_frame_us, 0.001),
		"render_cpu_ms": _stats(_render_cpu),
		"render_gpu_ms": _stats(_render_gpu),
	}
	_results.append(r)
	print(
		(
			# gdlint: ignore=max-line-length
			"bench: N=%d fps=%.1f ticks/s=%.2f frame p50=%.2f p99=%.2f max=%.2f ms | sim p50=%d p99=%d µs | refresh p50=%d p99=%d µs (shots p50=%d) | render cpu p50=%.2f gpu p50=%.2f ms"
			% [
				n,
				r.fps,
				r.ticks_per_s,
				r.frame_ms.p50,
				r.frame_ms.p99,
				r.frame_ms.max,
				r.sim_us.p50,
				r.sim_us.p99,
				r.refresh_us.p50,
				r.refresh_us.p99,
				r.shots_us.p50,
				r.render_cpu_ms.p50,
				r.render_gpu_ms.p50
			]
		)
	)


func _stats(values: Array, scale := 1.0) -> Dictionary:
	if values.is_empty():
		return {"p50": 0, "p99": 0, "max": 0, "mean": 0}
	var v := values.duplicate()
	v.sort()
	var total := 0.0
	for x in v:
		total += x
	var q := func(p: float) -> float: return v[mini(int(v.size() * p), v.size() - 1)] * scale
	return {"p50": q.call(0.5), "p99": q.call(0.99), "max": v[-1] * scale, "mean": total / v.size() * scale}


func _physics_process(_delta: float) -> void:
	var sim: Node = _scene.sim
	if sim.level_ended():
		return
	var t0 := Time.get_ticks_usec()
	_seed = sim.bench_fill_enemy_shots(_n, _seed)
	var mask := Game.MASK_FIRE | (Game.MASK_LEFT if (_ticks / 30) % 2 == 0 else Game.MASK_RIGHT)
	sim.step(mask, 0)
	var t1 := Time.get_ticks_usec()
	# Same work as level.gd:_refresh, split up to time each part.
	_scene._update_layers()
	_scene._update_enemies()
	var t2 := Time.get_ticks_usec()
	if _legacy_shots:
		_scene._update_shots_from(sim.player_shots(), sim.enemy_shots())
	else:
		_scene._update_shots()
	var t3 := Time.get_ticks_usec()
	_scene._update_effects_and_sounds()
	_scene._player_node.queue_redraw()
	_scene._explosion_node.queue_redraw()
	_scene._hud.queue_redraw()
	var t4 := Time.get_ticks_usec()
	if _measuring:
		_sim_us.append(t1 - t0)
		_refresh_us.append(t4 - t1)
		_shots_us.append(t3 - t2)
		_ticks += 1
		_shots_seen += sim.bench_enemy_shot_count()


func _process(_delta: float) -> void:
	var now := Time.get_ticks_usec()
	if _measuring:
		_frame_us.append(now - _last_frame)
		_frames += 1
		_render_cpu.append(
			(
				RenderingServer.viewport_get_measured_render_time_cpu(get_viewport().get_viewport_rid())
				+ RenderingServer.viewport_get_measured_render_time_cpu(Effects.game_viewport.get_viewport_rid())
			)
		)
		_render_gpu.append(
			(
				RenderingServer.viewport_get_measured_render_time_gpu(get_viewport().get_viewport_rid())
				+ RenderingServer.viewport_get_measured_render_time_gpu(Effects.game_viewport.get_viewport_rid())
			)
		)
	_last_frame = now


## --bench-probe : breaks down the cost of ShotBatch.update_from_sim (buffer
## construction by the extension, resizing, MultiMesh.buffer assignment)
## without rendering, 100 repetitions per N.
func _probe_shot_batch() -> void:
	replay_assist = true
	var scene: Node = Game.goto_scene(Game.SCENE_LEVEL)
	scene.interactive = false
	var sim: Node = scene.sim
	var b: ShotBatch = scene._enemy_shots[0]
	for n in _counts:
		sim.bench_set_enemy_shot_capacity(maxi(n, 60))
		var seed := 42
		var t := [0, 0, 0]
		var reps := 100
		var cnt := 0
		for k in reps:
			seed = sim.bench_fill_enemy_shots(n, seed)
			sim.step(Game.MASK_FIRE, 0)
			var t0 := Time.get_ticks_usec()
			var data: PackedFloat32Array = sim.shot_instances(true, 0, 500, 0, -24, b._frames)
			var t1 := Time.get_ticks_usec()
			cnt = data.size() / ShotBatch.INSTANCE_FLOATS
			var cap: int = b.multimesh.instance_count
			while cap < cnt:
				cap *= 2
			if cap != b.multimesh.instance_count:
				b.multimesh.instance_count = cap
			data.resize(cap * ShotBatch.INSTANCE_FLOATS)
			var t2 := Time.get_ticks_usec()
			b.multimesh.buffer = data
			b.multimesh.visible_instance_count = cnt
			var t3 := Time.get_ticks_usec()
			t[0] += t1 - t0
			t[1] += t2 - t1
			t[2] += t3 - t2
		print(
			(
				"bench probe : N=%d instances=%d shot_instances %d µs, resize %d µs, buffer= %d µs (means over %d)"
				% [n, cnt, t[0] / reps, t[1] / reps, t[2] / reps, reps]
			)
		)
	await get_tree().process_frame
