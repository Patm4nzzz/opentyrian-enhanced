## Capture tool (autoload `Capture`, inactive without an argument).
##
##   godot --rendering-driver opengl3 --path rewrite/godot -- \
##       --capture=<scene>:<output.png>:<ticks>[:<inputs>] [options]
##
## <scene>   : menu | level | shop | display_options | mods
## <ticks>   : number of simulation ticks to advance (level scene);
##             for the other scenes, number of rendered frames to wait for.
## <inputs>  : sequence of segments separated by ","; segment = mask
##             (names separated by "+": up, down, left, right, fire,
##             change_fire, left_sidekick, right_sidekick, or integer),
##             suffix "*N" = duration in ticks (the last segment lasts
##             until the end), suffix "~N" = alternates left/right
##             every N ticks. E.g.: up*40,fire~30
## Options   : --level=ep1_09, --start-loc=<cur_loc> (fast_forward_to),
##             --crt, --xbrz, --bullet-time,
##             --shop-loadout=<11 values> [--shop-section=4] [--shop-cubes=0]:
##             campaign shop (shop scene, keys through --keys),
##             --hd-sprites[=0|1] ("HD sprites" mode, display_settings.gd),
##             --lang=<en|fr|de|es|it|pt> (text language, like Setup > Gameplay),
##             --replay=<file.gm>: replays the inputs of a golden master of the
##             C version (level, equipment, seed and helpers read from its header)
##             and checks that curLoc and the ship position at the last tick
##             match the C ones; the <inputs> are then ignored.
##             --window: 8-bit screens saved like the other scenes,
##             window image (HD mode interface) and not framebuffer;
##             --paused: level paused (P key) before the image;
##             --quit-box[=yes]: Escape confirmation box open
##             ("No" chosen, or "Yes");
##             --pause-menu: pause menu (Escape) open after the ticks,
##             driven by --keys; --in-game-help: in-game help (F1);
##             --replay-strict: additionally compares on every tick the full line
##             of the golden master (positions, counters and seven state fingerprints,
##             golden_master.c:write_tick) with that of the simulation; exit
##             code 1 at the first divergence. Without display (--headless),
##             no image is saved: only the verdict matters (Windows
##             CI, scripts/test-package-windows.ps1).
##
## After the ticks, waits for a few rendered frames then saves the window
## image (enlarged x3 nearest-neighbor if the window is 320x200).
extends Node

const NAMES := {
	"up": 1,
	"down": 2,
	"left": 4,
	"right": 8,
	"fire": 16,
	"change_fire": 32,
	"left_sidekick": 64,
	"right_sidekick": 128,
}
const SCENES := {
	"menu": "res://scenes/title.tscn",
	# Alias: "title" was requested by the title regression and, being unknown,
	# let the capture run forever.
	"title": "res://scenes/title.tscn",
	"setup": "res://scenes/setup.tscn",
	"high_scores": "res://scenes/high_scores.tscn",
	"level": "res://scenes/level.tscn",
	"shop": "res://scenes/shop.tscn",
	"display_options": "res://scenes/display_options.tscn",
	"controls": "res://scenes/controls_options.tscn",
	"modes": "res://scenes/modes_menu.tscn",
	"mods": "res://scenes/mods.tscn",
	"endless_prompt": "res://scenes/endless_prompt.tscn",
	"xmas_prompt": "res://scenes/xmas_prompt.tscn",
	"destruct": "res://scenes/destruct.tscn",
	"debrief": "res://scenes/debrief.tscn",
	"instructions": "res://scenes/instructions.tscn",
}
const SETTLE_FRAMES := 4

var active := false
var _scene := ""
var _out := ""
var _ticks := 0
## Segments [mask, duration (-1 = until the end), alternation period].
var _segments: Array = []
var _options := {}
## Masks and state lines of a golden master (--replay).
var _replay_masks := PackedInt32Array()
var _replay_rows: Array = []
## Autopilot helpers requested by the header of the replayed golden master
## ("assist 1"); read by the level scene (ADR 0125).
var replay_assist := false
## Combos of the replayed golden master: loop duration from GOLDEN_COMBO_MS (line
## "combo frame_ms N"), 0 = cut as under golden master; -1 outside replay.
var replay_combo_ms := -1
## Campaign golden master: [initial difficulty, carry-over from the
## previous level (TyrianSim.campaign_carry)]; empty otherwise. Read by the level scene.
var replay_campaign := []
## --replay-strict: first divergence found ("": none).
var _strict_error := ""
## --mouse: last position sent (for relative movement).
var _last_mouse := Vector2.ZERO


