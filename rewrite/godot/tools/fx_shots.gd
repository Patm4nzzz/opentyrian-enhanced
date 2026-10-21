## Captures of the Setup's Enhanced effects (Explosions, Damage Numbers) for
## before/after comparisons, under Xvfb, with no visible window.
##
##   xvfb-run -a godot --rendering-driver opengl3 --audio-driver Dummy \
##       --path rewrite/godot res://tools/fx_shots.tscn -- \
##       --fx-out=<folder> --fx-style=1 --fx-dmg=2 \
##       --fx-level=3 --fx-seed=7 --fx-power=11 --fx-ticks=140,300
##
## With --fx-gm=<golden master>, the level, seed, difficulty, equipment
## and inputs come from the .gm (same run as the instrumented C executable of
## visual_capture_level.sh); --fx-log=<file> writes the effects state on every tick
## (freeze, particles, numbers) to pick the ticks to compare.
##
## Achievements unlock and are saved as in the game: to compare against the C
## from a clean save, run with an empty XDG_DATA_HOME (otherwise
## "Achievement: First Blood" is already earned and the banner does not appear).
##
## Plays the level frame by frame (no replay: the player's effects are
## active, Capture stays inactive), advances to the requested tick, then saves
## the window image. Read-only tool for the simulation.
extends Node

var _opts := {}


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--fx-"):
			var kv := arg.trim_prefix("--fx-").split("=", true, 1)
			_opts[kv[0]] = kv[1] if kv.size() > 1 else "1"
	_run.call_deferred()


func _run() -> void:
	if get_parent() != get_tree().root:
		reparent(get_tree().root)
		await get_tree().process_frame
	if not _opts.has("out"):
		printerr("fx_shots: --fx-out option missing")
		Game.quit_cleanly(2)
		return
	var out := String(_opts["out"])
	DirAccess.make_dir_recursive_absolute(out)
	var style := int(_opts.get("style", "1"))
	var dmg := int(_opts.get("dmg", "2"))
	var level_num := int(_opts.get("level", "3"))
	var rng_seed_value := int(_opts.get("seed", "7"))
	var power := int(_opts.get("power", "11"))
	var masks := PackedInt32Array()
	var assist := false
	var gm_items := PackedInt32Array()
	if _opts.has("gm"):
		for line in FileAccess.get_file_as_string(String(_opts["gm"])).split("\n"):
			var w := line.split(" ", false)
			if w.size() > 2 and w[0] == "episode":
				level_num = int(w[w.find("lvlfile") + 1])
				rng_seed_value = int(w[w.find("seed") + 1])
				Game.difficulty = int(w[w.find("difficulty") + 1])
				# Golden master level name (shown in the dashboard).
				Game.level_name = String(" ".join(w.slice(w.find("level") + 1, w.find("lvlfile")))).strip_edges()
			elif w.size() > 14 and w[0] == "items":
				var front := w[4].split(":")
				var rear := w[6].split(":")
				var sk := w[12].split(":")
				gm_items = PackedInt32Array(
					[
						int(w[2]),
						int(front[0]),
						int(front[1]),
						int(rear[0]),
						int(rear[1]),
						int(w[8]),
						int(w[10]),
						int(sk[0]),
						int(sk[1]),
						int(w[14]),
						10000
					]
				)
			elif w.size() > 1 and w[0] == "assist":
				assist = w[1] == "1"
			elif line.length() > 0 and line[0].is_valid_int():
				if w.size() >= 18:
					masks.append(w[1].hex_to_int())
	var ticks := []
	for t in String(_opts.get("ticks", "140,300")).split(",", false):
		ticks.append(int(t))

	# Player effects, as on the keyboard (setup.gd).
	var script := load("res://scripts/setup.gd")
	script.settings.hitstop_enabled = bool(int(_opts.get("hitstop", "1")))
	script.settings.damage_numbers_enabled = dmg > 0
	script.settings.colored_damage_numbers = dmg == 2
	script.settings.explosion_style = style
	script.settings.rumble_enabled = false

	Game.episode = 1
	Game.lvl_file_num = level_num
	Game.start_cur_loc = 0
	Game.rng_seed = rng_seed_value
	if not _opts.has("gm"):
		Game.difficulty = 2
	Game.loadout = PackedInt32Array([1, 4, power, 0, 1, 4, 2, 0, 0, 0, 10000])
	if not gm_items.is_empty():
		Game.loadout = gm_items
	var scene: Node = Game.goto_scene(Game.SCENE_LEVEL)
	await get_tree().process_frame
	if scene == null or scene.sim == null:
		printerr("fx_shots: simulation missing (GDExtension?)")
		Game.quit_cleanly(1)
		return
	scene.interactive = false
	if assist:
		scene.sim.set_assist(true)
	var log_file: FileAccess = FileAccess.open(String(_opts["log"]), FileAccess.WRITE) if _opts.has("log") else null

	var written := 0
	var done := 0
	var last: int = ticks[ticks.size() - 1]
	for t in range(0, last + 1):
		scene.step_sim(
			masks[t] if t < masks.size() else (16 | (4 if (t / 90) % 2 == 0 else 8)) if masks.is_empty() else 0
		)
		if log_file != null:
			log_file.store_line("%d %s dn=%d" % [t, scene.sim.fx_stats(), scene.sim.damage_numbers().size() / 4])
		# One frame per tick: particles and damage numbers are
		# advanced at draw time (like JE_updateExplosions per frame in C).
		await RenderingServer.frame_post_draw
		if done < ticks.size() and t == ticks[done]:
			var img := get_viewport().get_texture().get_image()
			if img.get_width() != 320 and not _opts.has("native"):
				img.resize(320, 200, Image.INTERPOLATE_NEAREST)
			var path := out.path_join("fx-style%d-dmg%d-t%04d.png" % [style, dmg, t])
			if img.save_png(path) != OK:
				printerr("fx_shots: cannot write ", path)
				Game.quit_cleanly(1)
				return
			print("fx_shots: %s" % path)
			written += 1
			done += 1
	print("fx_shots: %d frames (style %d, damage %d) -> %s" % [written, style, dmg, out])
	Game.quit_cleanly(0)
