## Trailer clip: the real game, frame by frame, with sound.
## To be run with Godot's movie writer (`--write-movie`), under Xvfb:
##
##   godot --rendering-driver opengl3 --path rewrite/godot --write-movie out/clip.png \
##       --fixed-fps 35 --resolution 1920x1200 res://tools/trailer_clip.tscn -- \
##       --tc-mode=replay --tc-gm=rewrite/golden/e1-savara-autopilot.gm \
##       --tc-from=4585 --tc-frames=315 [--tc-style=2] [--tc-dmg=2] [--hd-sprites]
##
## One movie frame = one game tick (35 Hz, original rate); the neighboring
## .png file gets a .wav track (game music and sound effects).
##
## Modes (--tc-mode):
##   replay   : replays the inputs of a golden master (level, equipment,
##              seed and helpers read from its header, like capture.gd --replay)
##              with the player's effects (Freeze Frames, explosion style,
##              damage numbers: --tc-style=0..2, --tc-dmg=0..2,
##              --tc-hitstop=0|1). --tc-from: start tick (fast-forward
##              without frames).
##   live     : a level played by the autopilot (--tc-level=<file
##              number>, --tc-episode, --tc-seed, --tc-difficulty), with the mods
##              from --mods=a,b and their starting equipment; --tc-loadout=<11
##              values> to force the equipment; --tc-two=1: two players.
##   scene    : an 8-bit screen (--tc-scene=menu|setup|modes|mods|..., see
##              Capture.SCENES) driven by --tc-keys=frame:Key,... ;
##              for the shop, --shop-loadout=... like capture.gd.
##   destruct : the Destruct mini-game (--destruct-no-intro=<mode>).
## --tc-frames: number of frames to record, then clean exit.
## Read-only tool for the simulation and the player's files.
extends Node

var _opts := {}


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--tc-"):
			var kv := arg.trim_prefix("--tc-").split("=", true, 1)
			_opts[kv[0]] = kv[1] if kv.size() > 1 else "1"
	_run.call_deferred()


func _frames() -> int:
	return int(_opts.get("frames", "300"))


func _keys_schedule() -> Dictionary:
	var out := {}
	for entry in String(_opts.get("keys", "")).split(",", false):
		var p := entry.split(":")
		if p.size() == 2:
			var f := int(p[0])
			if not out.has(f):
				out[f] = []
			out[f].append(p[1])
	return out


## A key sent as on the keyboard (capture.gd, 8-bit screens or Node2D menus).
func _press(name: String, scene: Node) -> void:
	var code := OS.find_keycode_from_string(name)
	var cur: Node = Effects.current_game_scene()
	if is_instance_valid(scene) and scene.is_inside_tree() and scene.has_method("handle_action"):
		cur = scene
	if cur == null:
		return
	if cur is TyrianScreen:
		cur.handle_event(TyrianControls.key_event(code))
	else:
		var action := TyrianControls.menu_action(TyrianControls.key_event(code))
		if action != &"" and cur.has_method("handle_action"):
			cur.handle_action(action)


func _run() -> void:
	if get_parent() != get_tree().root:
		reparent(get_tree().root)
		await get_tree().process_frame
	await get_tree().process_frame
	# Movie frame size: the project enforces 960x600 (window_*_override).
	var size := String(_opts.get("size", "1920x1200")).split("x")
	DisplayServer.window_set_size(Vector2i(int(size[0]), int(size[1])))
	await get_tree().process_frame
	await get_tree().process_frame
	print("trailer_clip: window %s, root viewport %s" % [get_window().size, get_viewport().get_visible_rect().size])
	var mode := String(_opts.get("mode", "replay"))
	if _opts.has("mute"):
		Music.user_volume = 0
		Music.set_volume(0)
	if _opts.has("crt"):
		Effects.set_crt_mode(int(_opts["crt"]))
	if mode == "replay":
		await _run_replay()
	elif mode == "live":
		await _run_live()
	elif mode == "destruct":
		await _run_destruct()
	else:
		await _run_scene()


func _setup_effects() -> void:
	var script := load("res://scripts/setup.gd")
	script.settings.hitstop_enabled = bool(int(_opts.get("hitstop", "1")))
	var dmg := int(_opts.get("dmg", "2"))
	script.settings.damage_numbers_enabled = dmg > 0
	script.settings.colored_damage_numbers = dmg == 2
	script.settings.explosion_style = int(_opts.get("style", "2"))
	script.settings.rumble_enabled = false