## Button of a --mouse step: l (left), r (right), m (middle), u / d
## (wheel up / down); "" or -: movement only.
static func _mouse_button(name: String) -> MouseButton:
	match name:
		"l":
			return MOUSE_BUTTON_LEFT
		"r":
			return MOUSE_BUTTON_RIGHT
		"m":
			return MOUSE_BUTTON_MIDDLE
		"u":
			return MOUSE_BUTTON_WHEEL_UP
		"d":
			return MOUSE_BUTTON_WHEEL_DOWN
	return MOUSE_BUTTON_NONE


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--capture="):
			_parse(arg.trim_prefix("--capture="))
		elif arg.begins_with("--"):
			var kv := arg.trim_prefix("--").split("=", true, 1)
			_options[kv[0]] = kv[1] if kv.size() > 1 else "1"
	if active:
		if _options.has("overlay-probe"):
			# Block 20.3 (ADR 0122): a layer provider draws on
			# every screen; the OTFR images must remain identical.
			TyrianScreen.add_overlay_provider(_overlay_probe)
		_run.call_deferred()


## Test layer provider: a rectangle and a text over
## the 8-bit image (like a mod element or an HD sprite).
static func _overlay_probe(screen: TyrianScreen) -> void:
	var r := ColorRect.new()
	r.color = Color(1, 0, 1, 0.5)
	r.position = Vector2(8, 8)
	r.size = Vector2(64, 16)
	screen.overlay.add_child(r)
	var t := TyrianText.new()
	t.text = "OVERLAY"
	t.position = Vector2(10, 10)
	screen.overlay.add_child(t)
	print("capture: test layer placed on %s" % screen.name)


func _parse(spec: String) -> void:
	# The output path may contain ":" (C:/...): split from the right.
	var parts := spec.split(":")
	if parts.size() < 3:
		printerr("capture: expected format <scene>:<output.png>:<ticks>[:<inputs>]")
		return
	_scene = parts[0]
	var rest := parts.slice(1)
	var inputs := ""
	if rest.size() >= 3 and not rest[-1].is_valid_int():
		inputs = rest[-1]
		rest = rest.slice(0, rest.size() - 1)
	_ticks = int(rest[-1])
	_out = ":".join(rest.slice(0, rest.size() - 1))
	_parse_inputs(inputs)
	active = SCENES.has(_scene)
	if not active:
		printerr("capture: unknown scene ", _scene)


func _parse_inputs(inputs: String) -> void:
	for seg in inputs.split(",", false):
		var duration := -1
		var swing := 0
		var parts := seg.split("*")
		if parts.size() == 2:
			duration = int(parts[1])
		var spec := parts[0]
		var swing_parts := spec.split("~")
		if swing_parts.size() == 2:
			spec = swing_parts[0]
			swing = int(swing_parts[1])
		# "*N" may follow "~N": fire~30*100
		if swing_parts.size() == 2 and swing_parts[1].contains("*"):
			var sp := swing_parts[1].split("*")
			swing = int(sp[0])
			duration = int(sp[1])
		var mask := 0
		if spec.is_valid_int():
			mask = int(spec)
		else:
			for n in spec.split("+", false):
				mask |= int(NAMES.get(n.strip_edges(), 0))
		_segments.append([mask, duration, swing])


## Header and lines of a .gm file (src/golden_master.c, ADR 0006).
func _load_replay(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		printerr("capture: unreadable golden master ", path)
		return false
	var cash := -1
	while not f.eof_reached():
		var line := f.get_line()
		var w := line.split(" ", false)
		if w.is_empty():
			continue
		if w[0] == "episode":
			Game.episode = int(w[1])
			Game.lvl_file_num = int(w[w.find("lvlfile") + 1])
			Game.difficulty = int(w[w.find("difficulty") + 1])
			Game.rng_seed = _u64_bits(w[w.find("seed") + 1])
			# Name displayed by the HUD (tyrian2.c:JE_outText(levelName)).
			# The name may contain spaces ("** ALE **"): text between
			# "level" and "lvlfile", trailing spaces included as in C.
			var a := line.find(" level ")
			var b := line.find(" lvlfile ")
			Game.level_name = line.substr(a + 7, b - a - 7) if a >= 0 and b > a else ""
		elif w[0] == "assist":
			replay_assist = w[1] == "1"
			replay_combo_ms = 0
		elif w[0] == "combo":
			replay_combo_ms = int(w[w.find("frame_ms") + 1])
		elif w[0] == "campaign":
			# Campaign (record_campaign_chain.sh): initial difficulty and
			# carry-over from the previous level of the chain (tyrian-golden,
			# GoldenMaster::load_chained).
			replay_campaign = [int(w[w.find("initial_difficulty") + 1]), _chain_carry(path)]
		elif w[0] == "mode":
			# Golden master v2 (side modes): same order as TyrianSim.set_modes.
			var dev := w[w.find("devices") + 1].split(":")
			Game.modes = PackedInt32Array(
				[
					int(w[2]),
					int(w[4]),
					int(w[6]),
					int(w[8]),
					int(w[10]),
					int(w[12]),
					w[14].hex_to_int(),
					w[16].hex_to_int(),
					int(dev[0]),
					int(dev[1])
				]
			)
			var cash2 := w[w.find("cash") + 1].split(":")
			Game.modes_extra = [int(cash2[0]), int(cash2[1]), int(w[w.find("initdiff") + 1]), PackedInt32Array()]
		elif w[0] == "items2" and Game.modes_extra.size() == 4:
			var f2 := w[4].split(":")
			var r2 := w[6].split(":")
			var s2 := w[12].split(":")
			Game.modes_extra[3] = PackedInt32Array(
				[
					int(w[2]),
					int(f2[0]),
					int(f2[1]),
					int(r2[0]),
					int(r2[1]),
					int(w[8]),
					int(w[10]),
					int(s2[0]),
					int(s2[1]),
					int(w[14])
				]
			)
		elif w[0] == "items":
			var front := w[4].split(":")
			var rear := w[6].split(":")
			var sk := w[12].split(":")
			Game.loadout = PackedInt32Array(
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
					0
				]
			)
		elif w[0].is_valid_int():
			if cash < 0:
				cash = int(w[6])
			_replay_masks.append(w[1].hex_to_int())
			_replay_rows.append(w)
	# Starting credits: those of the first line (no gain at tick 0).
	Game.loadout[10] = maxi(cash, 0)
	Game.start_cur_loc = 0
	return true


## Campaign carry-over from the previous level of a chain ("...-K.gm" ->
## "...-(K-1).gm"): that level replayed in a TyrianSim until its end.
## Two-level chains only, like the tests (campaign_test.gd).
func _chain_carry(path: String) -> PackedInt32Array:
	var m := RegEx.create_from_string("-(\\d+)\\.gm$").search(path)
	if m == null or int(m.get_string(1)) <= 1:
		return PackedInt32Array()
	var prev := path.substr(0, m.get_start()) + "-%d.gm" % (int(m.get_string(1)) - 1)
	if not FileAccess.file_exists(prev):
		return PackedInt32Array()
	var lines := FileAccess.get_file_as_string(prev).split("\n")
	var hdr := PackedStringArray()
	var camp := PackedStringArray()
	var it := PackedStringArray()
	for line in lines:
		var w := line.split(" ", false)
		if w.is_empty():
			continue
		if w[0] == "episode":
			hdr = w
		elif w[0] == "campaign":
			camp = w
		elif w[0] == "items":
			it = w
	if hdr.is_empty() or camp.is_empty() or it.is_empty():
		return PackedInt32Array()
	var fr := it[4].split(":")
	var rr := it[6].split(":")
	var sk := it[12].split(":")
	var sim: Node = ClassDB.instantiate("TyrianSim")
	sim.set_loadout(
		PackedInt32Array(
			[
				int(it[2]),
				int(fr[0]),
				int(fr[1]),
				int(rr[0]),
				int(rr[1]),
				int(it[8]),
				int(it[10]),
				int(sk[0]),
				int(sk[1]),
				int(it[14]),
				int(camp[camp.find("cash") + 1])
			]
		)
	)
	sim.set_assist(true)
	sim.set_campaign_state(int(camp[camp.find("initial_difficulty") + 1]), _chain_carry(prev))
	sim.load_level(
		Game.data_dir,
		int(hdr[1]),
		int(hdr[hdr.find("lvlfile") + 1]),
		int(hdr[hdr.find("difficulty") + 1]),
		_u64_bits(hdr[hdr.find("seed") + 1])
	)
	for line in lines:
		if line.is_empty() or not line[0].is_valid_int():
			continue
		if sim.level_ended():
			break
		sim.step(line.split(" ", false)[1].hex_to_int(), 0)
	var carry: PackedInt32Array = sim.campaign_carry()
	sim.free()
	return carry