func _run_replay() -> void:
	_setup_effects()
	if not Capture._load_replay(String(_opts["gm"])):
		Game.quit_cleanly(1)
		return
	if _opts.has("mods-loadout"):
		# Starting equipment of the enabled mods (--mods=...), like a new game.
		Game.loadout = Mods.starter_loadout(Game.loadout)
	# Capture.active: the level scene does not read the keyboard (interactive=false).
	# The player's effects are applied afterwards (apply_effects), once
	# the tool is no longer "capture".
	Capture.active = true
	var scene: Node = Game.goto_scene(Capture.SCENES["level"])
	await get_tree().process_frame
	if scene.sim == null:
		printerr("trailer_clip: simulation missing (GDExtension?)")
		Game.quit_cleanly(1)
		return
	Capture.active = false
	scene.interactive = false
	scene.apply_effects()
	var from := int(_opts.get("from", "0"))
	var step := 0
	if from > 0:
		scene.advance(from, func(i): return Capture._mask_at(i), func(i): return Capture._mouse_at(i))
		step = from
		scene.apply_effects()
	if _opts.has("bullet"):
		Effects.bullet_time_activate()
	var n := _frames()
	for i in n:
		var m: Array = Capture._mouse_at(step)
		scene.sim.set_mouse(m[0], m[1], m[2])
		scene.step_sim(Capture._mask_at(step), 0)
		step += 1
		await get_tree().process_frame
	await _finish_level(scene)
	print("trailer_clip: %d frames (ticks %d..%d), level %s" % [n, from, step, Game.level_name])
	Game.quit_cleanly(0)


## --tc-bullet: bullet time armed before the frames; --tc-pause: pause menu
## open after the ticks (frames recorded for 30 more frames).
func _finish_level(scene: Node) -> void:
	if _opts.has("pause"):
		scene.open_pause_menu()
		var screen: Node = scene._pause_menu
		if screen != null:
			await Capture._wait_idle(screen)
		for k in String(_opts.get("pause-keys", "")).split(",", false):
			if screen == null or not is_instance_valid(screen) or not screen.is_inside_tree():
				break
			screen.handle_event(TyrianControls.key_event(OS.find_keycode_from_string(k)))
			await get_tree().process_frame
			await Capture._wait_idle(screen)
		for i in 20:
			await get_tree().process_frame


func _run_live() -> void:
	_setup_effects()
	Game.episode = int(_opts.get("episode", "1"))
	Game.lvl_file_num = int(_opts.get("level", "9"))
	Game.difficulty = int(_opts.get("difficulty", "2"))
	Game.rng_seed = int(_opts.get("seed", "7"))
	Game.start_cur_loc = 0
	Game.level_name = String(_opts.get("name", ""))
	if _opts.has("loadout"):
		Game.loadout = PackedInt32Array(Array(String(_opts["loadout"]).split(",")).map(func(v): return int(v)))
	else:
		Game.loadout = Mods.starter_loadout(Game.loadout)
	Capture.replay_assist = true
	Capture.active = true
	var scene: Node = Game.goto_scene(Capture.SCENES["level"])
	await get_tree().process_frame
	if scene.sim == null:
		printerr("trailer_clip: simulation missing (GDExtension?)")
		Game.quit_cleanly(1)
		return
	Capture.active = false
	scene.interactive = false
	scene.apply_effects()
	var from := int(_opts.get("from", "0"))
	var step := 0
	var fire := Game.MASK_FIRE | Game.MASK_LEFT_SIDEKICK | Game.MASK_RIGHT_SIDEKICK
	if from > 0:
		scene.advance(from, func(_i): return fire)
		step = from
		scene.apply_effects()
	var n := _frames()
	for i in n:
		scene.step_sim(fire, 0)
		step += 1
		await get_tree().process_frame
	print("trailer_clip: %d frames (ticks %d..%d), level %s" % [n, from, step, Game.level_name])
	Game.quit_cleanly(0)


func _run_scene() -> void:
	var name := String(_opts.get("scene", "menu"))
	if not Capture.SCENES.has(name):
		printerr("trailer_clip: unknown scene ", name)
		Game.quit_cleanly(2)
		return
	if name == "shop" and Capture.has_option("shop-loadout"):
		# The title (main scene) finishes its animation before the shop.
		Capture.active = true
		await Capture._wait_idle(Effects.current_game_scene())
		Capture._setup_shop_campaign()
	var scene: Node = Effects.current_game_scene()
	if scene == null or scene.scene_file_path != Capture.SCENES[name]:
		scene = Game.goto_scene(Capture.SCENES[name])
	var keys := _keys_schedule()
	var n := _frames()
	for i in n:
		if keys.has(i):
			for k in keys[i]:
				_press(k, scene)
		await get_tree().process_frame
	print("trailer_clip: %d frames, scene %s" % [n, name])
	Game.quit_cleanly(0)


func _run_destruct() -> void:
	Game.goto_scene(Capture.SCENES["destruct"])
	await get_tree().create_timer(0.0).timeout
	var n := _frames()
	for i in n:
		await get_tree().process_frame
	print("trailer_clip: %d frames, destruct" % n)
	Game.quit_cleanly(0)