## Folder where TyrianScreen writes each presented frame (--frames), "" otherwise.
func frames_dir() -> String:
	return String(_options.get("frames", ""))


## Value of a --name=value option ("" if absent).
func option(name: String) -> String:
	return str(_options.get(name, ""))


## Campaign shop in the same state as the C capture (scripts/
## visual_capture_shop_c.sh): --shop-loadout = 11 set_loadout values
## (end of the previous level), --shop-section (default 4), --shop-cubes.
func _setup_shop_campaign() -> void:
	Game.loadout = PackedInt32Array(Array(String(_options["shop-loadout"]).split(",")).map(func(v): return int(v)))
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = int(_options.get("shop-section", "4"))
	var cubes := int(_options.get("shop-cubes", "0"))
	var r: Dictionary = Game.campaign().run_script(Game.episode, Game.main_level, 2, Game.loadout[0], true, cubes)
	Game.set_shop(r, cubes)
	var m: PackedInt32Array = r.get("map", PackedInt32Array())
	Game.map_sections = PackedInt32Array()
	Game.map_planets = PackedInt32Array()
	for i in range(1, m.size(), 2):
		Game.map_planets.append(m[i - 1])
		Game.map_sections.append(m[i])
	Game.map_origin = int(r.get("map_origin", 1))
	Game.debrief = {"cube_max": cubes}


## First end-of-level screen (debrief.gd) with fixed values:
## --name=<level> --cubes=<n> --two-players --timed --bonus --dead (Exiting) --cash= --cash2=.
func _setup_debrief() -> void:
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = String(_options.get("name", "TYRIAN"))
	Game.next_level = 4
	Game.main_level = 3
	Game.level_start_ms = Time.get_ticks_msec()
	Game.campaign_modes = PackedInt32Array([1 if _options.has("two-players") else 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
	Game.time_battle_selection = 1 if _options.has("timed") else 0
	Game.loadout = PackedInt32Array([8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 1000])
	Game.debrief = {
		"level_name": Game.level_name,
		"cash": int(_options.get("cash", "12345")),
		"cash2": int(_options.get("cash2", "6789")),
		"enemy_killed": 7,
		"total_enemy": 10,
		"cube_max": int(_options.get("cubes", "2")),
		"all_players_alive": not _options.has("dead"),
		"bonus_level": _options.has("bonus"),
		"timer_countdown": 235,
		"lives": 2,
		"damage_by_bay": PackedInt32Array([0, 0, 0, 0, 0]),
		"damage_taken": 0,
		"damage_blocked": 0,
		"credits_start": 0
	}


func has_option(name: String) -> bool:
	return _options.has(name)


## Waits for an 8-bit screen to be idle (`idle` property), 20,000 frames at most.
func _wait_idle(scene: Node) -> void:
	for i in 20000:
		if scene == null or not is_instance_valid(scene):
			scene = Effects.current_game_scene()
		elif scene.get("idle") == true or scene.get("modal_active") == true:
			return
		await get_tree().process_frame


## Mouse of the tick (golden master v2: columns mmask mdx mdy, index 18 to 20).
func _mouse_at(tick: int) -> Array:
	if tick < _replay_rows.size() and _replay_rows[tick].size() > 20:
		var r: PackedStringArray = _replay_rows[tick]
		return [r[18].hex_to_int(), int(r[19]), int(r[20])]
	return [0, 0, 0]


func _mask_at(tick: int) -> int:
	if not _replay_masks.is_empty():
		return _replay_masks[tick] if tick < _replay_masks.size() else 0
	var t := tick
	for seg in _segments:
		if seg[1] >= 0 and t >= seg[1]:
			t -= seg[1]
			continue
		var m: int = seg[0]
		if seg[2] > 0:
			# Alternates left/right in slices of seg[2] ticks.
			m = (m & ~(NAMES["left"] | NAMES["right"])) | (NAMES["left"] if (t / seg[2]) % 2 == 0 else NAMES["right"])
		return m
	return 0


func _run() -> void:
	# Let Effects adopt the main scene before replacing it.
	await get_tree().process_frame
	Effects.crt = _options.has("crt")
	Effects.xbrz = _options.has("xbrz")
	# Setup graphics options (graphics_menu.c) for the comparison
	# sheets: CRT mode, scaler, scale mode and effects level.
	if _options.has("crt-mode"):
		Effects.set_crt_mode(int(_options["crt-mode"]))
	if _options.has("scaler"):
		DisplaySettings.set_scaler(int(_options["scaler"]))
	if _options.has("scaling-mode"):
		DisplaySettings.set_scaling_mode(int(_options["scaling-mode"]))
	if _options.has("detail"):
		Game.processor_type = int(_options["detail"])
	Effects.apply_display_settings()
	if _options.has("lang"):
		TyrianText.language = String(_options["lang"])
	if _options.has("replay") and not _load_replay(_options["replay"]):
		Game.quit_cleanly(1)
		return
	# The main scene (title) is already running: reloading it would
	# play its animation twice and mix their frames (--frames).
	if _scene == "shop" and _options.has("shop-loadout"):
		# The title (main scene) shares the VGA: let it finish its
		# animation, otherwise its fades keep writing after the change.
		await _wait_idle(Effects.current_game_scene())
		_setup_shop_campaign()
	elif _scene == "level" and (_options.has("pause-menu") or _options.has("in-game-help")):
		# Same reason: the pause menu and the help draw in the shared
		# VGA, which the title fades would still overwrite.
		await _wait_idle(Effects.current_game_scene())
	if _scene == "debrief":
		_setup_debrief()
	var scene: Node = Effects.current_game_scene()
	if scene == null or scene.scene_file_path != SCENES[_scene]:
		scene = Game.goto_scene(SCENES[_scene])
	# Bullet time only exists in a level (Effects.show_scene resets it to zero).
	if _scene == "level" and _options.has("bullet-time"):
		Effects.bullet_time_activate()
	if _scene == "level" and _options.has("replay-strict"):
		var code := _replay_strict(scene)
		if code != 0 or DisplayServer.get_name() == "headless":
			Game.quit_cleanly(code)
			return
	elif _scene == "level":
		scene.advance(_ticks, _mask_at, _mouse_at)
	if _scene == "level":
		if _options.has("paused"):
			scene.set_paused(true)
		elif _options.has("quit-box"):
			# Escape confirmation box; with "--quit-box=yes": "Yes" chosen.
			scene._confirm_exit()
			if _options["quit-box"] == "yes":
				scene._quit_box_input(TyrianControls.key_event(KEY_LEFT))
		elif _options.has("pause-menu") or _options.has("in-game-help"):
			# Pause menu (mainint.c:JE_inGameSetup) or help (JE_inGameHelp)
			# on the image of the last tick; --keys drives the open screen, whose
			# every frame (present) goes into --frames.
			if _options.has("pause-menu"):
				scene.open_pause_menu()
			else:
				scene.open_help()
			var screen: Node = scene._pause_menu if scene._pause_menu != null else scene._help_screen
			if screen != null:
				await _wait_idle(screen)
			for k in String(_options.get("keys", "")).split(",", false):
				if screen == null or not is_instance_valid(screen) or not screen.is_inside_tree():
					break
				screen.handle_event(TyrianControls.key_event(OS.find_keycode_from_string(k)))
				await get_tree().process_frame
				if is_instance_valid(screen) and screen.is_inside_tree():
					await _wait_idle(screen)
		if scene.sim != null:
			var p: PackedInt32Array = scene.sim.player(0)
			print(
				(
					"capture: tick=%d cur_loc=%d x=%d y=%d enemies=%d shots=%d"
					% [
						scene.sim.tick(),
						scene.sim.cur_loc(),
						p[0],
						p[1],
						scene.sim.enemies().size() / 8,
						scene.sim.player_shots().size() / 3
					]
				)
			)
			if not _replay_rows.is_empty() and _ticks <= _replay_rows.size():
				# Line of the last tick played in the C golden master.
				var row: PackedStringArray = _replay_rows[_ticks - 1]
				var same: bool = int(row[7]) == scene.sim.cur_loc() and int(row[2]) == p[0] and int(row[3]) == p[1]
				print(
					(
						"capture: C version at tick %s -> cur_loc=%s x=%s y=%s enemies=%s: %s"
						% [row[0], row[7], row[2], row[3], row[8], "IDENTICAL" if same else "DIFFERENT"]
					)
				)
	elif scene is TyrianScreen and not _options.has("window"):
		# 8-bit screens (ADR 0020): wait for the screen to be idle
		# (`idle`), send the --keys keys one by one, then save
		# the 320x200 framebuffer itself (not the window).
		await _wait_idle(scene)
		for k in String(_options.get("keys", "")).split(",", false):
			var code := OS.find_keycode_from_string(k)
			if code == KEY_NONE:
				code = int(k) if k.is_valid_int() else KEY_NONE
			var cur := Effects.current_game_scene()
			# Right after goto_scene, Effects' current scene may still
			# be the title screen: the requested scene receives the keys.
			if is_instance_valid(scene) and scene.is_inside_tree() and scene.has_method("handle_action"):
				cur = scene
			# Help opened by the shop (F1): it receives the keys.
			if is_instance_valid(scene) and scene.get("_help") != null and is_instance_valid(scene.get("_help")):
				cur = scene.get("_help")
			# Same path as the keyboard: event -> InputMap action (ADR 0120).
			if cur != null and cur.has_method("handle_action"):
				cur.handle_event(TyrianControls.key_event(code))
			await get_tree().process_frame
			var help: Variant = scene.get("_help") if is_instance_valid(scene) else null
			await _wait_idle(help if help != null and is_instance_valid(help) else Effects.current_game_scene())
		# --mouse=x:y:b,...: scripted mouse events in order, like
		# --keys (b = l, r, m, u, d; empty: movement only).
		for step in String(_options.get("mouse", "")).split(",", false):
			var f := step.split(":")
			var p := Vector2(float(f[0]), float(f[1]))
			var cur3: Node = Effects.current_game_scene()
			if is_instance_valid(scene) and scene.is_inside_tree() and scene.has_method("handle_mouse"):
				cur3 = scene
			if cur3 == null or not cur3.has_method("handle_mouse"):
				break
			var motion := InputEventMouseMotion.new()
			motion.position = p
			motion.global_position = p
			motion.relative = Vector2.ZERO if _last_mouse == p else p - _last_mouse
			_last_mouse = p
			cur3.handle_mouse(motion)
			await get_tree().process_frame
			if f.size() > 2 and f[2] != "" and f[2] != "-":
				var btn := _mouse_button(f[2])
				if btn != MOUSE_BUTTON_NONE:
					for pressed in [true, false]:
						# Checks that the scene did not change between pressed and released.
						if not is_instance_valid(cur3) or cur3.is_queued_for_deletion():
							break
						var mb := InputEventMouseButton.new()
						mb.position = p
						mb.global_position = p
						mb.button_index = btn
						mb.pressed = pressed
						cur3.handle_mouse(mb)
						await get_tree().process_frame
			await _wait_idle(Effects.current_game_scene())
		var cur2: Node = Effects.current_game_scene()
		var shot: Image = cur2.vga.image(0)
		var s := int(_options.get("scale", "1"))
		if s > 1:
			shot.resize(320 * s, 200 * s, Image.INTERPOLATE_NEAREST)
		DirAccess.make_dir_recursive_absolute(_out.get_base_dir())
		var e := shot.save_png(_out)
		print(
			(
				"capture: %s (%dx%d, %d frames) -> %s"
				% [
					_scene,
					shot.get_width(),
					shot.get_height(),
					TyrianScreen.frame_counter,
					_out if e == OK else "ERROR %d" % e
				]
			)
		)
		Game.quit_cleanly(0 if e == OK else 1)
		return
	else:
		if scene is TyrianScreen:
			await _wait_idle(scene)
		# Node2D menus (mods, modes, editor...) and windowed 8-bit screens
		# (--window): --keys goes through the same InputMap as the keyboard
		# (TyrianControls.menu_action, ADR 0120), to the current scene (a
		# menu may open another one: title -> Setup -> Jukebox).
		for k in String(_options.get("keys", "")).split(",", false):
			var code := OS.find_keycode_from_string(k)
			var cur: Node = Effects.current_game_scene()
			# Right after goto_scene, Effects' current scene may still
			# be the title screen: the requested scene receives the keys.
			if is_instance_valid(scene) and scene.is_inside_tree() and scene.has_method("handle_action"):
				cur = scene
			if cur is TyrianScreen:
				cur.handle_event(TyrianControls.key_event(code))
			else:
				var action := TyrianControls.menu_action(TyrianControls.key_event(code))
				if action != &"" and cur != null and cur.has_method("handle_action"):
					cur.handle_action(action)
			await get_tree().process_frame
			var now: Node = Effects.current_game_scene()
			if now is TyrianScreen:
				await _wait_idle(now)
		for i in _ticks:
			await get_tree().process_frame
	for i in SETTLE_FRAMES:
		await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	if img.get_width() <= Effects.GAME_SIZE.x:
		img.resize(img.get_width() * 3, img.get_height() * 3, Image.INTERPOLATE_NEAREST)
	DirAccess.make_dir_recursive_absolute(_out.get_base_dir())
	var err := img.save_png(_out)
	print(
		(
			"capture: %s (%dx%d) -> %s"
			% [_scene, img.get_width(), img.get_height(), _out if err == OK else "ERROR %d" % err]
		)
	)
	Game.quit_cleanly(0 if err == OK else 1)


## --replay-strict: one tick at a time (like tools/video_frames.gd), simulation line
## compared to the 18 columns of the C line of the same tick. The
## last line of an exhausted scenario ("end script") is rewritten by the C
## without a new iteration (writer.rs, script_tail): it is not compared.
## Returns the exit code: 0 if all the ticks played are identical.
func _replay_strict(scene: Node) -> int:
	if _replay_rows.is_empty() or scene.sim == null:
		printerr("capture: --replay-strict without a readable --replay")
		return 1
	var n := mini(_ticks, _replay_rows.size() - 1)
	for i in n:
		scene.advance(1, func(_t): return _mask_at(i), func(_t): return _mouse_at(i))
		var exp := " ".join(Array(_replay_rows[i]).slice(0, 18))
		var got: String = scene.sim.frame_line()
		if got != exp:
			_strict_error = "tick %d\n  C    : %s\n  Godot: %s" % [i, exp, got]
			printerr("capture: strict replay DIVERGENT at %s" % _strict_error)
			return 1
	print("capture: strict replay: %d ticks identical to the C (18 columns, fingerprints included)" % n)
	return 0


## Decimal u64 seed from the .gm (golden_master.c writes %llu) -> same bit pattern as an int (i64);
## TyrianSim.load_level reinterprets it as u64 (`seed as u64`). int() saturates beyond 2^63 - 1.
## Computed in two halves (low 9 digits) to never overflow: v - 2^64 if v >= 2^63.
static func _u64_bits(s: String) -> int:
	if s.length() <= 18:
		return int(s)
	var hi := int(s.substr(0, s.length() - 9))
	var lo := int(s.substr(s.length() - 9))
	if hi < 9223372036 or (hi == 9223372036 and lo < 854775808):
		return hi * 1000000000 + lo
	# 2^64 = 18446744073 * 10^9 + 709551616
	return (hi - 18446744073) * 1000000000 + (lo - 709551616)
