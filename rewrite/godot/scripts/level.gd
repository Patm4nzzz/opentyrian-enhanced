## Game scene: displays an imported level driven by the Rust core.
##
## The simulation (TyrianSim, GDExtension) is advanced by one tick per
## `_physics_process` (35 Hz, ADR 0003). This script only READS the state:
## positions of the layers, ship, enemies, shots, explosions.
## Particles and sounds are purely cosmetic and never feed back
## into the simulation.
##
## Drawing order (tyrian2.c, main loop): layer 1, layer 2,
## ground enemies, sky enemies, layer 3, top enemies, player
## shots, ship, enemy shots, explosions, then the dashboard.
extends Node2D

## Play area (video.h): 264×184, the C draws into game_screen with an
## offset PLAY_AREA_X = 24 removed when copying to the screen.
const PLAY_AREA_SIZE := Vector2i(264, 184)
const PLAY_AREA_X := 24
## Screen offset applied to the simulation coordinates (game_screen
## coordinates, as in the C).
const SIM_X_TO_SCREEN := -PLAY_AREA_X

## In-game dashboard: tyrian.pic no. 3 (tyrian2.c:430, one player).
const HUD_PIC := 3
const HUD_PIC_TWO_PLAYERS := 6

## Ranks spaced by 4: in HD (AI) mode, HdPlayfieldEffects interleaves the
## elements that tyrian2.c draws in a different order depending on the level.
enum Z {
	LAYER1 = 0,
	LAYER2 = 4,
	ENEMY_GROUND = 8,
	ENEMY_SKY = 12,
	LAYER3 = 16,
	ENEMY_TOP = 20,
	PLAYER_SHOTS = 24,
	PLAYER = 28,
	ENEMY_SHOTS = 32,
	EXPLOSIONS = 36,
	HUD = 44
}

var sim: Node
var map: LevelMap
var level_json: Dictionary
var ship_sheet: SpriteSheet
var ship_graphic := 233
var explosion_sheet: SpriteSheet

## External control (capture tool): when false, the keyboard is ignored
## and the simulation only advances through `advance()`.
var interactive := true

var _enemy_root := {}
var _enemy_pool := {}
var _player_node: Node2D
var _player_shots: Array[ShotBatch] = []
var _enemy_shots: Array[ShotBatch] = []
var _explosion_node: Node2D
var _hud: Node2D
## Pixel-perfect 8-bit HUD (hud_renderer.gd); null without GDExtension.
var _hud_vga: HudRenderer
## Layers 1-2 composed in indices (playfield_background.gd); null otherwise.
var _playfield_bg: PlayfieldBackground
## HD (AI) mode: blend material of layer 2; color → index tables per palette.
const HD_LAYER_BLEND := preload("res://shaders/hd_layer_blend.gdshader")
var _hd_layer2_material: ShaderMaterial
static var _hd_lut_cache := {}
var _debug_text: TyrianText
var _last_counts := Vector3i.ZERO
## Mod PNG for the ship / shots (Mods.texture), otherwise null.
var _mod_ship_texture: Texture2D
## shipgraphic of the ship in the mods' "ships" table (poses at +-2 and +-4).
var _mod_ship_graphic := 0
var _mod_shots: Node2D
var _mod_shot_items: Array = []

var _campaign_done := false
## Autopilot aids passed to the Sim (replay tools only).
var assist := false
## Playback of a `.rep` (Replays menu or --rep=): inputs come from the
## file, the end of playback returns to the title (tyrian2.c, replay_stop).
var replaying := false
## Title demo (play_demo): keys from data/demo.N, any key
## ends it (tyrian2.c, "input kills demo"), end of level = return to the title.
var demo := false
var _demo_song := 0
var _demo_key := false
var _demo_done := false

const SetupScreen := preload("res://scripts/setup.gd")

## Level shortcuts (F3 to F12, slow motion) and the C's overlays.
var hotkeys: LevelHotkeys


func _ready() -> void:
	hotkeys = LevelHotkeys.new(self)
	# Setup effect settings applied immediately (setup.gd:_apply_effects).
	add_to_group(&"tyrian_level")
	# In capture mode, the simulation only advances through advance().
	interactive = not Capture.active
	# tyrian2.c:JE_main, "JE_clearKeyboard()" at level start: an Escape
	# pressed on the previous screen does not open the pause menu.
	_resumed_frame = Engine.get_physics_frames()
	_set_raw_mouse_events(true)
	_load_level()


func _load_level() -> void:
	sim = ClassDB.instantiate("TyrianSim") if ClassDB.class_exists("TyrianSim") else null
	if sim == null:
		push_error("GDExtension tyrian-godot missing: copy libtyrian_godot.so / tyrian_godot.dll into res://bin/")
	else:
		sim.name = "TyrianSim"
		add_child(sim)
		Mods.attach(sim)
		sim.set_loadout(Game.loadout)
		# config.c:processorType (Setup > Graphics > Visual FX), applied BEFORE
		# loading: initialize_starfield and the hitstop.c draws
		# depend on it (backgrnd.c, collision.c).
		if sim.has_method("set_processor_type"):
			sim.set_processor_type(Game.processor_type)
		# Autopilot aids (autopilot.c:autopilot_drive): reserved
		# for replaying "assist 1" golden masters by the tools (ADR 0125)
		# and the campaign bot (--campaign-bot), never enabled by the player flow.
		assist = Capture.replay_assist or Bench.replay_assist or CampaignBot.replay_assist
		# Mini-games (]e[: ** ALE **, SQUADRON): with no end event, they only
		# end at game over; the aids' invulnerability would make them
		# endless. The bot therefore plays them without aids, like a player.
		if Game.campaign_active and Game.mini_game:
			assist = false
		sim.set_assist(assist)
		sim.set_bonus_level_current(Game.campaign_active and Game.bonus_level_current)
		# Setup effects (hitstop, particles, damage numbers, vibration):
		# player's game only; the tools keep the golden master settings
		# (golden_master.c:golden_master_level_start).
		if _player_effects():
			SetupScreen.apply_effects_to(sim)
		if Game.campaign_active:
			sim.set_campaign_state(Game.initial_difficulty, Game.campaign_carry)
		elif Capture.active and Capture.replay_campaign.size() == 2:
			# Campaign golden master replayed: like tyrian-golden.
			sim.set_campaign_state(Capture.replay_campaign[0], Capture.replay_campaign[1])
		if Game.modes.size() >= 10 and sim.has_method("set_modes"):
			# B-02: the C's mut_* globals apply to every game; the modes built by
			# New Game carry modes[6] = 0, the Setup mutators are added to it.
			sim.set_modes(Game.modes_with_mutators(_player_effects()))
			if Game.modes_extra.size() == 4:
				sim.set_modes_extra(Game.modes_extra[0], Game.modes_extra[1], Game.modes_extra[2], Game.modes_extra[3])
		elif _player_effects() and sim.has_method("set_modes"):
			# Setup mutators (mutators_menu.c): the C's mut_* globals, read by
			# every game (campaign, Endless); the Endless score counts them
			# (endless.c: mutators_score_multiplier).
			EndlessSession.mutators = Game.mutators
			if Game.mutators != 0:
				sim.set_modes(PackedInt32Array([0, 0, 0, 0, 0, 0, Game.mutators, 0, 0, 0]))
		EndlessSession.configure(sim)
		if Game.demo_pending > 0:
			# level_load.c:JE_loadMap with play_demo -> mainint.c:load_next_demo.
			demo = true
			sim.set_campaign_state(Game.c_initial_difficulty, Game.demo_carry)
			var info: Dictionary = sim.load_demo(Game.data_dir, Game.demo_pending, Game.rng_seed, Game.demo_cash)
			if info.is_empty():
				push_error("Demo unreadable : %s" % sim.last_error())
			else:
				Game.episode = int(info["episode"])
				Game.lvl_file_num = int(info["lvl_file_num"])
				Game.level_name = str(info["level_name"])
				_demo_song = int(info["level_song"])
			Game.demo_pending = 0
		elif not Game.replay_file.is_empty():
			# C version .rep replay (ADR 0090): level and inputs from the file.
			# Consumed here: the next game (New Game...) must not re-read it.
			replaying = true
			var replay_path := Game.replay_file
			Game.replay_file = ""
			if sim.load_replay(Game.data_dir, replay_path):
				var info: PackedInt32Array = sim.replay_info()
				Game.episode = info[0]
				Game.lvl_file_num = info[4]
				# JE_loadMap sets levelName before the playback: HUD level name
				# box (the demo branch does the same).
				Game.level_name = Game.replay_level_name(info[0], info[1], info[2])
			else:
				push_error("Replay unreadable : %s" % sim.last_error())
		else:
			# replay.c:replay_level_start_hook: an armed recording opens at the
			# start of the level, but never during a demo, a
			# playback or under a tool (capture, bench, bot).
			if ReplayRecording.armed and interactive and _player_effects() and sim.has_method("request_recording"):
				ReplayRecording.armed = false
				_recording_requested = true
				var rec_seed := (randi() << 32) | randi()
				var rec_speed := int(SetupScreen.settings.get("game_speed", GAME_SPEED.NORMAL))
				sim.request_recording(Game.main_level if Game.campaign_active else 0, rec_seed, rec_speed)
			if not sim.load_level(Game.data_dir, Game.episode, Game.lvl_file_num, Game.difficulty, Game.rng_seed):
				push_error("Level loading failed : %s" % sim.last_error())
		# Comparison with the C (scripts/golden_pair.sh): low-armor blink clock
		# shifted like the golden master's.
		if Capture.active and Capture.has_option("clock-offset"):
			sim.set_render_clock_offset(int(Capture._options["clock-offset"]))

	# tyrian2.c:JE_main → stats.c:level_stats_reset: start_ticks.
	Game.level_start_ms = Time.get_ticks_msec()
	_build_map()
	_build_actors()
	_build_hud()
	_apply_session_options()

	if sim != null and Game.start_cur_loc > 0:
		sim.fast_forward_to(Game.start_cur_loc)
	if sim != null:
		sim.take_music_cues()  # orders emitted during the fast-forward: ignored
	if _hud_vga != null and not _hud_vga.start(sim, Game.level_name):
		_hud_vga.queue_free()
		_hud_vga = null
	if _hud_vga != null and replaying:
		# enemy_render.c:221: hardcoded "PLAY", the same in all languages.
		_hud_vga.replay_label = "PLAY"
	_recording_started()
	if ClassDB.class_has_method("TyrianSim", "set_playfield_texts"):
		(
			ClassDB
			. class_call_static(
				"TyrianSim",
				"set_playfield_texts",
				PackedStringArray(
					[
						TyrianText.L("hud_warning"),
						TyrianText.L("miscText_59"),
						TyrianText.L("miscText_66"),
						# helptext.c: miscText[21] is replaced by the misc_game_over key.
						TyrianText.L("misc_game_over")
					]
				)
			)
		)
	if demo and ClassDB.class_has_method("TyrianSim", "set_playfield_demo_text"):
		# player_movement.c: "JE_dString(VGAScreen, 115, 10, miscText[7], SMALL_FONT_SHAPES)".
		ClassDB.class_call_static("TyrianSim", "set_playfield_demo_text", TyrianText.L("miscText_7"))
	_ach_start()
	_start_playfield_background()
	_music_start()
	_refresh()
	_start_entry_fade()


## level_load.c:JE_loadMap: visual effects from Fast (processorType >= 2),
## the previous screen (the shop) goes white in 4 steps then from white to black
## in 12 steps; in Classic, the previous screen fades to black in 50 steps
## (fade_black(50), level_load.c:971); in a demo, neither
## (load_next_demo). Then, in all cases, tyrian2.c:438: fade-in of the
## level in 50 steps (fade_palette). The simulation waits for the end, like the C. One
## step = one setDelay(1) of the C (DELAY_PERIOD, tyrian_screen.gd). No fade for
## the playback of a recording nor for the tools (bot, measurements).
const ENTRY_FLASH_STEPS := 4
const ENTRY_BLACK_STEPS := 12
const ENTRY_CLASSIC_BLACK_STEPS := 50
const ENTRY_FADE_IN_STEPS := 50
var _entry_fade: ColorRect
## Play area held black during the fade-in (see _start_entry_fade).
var _entry_playfield_mask: ColorRect
var _entry_previous: TextureRect
var _entry_fade_start_ms := 0
var _entry_flash_steps := 0
var _entry_black_steps := 0


func _start_entry_fade() -> void:
	if not interactive or replaying or Bench.active or CampaignBot.active:
		return
	if demo:
		_entry_flash_steps = 0
		_entry_black_steps = 0
	elif Game.processor_type >= 2:
		_entry_flash_steps = ENTRY_FLASH_STEPS
		_entry_black_steps = ENTRY_BLACK_STEPS
	else:
		_entry_flash_steps = 0
		_entry_black_steps = ENTRY_CLASSIC_BLACK_STEPS
	# Last presented image of the previous screen: the C fades the current screen,
	# not a black screen.
	# Without rendering (--headless, dummy driver) the viewport texture does not exist.
	if (
		_entry_flash_steps + _entry_black_steps > 0
		and DisplayServer.get_name() != "headless"
		and is_instance_valid(Effects)
		and Effects.game_viewport != null
	):
		var img := Effects.game_viewport.get_texture().get_image()
		if img != null and not img.is_empty():
			_entry_previous = TextureRect.new()
			_entry_previous.texture = ImageTexture.create_from_image(img)
			_entry_previous.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
			_entry_previous.stretch_mode = TextureRect.STRETCH_SCALE
			_entry_previous.size = Vector2(320, 200)
			_entry_previous.mouse_filter = Control.MOUSE_FILTER_IGNORE
			_entry_previous.z_index = 199
			add_child(_entry_previous)
	_entry_fade = ColorRect.new()
	_entry_fade.color = (
		Color(1, 1, 1, 0) if _entry_flash_steps > 0 else Color(0, 0, 0, 0 if _entry_previous != null else 1)
	)
	_entry_fade.size = Vector2(320, 200)
	_entry_fade.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_entry_fade.z_index = 200
	add_child(_entry_fade)
	# tyrian2.c:438: fade_palette only lights up the dashboard image
	# (JE_loadPic 3/6, black play area); the play area only appears with the
	# level loop, at the brightness of the opening filter (-14 then 0).
	_entry_playfield_mask = ColorRect.new()
	_entry_playfield_mask.color = Color.BLACK
	_entry_playfield_mask.size = Vector2(HudRenderer.PLAY_W, HudRenderer.PLAY_H)
	_entry_playfield_mask.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_entry_playfield_mask.z_index = 199
	_entry_playfield_mask.visible = false
	add_child(_entry_playfield_mask)
	_entry_fade_start_ms = Time.get_ticks_msec()
	_entry_last_ms = _entry_fade_start_ms


## Last pass through _entry_fade_active (0: none).
var _entry_last_ms := 0
## Longest gap taken into account between two passes: the C presents each
## step (setDelay(1) then JE_showVGA); a long frame (loading the HD
## images, first render) must not skip the flash or the fade.
const ENTRY_MAX_GAP_MS := 40


## True while the entry transition is displayed (the simulation does not advance).
func _entry_fade_active() -> bool:
	if _entry_fade == null:
		return false
	var now := Time.get_ticks_msec()
	if _entry_last_ms > 0 and now - _entry_last_ms > ENTRY_MAX_GAP_MS:
		_entry_fade_start_ms += now - _entry_last_ms - ENTRY_MAX_GAP_MS
	_entry_last_ms = now
	var step := float(now - _entry_fade_start_ms) / 1000.0 / TyrianScreen.DELAY_PERIOD
	var to_black := float(_entry_flash_steps + _entry_black_steps)
	if step >= to_black + ENTRY_FADE_IN_STEPS:
		_entry_fade.queue_free()
		_entry_fade = null
		if _entry_playfield_mask != null:
			_entry_playfield_mask.queue_free()
			_entry_playfield_mask = null
		if _entry_previous != null:
			_entry_previous.queue_free()
			_entry_previous = null
		return false
	if _entry_playfield_mask != null:
		_entry_playfield_mask.visible = step >= to_black
	if _entry_previous != null:
		# The previous screen stays visible until it is fully faded.
		_entry_previous.visible = step < (_entry_flash_steps if _entry_flash_steps > 0 else to_black)
	if step < _entry_flash_steps:
		# fade_white: each color goes towards white.
		_entry_fade.color = Color(1, 1, 1, step / _entry_flash_steps)
	elif step < to_black:
		var k := (step - _entry_flash_steps) / _entry_black_steps
		if _entry_flash_steps > 0:
			# fade_black after the flash: from white to black.
			_entry_fade.color = Color(1.0 - k, 1.0 - k, 1.0 - k, 1.0)
		else:
			# Classic: the previous screen towards black.
			_entry_fade.color = Color(0, 0, 0, k if _entry_previous != null else 1.0)
	else:
		_entry_fade.color = Color(0, 0, 0, 1.0 - (step - to_black) / ENTRY_FADE_IN_STEPS)
	return true


## Session settings set at level start: game speed
## (tyrian2.c:JE_main, JE_setNewGameSpeed) and --devmode god mode
## (params.c:constantPlay, a C global that survives levels). The tools
## (capture, bench, campaign bot) keep their own pace.
var _game_speed_applied := false


func _apply_session_options() -> void:
	if interactive and not Bench.active and not CampaignBot.active:
		GAME_SPEED.apply(int(SetupScreen.settings.get("game_speed", GAME_SPEED.NORMAL)))
		_game_speed_applied = true
	if sim != null and DEV_CONSOLE.available() and DEV_CONSOLE.god_mode and sim.has_method("set_dev_god_mode"):
		sim.set_dev_god_mode(true)


## Scenery layers: scene generated by editor/build_levels.gd if it exists,
## otherwise direct construction from the imported JSON.
func _build_map() -> void:
	var id := Game.level_id()
	var scene_path := "res://levels/%s.tscn" % id
	level_json = TyrianAssets.read_json_uncached("levels/%s.json" % id)
	if ResourceLoader.exists(scene_path):
		map = Effects.instantiate_scene(scene_path)
	if map == null and level_json != null:
		map = LevelMapBuilder.build_map(level_json, LevelMapBuilder.build_tileset(level_json["tileset"]))
	elif map == null:
		push_error("Level %s not found (run tyrian-import)" % id)
		map = LevelMap.new()
	add_child(map)
	_apply_hd_map()
	Effects.effects_changed.connect(_on_display_changed)
	var z := [Z.LAYER1, Z.LAYER2, Z.LAYER3]
	for i in 3:
		var l := map.layer(i)
		if l != null:
			l.z_index = z[i]


## HD (AI) mode, ADR 0100: each layer displays its whole upscaled
## ×4 map (HdMapLayer, WebP strips), child of the TileMapLayer whose tiles
## are hidden (self_modulate): same position, hence same scrolling. A
## layer without HD images (level of a mod that changes the tiles) keeps its original
## tiles. Called again on every rendering change (F3, F10).
func _apply_hd_map() -> void:
	if map == null or map.tileset_name.is_empty() or level_json == null:
		return
	var want := DisplaySettings.hd_sprites()
	var blank := {}
	for i in 3:
		var l := map.layer(i)
		if l == null:
			continue
		var hd := l.get_node_or_null("HdMap") as HdMapLayer
		if hd == null and want:
			if blank.is_empty():
				for n in TyrianAssets.read_json("tiles/%s.json" % map.tileset_name)["blank_tiles"]:
					blank[int(n)] = true
			hd = HdMapLayer.open(
				DisplaySettings.UPSCALED.path_join("maps"), map.tileset_name, level_json["layers"][i], blank
			)
			if hd != null:
				l.add_child(hd)
				if i == 1:
					_hd_layer2_material = _hd_blend_material()
					hd.material = _hd_layer2_material
		if hd != null:
			hd.visible = want
			hd.set_process(want)
			if want:
				hd.update_view()
		l.self_modulate.a = 0.0 if hd != null and want else 1.0
	_update_hd_blend()


## HD layer 2 material: the C's blend by index (shaders/hd_layer_blend.gdshader)
## with the game palette (that of the indexed background), or null without the extension.
func _hd_blend_material() -> ShaderMaterial:
	var vga: RefCounted = _hud_vga.vga if _hud_vga != null else _two_player_palette()
	if vga == null and ClassDB.class_exists("TyrianVga"):
		vga = ClassDB.instantiate("TyrianVga")
		ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
		if not vga.load_pic(0, HUD_PIC, true):
			vga = null
	if vga == null:
		return null
	var pal: PackedByteArray = vga.palette_bytes()
	var rows: int = sim.layer2_hue_rows(Game.data_dir) if sim != null and sim.has_method("layer2_hue_rows") else 0xffff
	for r in [rows, 0xffff]:
		var key := [pal, r]
		if not _hd_lut_cache.has(key):
			if _hd_lut_cache.size() >= 8:
				_hd_lut_cache.clear()
			var lut: Image = ClassDB.class_call_static("TyrianVga", "nearest_index_lut", pal, r)
			if lut == null:
				return null
			_hd_lut_cache[key] = ImageTexture.create_from_image(lut)
	var pal_img := Image.create_from_data(256, 1, false, Image.FORMAT_RGB8, pal)
	var m := ShaderMaterial.new()
	m.shader = HD_LAYER_BLEND
	m.set_shader_parameter("index_lut", _hd_lut_cache[[pal, rows]])
	m.set_shader_parameter("under_lut", _hd_lut_cache[[pal, 0xffff]])
	m.set_shader_parameter("palette_tex", ImageTexture.create_from_image(pal_img))
	return m


## Layer 2 blended or opaque depending on the published state (events 48, background2over).
func _update_hd_blend() -> void:
	if _hd_layer2_material != null and sim != null and sim.has_method("layer2_blended"):
		var on: bool = sim.layer2_blended()
		if on != _hd_layer2_material.get_shader_parameter("blend_on"):
			_hd_layer2_material.set_shader_parameter("blend_on", on)


## HD (AI) mode: the indexed background (PlayfieldBackground) is absent, along with what
## it drew in addition to the Godot nodes. Two pixel-perfect layers
## replace it: the starfield (TyrianSim.playfield_stars) between
## layer 1 and layer 2, like tyrian2.c:JE_main (draw_background_1,
## update_and_draw_starfield, then layer 2); level texts, boss
## bars, sparks and RGBA layer (TyrianSim.playfield_hd_overlay)
## above the game.
var _hd_stars: Sprite2D
var _hd_overlay: Sprite2D
var _hd_overlay_xbrz: HdInterface
var _hd_palette: RefCounted
## Ships, shadows, explosion blending, filter and lamp in HD.
var _hd_effects: HdPlayfieldEffects


func _update_hd_overlays() -> void:
	var want := (
		DisplaySettings.hd_sprites()
		and _playfield_bg == null
		and sim != null
		and sim.has_method("playfield_hd_overlay")
	)
	for s in [_hd_stars, _hd_overlay]:
		if s != null:
			s.visible = want
	if _hd_effects != null:
		_hd_effects.set_active(want, _explosion_node)
	# In HD the ships come from HdPlayfieldEffects (except a mod ship).
	# Without HD, the indexed background (PlayfieldBackground) decides its visibility.
	if want:
		_player_node.visible = _mod_ship_texture != null
	elif _playfield_bg == null:
		_player_node.visible = true
	# Enemies: in HD, drawn by HdPlayfieldEffects from the C's list.
	if _hd_effects != null and (want or _playfield_bg == null):
		for root in _enemy_root.values():
			root.visible = not want
		for n in _player_shots + _enemy_shots:
			n.visible = not want
	if not want:
		return
	if _hd_palette == null:
		_hd_palette = _hud_vga.vga if _hud_vga != null else _two_player_palette()
		if _hd_palette == null:
			return
	var pal: PackedByteArray = _hd_palette.palette_bytes()
	_hd_stars = _hd_sprite(_hd_stars, "HdStars", Z.LAYER1, sim.playfield_stars(Game.data_dir, pal))
	# Texts, sparks and halos: after the filter and the lamp (EXPLOSIONS + 2).
	var overlay_img: Image = sim.playfield_hd_overlay(pal)
	_hd_overlay = _hd_sprite(_hd_overlay, "HdOverlay", Z.EXPLOSIONS + 3, overlay_img)
	# Level texts (countdown, boss bars...) sharply upscaled (xBRZ).
	if _hd_overlay != null and overlay_img != null:
		if _hd_overlay_xbrz == null:
			_hd_overlay_xbrz = HdInterface.new(_hd_overlay, null)
		_hd_overlay_xbrz.update(0, overlay_img)
	if _hd_effects != null:
		_hd_effects.update(pal)


## 264×184 layer placed over the play area, created on first call: added
## after the nodes of the same z, it is drawn above them.
func _hd_sprite(s: Sprite2D, node_name: String, z: int, img: Image) -> Sprite2D:
	if img == null:
		return s
	if s == null:
		s = Sprite2D.new()
		s.name = node_name
		s.centered = false
		s.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		s.z_index = z
		s.texture = ImageTexture.create_from_image(img)
		add_child(s)
		# Stars: right after the scenery, under the ground enemies placed at the same z.
		if z == Z.LAYER1:
			move_child(s, map.get_index() + 1)
	else:
		(s.texture as ImageTexture).update(img)
	s.visible = true
	return s


## Rendering toggle during the level: HD scenery or tiles, and indexed background
## (only with the original tiles).
func _on_display_changed() -> void:
	# Level already removed (Effects.show_scene emits effects_changed after the
	# removal, bullet_time_reset): nothing to rebuild.
	if not is_inside_tree():
		return
	_apply_hd_map()
	if DisplaySettings.hd_sprites() and _playfield_bg != null:
		_playfield_bg.stop(map)
		_playfield_bg = null
	elif not DisplaySettings.hd_sprites() and _playfield_bg == null:
		_start_playfield_background()


func _build_actors() -> void:
	for layer in 4:
		var root := Node2D.new()
		root.name = "Enemies%d" % layer
		root.z_index = [Z.ENEMY_GROUND, Z.ENEMY_GROUND, Z.ENEMY_SKY, Z.ENEMY_TOP][layer]
		add_child(root)
		_enemy_root[layer] = root
		_enemy_pool[layer] = []

	# Ship: sheet spriteSheet9 (or T2000 if shipgraphic > 500, varz.c).
	var ships: Array = TyrianAssets.read_json("items.json")["episodes_1_3"]["ships"]
	var ship_id := clampi(Game.loadout[0], 0, ships.size() - 1)
	ship_graphic = int(ships[ship_id]["shipgraphic"])
	# Mod ship (19..32): shipgraphic comes from the tables resolved in Rust,
	# the image from the mod's PNG ("ships" table).
	var mod_gr := Mods.ship_graphic(Game.loadout[0])
	if mod_gr > 0:
		ship_graphic = mod_gr
	_mod_ship_texture = Mods.texture("ships", ship_graphic)
	# The mods' ship table is read at the full shipgraphic (ADR 0134); only the
	# original sheets take the "- 500" of the T2000 ships.
	_mod_ship_graphic = ship_graphic
	if ship_graphic > 500:
		ship_sheet = TyrianAssets.sheet("main_12_ships_t2000")
		ship_graphic -= 500
	else:
		ship_sheet = TyrianAssets.sheet("main_08_player_ships")
	_player_node = Node2D.new()
	_player_node.name = "Player"
	_player_node.z_index = Z.PLAYER
	_player_node.draw.connect(_draw_player)
	add_child(_player_node)

	# Shots: spriteSheet8 (main_07) and spriteSheet12 (main_11, index - 500), shots.c.
	for spec in [["main_07_player_shots", 0], ["main_11_player_shots2", 500]]:
		var a := ShotBatch.new()
		a.setup(spec[0], spec[1])
		a.z_index = Z.PLAYER_SHOTS
		add_child(a)
		_player_shots.append(a)
		var b := ShotBatch.new()
		b.setup(spec[0], spec[1])
		b.z_index = Z.ENEMY_SHOTS
		add_child(b)
		_enemy_shots.append(b)

	# Shots whose image comes from a mod PNG ("shots" table): direct drawing.
	_mod_shots = Node2D.new()
	_mod_shots.name = "ModShots"
	_mod_shots.z_index = Z.PLAYER_SHOTS
	_mod_shots.draw.connect(
		func() -> void:
			for it in _mod_shot_items:
				_mod_shots.draw_texture(it[0], it[1])
	)
	add_child(_mod_shots)

	# Explosions: explosionSpriteSheet = newsh6 (sprite.c), image sprite + 1.
	explosion_sheet = TyrianAssets.sheet("newsh6")
	_explosion_node = Node2D.new()
	_explosion_node.name = "Explosions"
	_explosion_node.z_index = Z.EXPLOSIONS
	_explosion_node.draw.connect(_draw_explosions)
	add_child(_explosion_node)
	if sim != null and sim.has_method("hd_player_sprites"):
		_hd_effects = HdPlayfieldEffects.new(self, sim, map, _explosion_node, Z, SIM_X_TO_SCREEN)


func _build_hud() -> void:
	_hud = Node2D.new()
	_hud.name = "Hud"
	_hud.z_index = Z.HUD
	_hud.draw.connect(_draw_hud)
	add_child(_hud)
	# HudRenderer: the C's dashboard, one or two players (pic 3 or 6).
	if sim != null and sim.has_method("hud_state"):
		_hud_vga = HudRenderer.new()
		_hud_vga.name = "HudVga"
		_hud.add_child(_hud_vga)
	_debug_text = TyrianText.new()
	_debug_text.font = TyrianText.FontKind.SMALL
	_debug_text.position = Vector2(2, 2)
	_debug_text.visible = false
	_hud.add_child(_debug_text)
	# Endless HUD (endless.c:endless_draw_hud, mainint.c:JE_inGameDisplays):
	# drawn every frame by _draw_endless_hud.
	if EndlessSession.active():
		# Real clock (the C's SDL_GetTicks), started at the first frame of the level.
		_endless_intro_start_ms = 0
	# Mode timer (tyrian2.c: "%d.%d" in SMALL_FONT at (100, 2));
	# lives and player labels: HudRenderer (JE_inGameDisplays).
	_timer_text = TyrianText.new()
	_timer_text.font = TyrianText.FontKind.SMALL
	_timer_text.position = Vector2(100, 2)
	_hud.add_child(_timer_text)
	# mainint.c:JE_pauseGame: "JE_dString(VGAScreenSeg, 120, 90, miscText[22], FONT_SHAPES)".
	_pause_label = TyrianText.new()
	_pause_label.font = TyrianText.FontKind.LARGE
	_pause_label.text = TyrianText.L("miscText_22")
	_pause_label.position = Vector2(120, 90)
	_pause_label.visible = false
	_hud.add_child(_pause_label)


var _timer_text: TyrianText


## Mode HUD texts, updated on every refresh.
func _update_mode_hud() -> void:
	if _timer_text == null or sim == null:
		return
	var h: Dictionary = sim.mode_hud()
	var countdown := int(h.get("level_timer_countdown", 0))
	# 8-bit composition: drawn by TyrianSim (tyrian2.c, "Level Timer"),
	# also in HD (AI) by the HdOverlay layer.
	var hd_overlay := _hd_overlay != null and _hd_overlay.visible
	_timer_text.visible = (
		_playfield_bg == null and not hd_overlay and bool(h.get("level_timer", false)) and countdown > 0
	)
	if _timer_text.visible:
		_timer_text.text = "%d.%d" % [countdown / 100, (countdown / 10) % 10]


func _physics_process(_delta: float) -> void:
	if _entry_fade_active():
		return
	if not interactive or paused or _quit_box != null or _pause_menu != null or _help_screen != null:
		# mainint.c:JE_inGameSetup presents the screen on every turn (JE_showVGA) and
		# video.c:449 decays the shake there: the menu does not stay shifted.
		if interactive and not paused and sim != null and sim.has_method("shake_present_frame"):
			Effects.shake_px = sim.shake_present_frame()
		return
	if demo:
		_demo_tick()
		return
	# tyrian2.c: "ESC quits replay" (replay_stop), return to the title.
	if replaying and Input.is_action_just_pressed("tyrian_level_exit"):
		set_physics_process(false)
		Game.goto_scene(Game.SCENE_TITLE)
		return
	# Escape or Back: pause menu (tyrian2.c: ingamemenu_pressed,
	# INPUT_MENU_CANCEL -> JE_doInGameSetup); not ui_cancel: it includes the
	# gamepad B button, which here is "weapon switch". On the
	# "Game Over" screen, Escape is a key like any other: it closes it
	# (tyrian2.c, "if (allPlayersGone) nothing", newkey).
	if (
		Input.is_action_just_pressed("tyrian_level_exit")
		and Engine.get_physics_frames() > _resumed_frame + 1
		and not _game_over_shown()
	):
		open_pause_menu()
		_any_key_pressed = false
		return
	# tyrian2.c, start_level: "A recording stops at the end of the level".
	if sim != null and sim.level_ended():
		stop_recording()
	if EndlessSession.active() and sim != null and sim.level_ended():
		set_physics_process(false)
		EndlessSession.level_finished(sim, self)
		return
	# tyrian2.c: "During replay playback, return to menu instead of
	# continuing" (end of level or file exhausted, replay_stop).
	if replaying and (sim == null or sim.level_ended()):
		set_physics_process(false)
		Game.goto_scene(Game.SCENE_TITLE)
		return
	# Slow motion (bullet time, end of boss): tyrian2.c:741 divides the loop delay
	# by game_speed; here after Escape/pause so as not to lose any press.
	if is_instance_valid(Effects) and not Effects.consume_tick():
		return
	# tyrian2.c, GAME OVER block: "newkey || button[0..2] || newmouse" ends
	# the level; the simulation reads newkey (set_new_key) like fire.
	# Mouse accumulated since the previous tick (keyboard.c:mouseGetRelativePosition);
	# the simulation only applies it to the player whose device is 0 or 2.
	# During the playback of a replay, only the recorded inputs count.
	if sim != null:
		sim.set_new_key(_any_key_pressed and not replaying)
	if sim != null and not replaying:
		_take_mouse_delta()
		sim.set_mouse(_mouse_mask, _mouse_dx, _mouse_dy)
		_trace_mouse = Vector2i(_mouse_dx, _mouse_dy)
		# Analog stick (player_movement.c, joystick[j].analog): proportional
		# movement, added to the mouse movement by the simulation.
		var a1 := TyrianControls.read_analog(1)
		var a2 := TyrianControls.read_analog(2) if _two_players() else Vector2i.ZERO
		sim.set_analog(a1.x, a1.y, a2.x, a2.y)
	_any_key_pressed = false
	_mouse_dx = 0
	_mouse_dy = 0
	var p2 := Game.read_input_mask_p2() if _two_players() else 0
	var mask := Game.read_input_mask()
	# mainint.c: the achievements list consumes the arrows (INPUT_MENU_*), the
	# ship does not move while it is browsed.
	if hotkeys.overlays.achievements_active:
		mask &= ~(Game.MASK_UP | Game.MASK_DOWN | Game.MASK_LEFT | Game.MASK_RIGHT)
	step_sim(mask, p2)
	_trace_tick()
	if EndlessSession.active():
		_hud.queue_redraw()
	# Level launched from "Side modes" (neither campaign, nor Endless, nor demo, nor
	# playback): without this exit, the "Game Over" screen stayed frozen after the
	# key that closes it (tyrian2.c: reallyEndLevel), with no way back. A
	# level finished alive goes through the debriefing (JE_endLevelAni) then the
	# title; it stayed frozen on the last frame, the sound was not the cause.
	if (
		interactive
		and not Game.campaign_active
		and not demo
		and not replaying
		and not EndlessSession.active()
		and not Capture.active
		and not Bench.active
		and not CampaignBot.active
		and sim != null
		and sim.level_ended()
		and not _campaign_done
	):
		if _game_over_shown():
			_campaign_done = true
			await _fade_out_before_leaving()
			_leave_level()
		elif bool(_finished_level_stats().get("any_player_alive", false)):
			_campaign_done = true
			_freeze_for_debrief()
			Game.side_mode_level_finished(_finished_level_stats())
		return
	if Game.campaign_active and sim != null and sim.level_ended() and not _campaign_done:
		_campaign_done = true
		var stats := _finished_level_stats()
		# tyrian2.c:325: death in a normal level (not bonus) = end of the game, stats_run_end.
		if (
			_ach_active
			and not bool(stats["any_player_alive"])
			and not Game.bonus_level_current
			and not replaying
			and not demo
		):
			sim.ach_run_end()
		_freeze_for_debrief()
		# tyrian2.c:325-327: all dead in a normal level (not bonus) = branch
		# "stats_run_end(); fade_song(); fade_black(10);" before JE_loadGame(11).
		if not bool(stats["any_player_alive"]) and not Game.bonus_level_current:
			await _fade_out_before_leaving()
		Game.campaign_level_finished(stats)


## End-of-level summary handed to Game (money, carried equipment, survivors).
func _finished_level_stats() -> Dictionary:
	var stats: Dictionary = sim.level_stats()
	stats["cash"] = int(sim.player_dict(0).get("cash", 0))
	if _two_players():
		stats["cash2"] = int(sim.player_dict(1).get("cash", 0))
	stats["carry"] = sim.campaign_carry()
	# tyrian2.c:JE_main tests all_players_dead(): in two-player mode, the
	# second player being alive is enough to continue the campaign.
	if not stats.has("any_player_alive"):
		stats["any_player_alive"] = (
			bool(stats.get("all_players_alive", true))
			or (_two_players() and sim.player(1).size() > 4 and sim.player(1)[4] != 0)
		)
	return stats


## tyrian2.c:327: fade_black(10) on the last presented image (palette towards
## black in 10 setDelay(1) steps) and fade_song(), then the shop.
func _fade_out_before_leaving() -> void:
	if not interactive:
		return
	Music.fade_song()
	var veil := ColorRect.new()
	veil.name = "LeaveFade"
	veil.color = Color(0, 0, 0, 0)
	veil.size = Vector2(320, 200)
	veil.mouse_filter = Control.MOUSE_FILTER_IGNORE
	veil.z_index = 200
	add_child(veil)
	var step_s: float = TyrianScreen.DELAY_PERIOD
	for i in range(1, LEAVE_FADE_STEPS + 1):
		await get_tree().create_timer(step_s).timeout
		veil.color.a = float(i) / LEAVE_FADE_STEPS


const LEAVE_FADE_STEPS := 10


## mainint.c:JE_endLevelAni writes the summary over the last image of the
## level (frozen VGAScreen): it is recomposed in the shared VGA, which
## the debriefing screen inherits (same mechanism as pause).
func _freeze_for_debrief() -> void:
	var frozen: Node = preload("res://scripts/frozen_level_screen.gd").new()
	Game.debrief_frozen = frozen.freeze(self)
	frozen.free()


## Mouse (block 5): relative movement in game pixels, buttons according to
## config.c:defaultMouseSettings {1, 4, 5} -> left fire, right both
## sidekicks, middle weapon switch (keyboard.c, SDL_MOUSEBUTTONDOWN).
var _mouse_mask := 0
var _mouse_dx := 0
var _mouse_dy := 0
## keyboard.c:mouseWindowXRelative/YRelative: raw movement accumulated in
## window pixels (xrel) since the previous tick.
var _mouse_raw := Vector2i.ZERO
## True right after the pointer capture: the first movement (recentering) is ignored.
var _ignore_next_motion := false


## keyboard.c, SDL_MOUSEBUTTONDOWN: mask of the function set for the button
## (Setup.settings.mouse_buttons, Options > Mouse); 0 for any other button.
static func mouse_button_mask(button_index: int) -> int:
	var slot: int = {MOUSE_BUTTON_LEFT: 0, MOUSE_BUTTON_RIGHT: 1, MOUSE_BUTTON_MIDDLE: 2}.get(button_index, -1)
	if slot < 0:
		return 0
	match int(SetupScreen.settings["mouse_buttons"][slot]):
		1:
			return Game.MASK_FIRE
		2:
			return Game.MASK_LEFT_SIDEKICK
		3:
			return Game.MASK_RIGHT_SIDEKICK
		4:
			return Game.MASK_LEFT_SIDEKICK | Game.MASK_RIGHT_SIDEKICK
		5:
			return Game.MASK_CHANGE_FIRE
	return 0


## keyboard.c:mouseGetRelativePosition: the RAW window pixels received
## since the previous tick (accumulated xrel), converted ONCE into game pixels
## by video.c:scaleWindowDistanceToScreen with the actual size of the displayed
## image, then the accumulator restarts from zero. Converting each event
## separately (relative already divided by the scale) lost the small
## movements: a 1000 Hz mouse sends steps of 1-2 px.
##
## Scaled Windows (125%, 150%): the C executable is not "DPI
## aware" (src/opentyrian2000.manifest). Windows announces a window in
## LOGICAL pixels (physical size × 96/dpi) to it; it places its image there with its
## own rules (video.c:calc_dst_render_rect) and divides the raw mouse
## counts, which are not scaled, by this logical image.
## Exported Godot is DPI aware: it sees the window in physical pixels.
## Measured on the maintainer's Windows at 125% (instrumented C c-final, bursts
## of 1 px at 1000 Hz): fullscreen 1920x1080, the C divides by 1280 (x4 in
## 1536x864 logical); window 1200x750, by 960 (x3 in 960x600). Dividing
## by the physical image (1600, 900) made Godot 20% too slow in
## fullscreen; dividing it by 1.25 made it 23% too fast in a window.
func _take_mouse_delta() -> void:
	var out_size := Vector2i(320, 200)
	if is_instance_valid(Effects):
		out_size = Effects.output_rect_size()
		var dpi := _screen_dpi()
		if dpi > 96:
			out_size = c_output_rect_size(
				logical_output_size(get_window().size, dpi),
				DisplaySettings.scaling_mode(),
				DisplaySettings.scaler_factor()
			)
	var d := scale_window_distance_to_screen(_mouse_raw, out_size)
	_mouse_raw = Vector2i.ZERO
	_mouse_dx = d.x
	_mouse_dy = d.y


## video.c:calc_dst_render_rect: size of the C's image in a window of
## `win` pixels, for Scaling Mode `mode` (0 Center ... 5 Fill) and the scaler
## factor `k` (Center: native texture k × 320×200).
static func c_output_rect_size(win: Vector2i, mode: int, k: int) -> Vector2i:
	match mode:
		0:
			return Vector2i(320, 200) * k
		1:
			var r := Vector2i(320, 200)
			while r.x + 320 <= win.x and r.y + 200 <= win.y:
				r += Vector2i(320, 200)
			return r
		2, 3, 4:
			# Ratios 8:5, 4:3, 16:9; the C computes in float then truncates (here in
			# integers: 864 × 16/9 does give 1536, not 1535 as in double).
			var num: int = [8, 4, 16][mode - 2]
			var den: int = [5, 3, 9][mode - 2]
			var maxh_width := win.y * num / den
			var maxw_height := win.x * den / num
			if maxh_width > win.x:
				return Vector2i(win.x, maxw_height)
			return Vector2i(maxh_width, win.y)
	return win


## Effective DPI of the window's screen under Windows (120 to 125%, 144 to
## 150%); 96 elsewhere: under X11 the C and Godot see the same pixels.
## --screen-dpi=N forces it (player-smoke measures the 125% case this way under Xvfb).
static func _screen_dpi() -> int:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--screen-dpi="):
			return maxi(int(arg.trim_prefix("--screen-dpi=")), 96)
	if OS.get_name() != "Windows" or DisplayServer.get_name() == "headless":
		return 96
	return maxi(DisplayServer.screen_get_dpi(DisplayServer.window_get_current_screen()), 96)


## Size in logical pixels (those of a non-DPI-aware program) of a window
## of `physical` pixels on a screen at `dpi`.
static func logical_output_size(physical: Vector2i, dpi: int) -> Vector2i:
	if dpi <= 96:
		return physical
	return Vector2i(roundi(physical.x * 96.0 / dpi), roundi(physical.y * 96.0 / dpi))


## video.c:scaleWindowDistanceToScreen, bit-exact (the C's integer division,
## truncated toward zero): x = (2x + 1) × 320 / (2 × displayed width).
static func scale_window_distance_to_screen(raw: Vector2i, out_size: Vector2i) -> Vector2i:
	var w := maxi(out_size.x, 1)
	var h := maxi(out_size.y, 1)
	return Vector2i(_c_div((2 * raw.x + 1) * 320, 2 * w), _c_div((2 * raw.y + 1) * 200, 2 * h))


## The C's integer division (truncation toward zero), for negative values.
static func _c_div(a: int, b: int) -> int:
	var q := absi(a) / absi(b)
	return q if (a >= 0) == (b >= 0) else -q


## --input-trace=<file>: one line per tick played (tick, mouse received by the
## simulation dx dy, ship position x y). Measures mouse speed
## against the C (player-smoke.sh, input parity report).
var _trace_file: FileAccess
var _trace_mouse := Vector2i.ZERO
var _trace_ticks := 0


func _trace_tick() -> void:
	if _trace_file == null:
		if _trace_ticks < 0:
			return
		var path := ""
		for arg in OS.get_cmdline_user_args():
			if arg.begins_with("--input-trace="):
				path = arg.trim_prefix("--input-trace=")
		if path == "":
			_trace_ticks = -1
			return
		_trace_file = FileAccess.open(path, FileAccess.WRITE)
		if _trace_file == null:
			_trace_ticks = -1
			return
	_trace_ticks += 1
	var p: PackedInt32Array = sim.player(0) if sim != null else PackedInt32Array([0, 0])
	_trace_file.store_line(
		"%d %d %d %d %d %d" % [_trace_ticks, _trace_mouse.x, _trace_mouse.y, p[0], p[1], _mouse_mask]
	)
	_trace_file.flush()
	_trace_mouse = Vector2i.ZERO


## SDL delivers each raw mouse movement, and Windows' gain applies
## to each one (SDL_mouse.c:GetScaledMouseDelta). Godot otherwise groups all
## the movements of a frame into one: the curve would see a large step (high
## gain) and the speed would depend on frames per second. During a played
## level, events therefore arrive one by one; menus keep the
## grouping.
func _set_raw_mouse_events(on: bool) -> void:
	if interactive:
		Input.use_accumulated_input = not on


func _two_players() -> bool:
	return sim != null and bool(sim.mode_hud().get("two_player", false))


func _input(event: InputEvent) -> void:
	if not interactive:
		return
	if event is InputEventMouseMotion:
		# keyboard.c, SDL_MOUSEMOTION: "mouseWindowXRelative += ev.motion.xrel"
		# in relative mode, active window. screen_relative = raw window
		# pixels (relative is already divided by the scale, and fractional).
		if _ignore_next_motion:
			_ignore_next_motion = false
		else:
			# SDL (relative mode, SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE set by
			# keyboard.c) first applies Windows' pointer gain.
			MouseSystemScale.ensure_loaded()
			_mouse_raw += MouseSystemScale.apply(
				Vector2i(roundi(event.screen_relative.x), roundi(event.screen_relative.y))
			)
	elif event is InputEventMouseButton and mouse_button_mask(event.button_index) != 0:
		if event.pressed:
			_mouse_mask |= mouse_button_mask(event.button_index)
		else:
			_mouse_mask &= ~mouse_button_mask(event.button_index)


## The C's newkey (keyboard.c): a key (or a click) since the last tick.
var _any_key_pressed := false

## Player pause (mainint.c:JE_pauseGame): no simulation tick,
## music at half volume (set_volume(tyrMusicVolume / 2)), "PAUSED"
## (miscText[22]) in FONT_SHAPES at (120, 90). It is not a simulation
## input: the golden masters do not see it.
var paused := false
var _pause_label: TyrianText
## Physics frame of the resume: the key that resumes (Escape included,
## like in the C where any key resumes) must not also quit the level.
var _resumed_frame := -10


func set_paused(on: bool) -> void:
	if on == paused:
		return
	paused = on
	# "PAUSED" through the 8-bit dashboard (level palette, like JE_dString).
	# the RGBA label is only used without the extension.
	if _hud_vga != null:
		_hud_vga.pause_text = TyrianText.L("miscText_22") if on else ""
		_hud_vga.refresh(sim)
	elif _pause_label != null:
		_pause_label.visible = on
	# JE_pauseGame: set_volume(tyrMusicVolume / 2), then tyrMusicVolume on resume.
	Music.set_volume(Music.user_volume / 2 if on else Music.user_volume)
	if not on:
		_resumed_frame = Engine.get_physics_frames()


## "Game Over" screen (or final explosion): tyrian2.c, allPlayersGone.
## Escape is a key like any other there, not an exit request.
func _game_over_shown() -> bool:
	return sim != null and bool(sim.mode_hud().get("all_players_gone", false))


# --- Exit confirmation (Escape / Back) -----------------------------------

## "Are you sure you want to exit?" box of game_menu.c:JE_quitRequest
## (the one the C shows before abandoning the game from the pause
## menu), "No" preselected; null if closed. The simulation is stopped
## while it is open.
var _quit_box: Node2D
var _quit_box_screen: TyrianScreen
var _quit_box_sprite: Sprite2D
var _quit_yes := false


## Opens the box: drawn by the same calls as JE_quitRequestMsg
## (TyrianScreen.draw_quit_box) in a dedicated TyrianVga, with the game
## palette (HUD pic), then placed as a layer above the level. JE_barShade
## (darkening of the image under the box) is rendered by a black veil.
func _confirm_exit() -> void:
	if not ClassDB.class_exists("TyrianVga"):
		_leave_level()
		return
	_quit_yes = false
	_quit_box = Node2D.new()
	_quit_box.name = "QuitBox"
	_quit_box.z_index = Z.HUD + 1
	var shade := ColorRect.new()
	shade.color = Color(0, 0, 0, 0.5)
	shade.position = Vector2(65, 55)
	shade.size = Vector2(255 - 65 + 1, 155 - 55 + 1)
	_quit_box.add_child(shade)
	_quit_box_screen = TyrianScreen.new()
	_quit_box_screen.vga = ClassDB.instantiate("TyrianVga")
	_quit_box.add_child(_quit_box_screen)
	_quit_box_screen.vga.load_pic(0, HUD_PIC_TWO_PLAYERS if _two_players() else HUD_PIC, true)
	_quit_box_sprite = Sprite2D.new()
	_quit_box_sprite.centered = false
	_quit_box_sprite.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_quit_box.add_child(_quit_box_sprite)
	add_child(_quit_box)
	_draw_quit_box()


## One frame of the box: drawn on two surfaces cleared to 0 and to 1;
## only identical pixels are opaque (TyrianVga.hud_overlay).
func _draw_quit_box() -> void:
	var vga: RefCounted = _quit_box_screen.vga
	for s in [1, 2]:
		vga.clear(s, s - 1)
		_quit_box_screen.draw_quit_box(TyrianText.L("miscText_28"), TyrianText.L("miscText_30"), _quit_yes, s)
	var img: Image = vga.hud_overlay(0, 0, 320, 200, 0)
	_quit_box_sprite.texture = ImageTexture.create_from_image(img)
	# HD mode: box sharply upscaled (xBRZ), like the other texts.
	HdInterface.new(_quit_box_sprite, null).update(0, img)


## JE_quitRequestMsg: Left / Right / Tab choose, Y / N answer,
## Enter / Space confirm; Escape, B or Back (again) = "No".
func _quit_box_input(event: InputEvent) -> void:
	# Hover: the pointer on "Yes" or "No" (same zones as the click)
	# moves the selection, like in the other mouse menus.
	if event is InputEventMouseMotion and not TyrianMouse.inactive and event.relative != Vector2.ZERO:
		var hovered: Variant = TyrianScreen.quit_box_answer(event.position)
		if hovered != null and bool(hovered) != _quit_yes:
			_quit_yes = bool(hovered)
			Sfx.play(Sfx.CURSOR)
			_draw_quit_box()
		return
	if TyrianMouse.is_button_press(event):
		# game_menu.c:JE_quitRequestMsg: any button, Yes if 56 < x < 142,
		# No if 151 < x < 237 (123 < y < 149); elsewhere the box stays.
		var answer: Variant = TyrianScreen.quit_box_answer(event.position)
		if answer != null:
			_close_quit_box(bool(answer))
		return
	if event.is_action_pressed("tyrian_level_exit", false, true):
		_close_quit_box(false)
		return
	match TyrianControls.menu_action(event):
		TyrianControls.LEFT:
			if not _quit_yes:
				Sfx.play(Sfx.CURSOR)
			_quit_yes = true
		TyrianControls.RIGHT:
			if _quit_yes:
				Sfx.play(Sfx.CURSOR)
			_quit_yes = false
		TyrianControls.TOGGLE:
			_quit_yes = not _quit_yes
			Sfx.play(Sfx.CURSOR)
		TyrianControls.YES:
			_close_quit_box(true)
			return
		TyrianControls.NO, TyrianControls.BACK:
			_close_quit_box(false)
			return
		TyrianControls.ACCEPT, TyrianControls.SELECT:
			_close_quit_box(_quit_yes)
			return
		_:
			return
	_draw_quit_box()


func _close_quit_box(leave: bool) -> void:
	Sfx.play(Sfx.SPRING if leave else Sfx.CLICK)
	_quit_box.queue_free()
	_quit_box = null
	_quit_box_screen = null
	_quit_box_sprite = null
	_mouse_raw = Vector2i.ZERO  # the mouse moved in the box does not move the ship
	if leave:
		_leave_level()
	else:
		# The key that closes the box (Escape, Back) does not reopen it.
		_resumed_frame = Engine.get_physics_frames()


func _leave_level() -> void:
	EndlessSession.stop()
	Game.goto_scene(Game.SCENE_TITLE)


## JE_pauseGame: resume on a key (except Ctrl and Alt alone, mainint.c:3284; Shift alone resumes) or a
## click; here also on a gamepad button (push_joysticks_as_keyboard).
static func _resumes_pause(event: InputEvent) -> bool:
	if event is InputEventKey:
		return event.pressed and not event.echo and not event.physical_keycode in [KEY_CTRL, KEY_ALT]
	return (event is InputEventJoypadButton and event.pressed) or TyrianMouse.is_button_press(event)


## tyrian2.c: "if (pause_pressed || !windowHasFocus) JE_pauseGame();": a
## window that loses focus pauses the game.
func _notification(what: int) -> void:
	# Level exited (Escape, exit box, title): the recording closes
	# as at the end of the level, the file is written.
	if what == NOTIFICATION_PREDELETE or what == NOTIFICATION_EXIT_TREE:
		stop_recording()
	if what == NOTIFICATION_EXIT_TREE:
		_set_raw_mouse_events(false)
	# hitstop.c:hitstop_reset (level exited): end of the boss slow motion.
	if what == NOTIFICATION_EXIT_TREE:
		Effects.kill_slowmo = -1.0
		# tyrian2.c: screenshake_reset on level change.
		Effects.shake_px = Vector2i.ZERO
	if (
		what == NOTIFICATION_APPLICATION_FOCUS_OUT
		and interactive
		and not demo
		and sim != null
		and not sim.level_ended()
		and _pause_menu == null
		and _help_screen == null
		and not Effects.leaving_fullscreen
	):
		set_paused(true)
	# Level exited while paused: the music gets its volume back (end of JE_pauseGame).
	elif what == NOTIFICATION_EXIT_TREE and paused:
		Music.set_volume(Music.user_volume)
	# Outside a level, Normal pace (tyrian2.c resets the speed on leaving a level).
	if what == NOTIFICATION_EXIT_TREE and _game_speed_applied:
		GAME_SPEED.restore()


## tyrian2.c:1524-1540: key, click, gamepad button or gamepad direction
## (joystick.c: stick beyond half of its travel, d-pad) ends
## the demo.
static func demo_stop_event(event: InputEvent) -> bool:
	if event is InputEventJoypadMotion:
		return absf((event as InputEventJoypadMotion).axis_value) > 0.5
	return (
		(event is InputEventKey or event is InputEventJoypadButton or TyrianMouse.is_button_press(event))
		and event.pressed
		and not event.is_echo()
	)


func _unhandled_input(event: InputEvent) -> void:
	if interactive and demo:
		# tyrian2.c: "if (newkey || newmouse || joy_pressed) { reallyEndLevel
		# = true; stopped_demo = true; }"; neither pause nor exit box.
		if demo_stop_event(event):
			_demo_key = true
			get_viewport().set_input_as_handled()
		return
	if interactive and (_pause_menu != null or _help_screen != null):
		return  # screens laid on top: they receive the keys before the level
	# dev_console.c (--devmode): Ctrl+Shift+G/H/C/N/U.
	if interactive and DEV_CONSOLE.handle(event, self):
		get_viewport().set_input_as_handled()
		return
		return
	if interactive and _quit_box != null:
		_quit_box_input(event)
		if is_inside_tree():  # "Yes": the level has just been exited
			get_viewport().set_input_as_handled()
		return
	if interactive and paused:
		if _resumes_pause(event):
			set_paused(false)
			get_viewport().set_input_as_handled()
		return
	# mainint.c: "pause_pressed || keysactive[SDL_SCANCODE_P] || input_held(INPUT_PAUSE)":
	# P always works, the pause key (F2 by default) is reconfigurable.
	if (
		interactive
		and sim != null
		and not sim.level_ended()
		and (
			TyrianControls.global_pressed(event, TyrianControls.PAUSE)
			or (event is InputEventKey and event.pressed and not event.echo and event.physical_keycode == KEY_P)
		)
	):
		set_paused(true)
		get_viewport().set_input_as_handled()
		return
	# The C's level shortcuts (input_map.c:enhanced_keys[]): achievements, statistics,
	# DPS, damage numbers, quick save and load, replay, screen
	# capture, gamma, slow motion (level_hotkeys.gd).
	if interactive and sim != null and not sim.level_ended():
		# The viewport is taken beforehand: a confirmed quick load exits the level.
		var viewport := get_viewport()
		if hotkeys.handle(event):
			viewport.set_input_as_handled()
			return
	# The C's newkey / newmouse; Back (gamepad) counts as Escape.
	if (
		(event is InputEventKey and event.pressed and not event.echo)
		or TyrianMouse.is_button_press(event)
		or (event is InputEventJoypadButton and event.is_action_pressed("tyrian_level_exit", false, true))
	):
		_any_key_pressed = true
	# Help key (F1 by default, controls screen): the C's in-game help
	# (mainint.c:JE_mainKeyboardInput -> JE_inGameHelp). With Ctrl: technical
	# help of the rewrite (tick, curLoc), which used to occupy F1 alone.
	if TyrianControls.global_pressed(event, TyrianControls.HELP):
		if event is InputEventKey and event.ctrl_pressed:
			_debug_text.visible = not _debug_text.visible
		elif interactive and sim != null and not sim.level_ended():
			open_help()
			get_viewport().set_input_as_handled()


# --- Replay recording (replay.c) ------------------------------------

## File written by the last closed recording ("" before).
var last_recording_path := ""


## Level start (replay.c:replay_level_start_hook): "REC" at the top right
## of the play area and "Recording started" message if the simulation
## opened the requested recording; otherwise, refusal message for a mode that
## replay.c cannot replay.
func _recording_started() -> void:
	if sim == null or not sim.has_method("is_recording") or _hud_vga == null:
		return
	if sim.is_recording():
		_hud_vga.replay_label = "REC"
		_hud_vga.replay_label_x = 233
		_hud_vga.replay_label_color = 4
		_hud_vga.queue_system_message(TyrianText.L("msg_replay_started"))
	elif _recording_requested:
		_hud_vga.queue_system_message(TyrianText.L("msg_replay_refused"))


## message_bar_queue(text, MSG_PRIORITY_SYSTEM) from a shortcut.
func hud_queue_message(text: String) -> void:
	if _hud_vga != null:
		_hud_vga.queue_system_message(text)


## message_bar_pin / message_bar_unpin (empty text = removes the pin).
func hud_pin_message(text: String) -> void:
	if _hud_vga != null:
		_hud_vga.pinned_message = text
		if text.is_empty():
			_hud_vga.erase_text_window()


## Achievements unlocked outside the simulation (screen capture, slow motion): banner,
## sound and save like in-game unlocks.
func ach_unlocked_many(ids: PackedInt32Array) -> void:
	for id in ids:
		_ach_unlocked(id)


## input_map.c:input_action_quick_load_confirm(true): JE_loadGame of
## the quick slot, then the level ends (reallyEndLevel) and the
## game resumes at the shop of the saved section.
func quick_load_confirmed() -> void:
	var slot := LevelHotkeys.QUICK_SLOT_TWO_PLAYERS if Game.two_player_game() else LevelHotkeys.QUICK_SLOT_ONE_PLAYER
	var slots := Game.campaign_slots()
	var s = slots[slot - 1] if slots.size() >= slot else null
	if s == null or str(s.get("error", "")) != "":
		return
	set_physics_process(false)
	Game.load_campaign_slot(s)


## An armed recording was requested from the simulation for this level.
var _recording_requested := false


## replay.c:replay_start_recording / replay_stop_recording (F10 key). During
## a recording: closes and writes the file; otherwise arms (or disarms) for the
## next level start.
func toggle_recording() -> void:
	if (
		sim == null
		or not sim.has_method("is_recording")
		or demo
		or Capture.active
		or Bench.active
		or CampaignBot.active
	):
		return
	if sim.is_recording():
		# mainint.c: only the manual stop (F10) counts for Filmmaker (replays_saved++).
		var before := last_recording_path
		stop_recording()
		if (
			last_recording_path != before
			and sim != null
			and is_instance_valid(sim)
			and sim.has_method("ach_note_replay_saved")
		):
			ach_unlocked_many(sim.ach_note_replay_saved())
		return
	var key := ReplayRecording.toggle_arm()
	if _hud_vga != null:
		_hud_vga.queue_system_message(TyrianText.L(key))


## Closes the recording in progress and writes the file (nothing if there is none).
func stop_recording() -> void:
	if sim == null or not is_instance_valid(sim) or not sim.has_method("is_recording") or not sim.is_recording():
		return
	var path := ReplayRecording.save(sim.finish_recording())
	if path.is_empty():
		return
	last_recording_path = path
	if _hud_vga != null:
		_hud_vga.replay_label = ""
	print("replay: recording closed, saved to %s" % path)


# --- Pause menu and in-game help (mainint.c) ---------------------------------

const PAUSE_MENU := preload("res://scripts/pause_menu.gd")
const GAME_SPEED := preload("res://scripts/game_speed.gd")
const DEV_CONSOLE := preload("res://scripts/dev_console.gd")
const IN_GAME_HELP := preload("res://scripts/in_game_help.gd")
## Pause menu (JE_inGameSetup) or help (JE_inGameHelp) open, null otherwise.
var _pause_menu: Node
var _help_screen: Node


## tyrian2.c: "yourInGameMenuRequest = true; JE_doInGameSetup();". Without an
## 8-bit image of the level (extension absent), the exit box alone.
func open_pause_menu() -> void:
	if _pause_menu != null:
		return
	var menu: Node = PAUSE_MENU.new()
	menu.name = "PauseMenu"
	menu.z_index = Z.HUD + 2
	add_child(menu)
	if _hud_vga != null:
		_hud_vga.prepare_frozen_screen(sim)
	if not menu.open_menu(self):
		if _hud_vga != null:
			_hud_vga.release_frozen_screen()
		menu.queue_free()
		_confirm_exit()
		return
	_pause_menu = menu
	menu.closed.connect(_on_pause_menu_closed)


## JE_doInGameSetup: "Quit" confirmed -> reallyEndLevel and playerEndLevel.
func _on_pause_menu_closed(quit: bool) -> void:
	# Quit: the C fades to black the image where the menu and the box still are
	# (VGAScreen is not redrawn); the menu leaves with the scene.
	if not quit:
		_pause_menu.queue_free()
	_pause_menu = null
	if _hud_vga != null:
		_hud_vga.release_frozen_screen()
	# input_consume(INPUT_MENU_CANCEL): the key that closes does not reopen;
	# the mouse moved in the menu does not move the ship.
	_resumed_frame = Engine.get_physics_frames()
	_any_key_pressed = false
	_mouse_dx = 0
	_mouse_dy = 0
	_mouse_raw = Vector2i.ZERO
	if quit:
		_quit_level()


## tyrian2.c:JE_main after playerEndLevel: in campaign, JE_loadGame(11)
## ("LAST LEVEL") and return to the game menu of this section; in Endless,
## end of game (endless.c:endless_on_level_end, exit without endLevel:
## score, leaderboard, title); otherwise, return to the title.
func _quit_level() -> void:
	set_physics_process(false)
	if Game.campaign_active:
		if _ach_active and not replaying and not demo:
			sim.ach_run_end()
		# tyrian2.c:325-327: abandon (playerEndLevel) = same branch as death,
		# "fade_song(); fade_black(10);" before JE_loadGame(11).
		await _fade_out_before_leaving()
		Game.campaign_abandon_level()
	elif EndlessSession.active():
		EndlessSession.quit_run(sim)
	else:
		_leave_level()


## mainint.c:JE_inGameHelp (F1).
func open_help() -> void:
	if _help_screen != null or _pause_menu != null:
		return
	var help: Node = IN_GAME_HELP.new()
	help.name = "InGameHelp"
	help.z_index = Z.HUD + 2
	add_child(help)
	if _hud_vga != null:
		_hud_vga.prepare_frozen_screen(sim)
	if not help.open_help(self):
		if _hud_vga != null:
			_hud_vga.release_frozen_screen()
		help.queue_free()
		return
	_help_screen = help
	help.closed.connect(_on_help_closed)


## "textErase = 1": the dashboard's message area is cleared at the
## next tick.
func _on_help_closed() -> void:
	_help_screen.queue_free()
	_help_screen = null
	_resumed_frame = Engine.get_physics_frames()
	_any_key_pressed = false
	if _hud_vga != null:
		_hud_vga.release_frozen_screen()
		_hud_vga.erase_text_window()


## One demo tick: the keys come from demo.N (TyrianSim), a
## player key or the end of the level returns to the title (Game.end_demo).
func _demo_tick() -> void:
	if _demo_done:
		return
	if not _demo_key and sim != null and not sim.level_ended():
		step_sim(0, 0)
	if _demo_key or sim == null or sim.level_ended():
		_demo_done = true
		set_physics_process(false)
		var cash := int(sim.player_dict(0).get("cash", 0)) if sim != null else Game.demo_cash
		var carry: PackedInt32Array = sim.campaign_carry() if sim != null else PackedInt32Array()
		Game.end_demo.call_deferred(cash, carry)


## One simulation tick with the masks of players 1 and 2, then display update.
func step_sim(mask: int, p2_mask := 0) -> void:
	if sim != null and not sim.level_ended():
		if sim.has_method("set_fx_bullet_time"):
			sim.set_fx_bullet_time(Effects.bullet_time_active())
		sim.step(mask, p2_mask)
		if _endless_intro_start_ms == 0 and EndlessSession.active():
			_endless_intro_start_ms = Time.get_ticks_msec()
		_fx_tick()
		_music_tick()
		_ach_tick()
		if _hud_vga != null:
			_hud_vga.wave_intro_elapsed_ms = _endless_intro_elapsed_ms()
			_hud_vga.tick(sim)
		if _playfield_bg != null:
			# Image filters (water, lava, blurs): blend each frame with the previous one.
			sim.playfield_filters_tick(Game.data_dir)
	_refresh()


## Player effects (Setup): not for the capture, measurement tools nor
## the campaign bot, which reproduce the golden master.
func _player_effects() -> bool:
	return not (Capture.active or Bench.active or CampaignBot.active)


## Setup effect settings changed during the level (setup.gd). A
## Visual Effects change (pause menu) applies right away to the
## level being played, like mainint.c:vfx_row_act then JE_initProcessorType; a
## playback keeps the processorType of its recording.
func apply_effects() -> void:
	if sim != null and _player_effects():
		SetupScreen.apply_effects_to(sim)
		if not replaying and sim.has_method("set_processor_type") and sim.processor_type() != Game.processor_type:
			sim.set_processor_type(Game.processor_type)


## rumble.c:rumble_trigger: each plugged-in gamepad, two motors at the same
## intensity (SDL_JoystickRumble(strength, strength, duration)). Replaceable
## by the tests.
var joy_devices := Callable(Input, "get_connected_joypads")
var joy_vibrate := Callable(Input, "start_joy_vibration")


## After a tick: requested vibrations (rumble.c) and boss slow motion
## (hitstop.c:kill_slowmo_update).
func _fx_tick() -> void:
	if not sim.has_method("take_rumble"):
		return
	var r: PackedFloat32Array = sim.take_rumble()
	for k in range(0, r.size() - 1, 2):
		for device in joy_devices.call():
			joy_vibrate.call(device, r[k], r[k], r[k + 1] / 1000.0)
	Effects.kill_slowmo = sim.kill_slowmo_speed()
	# screenshake.c: whole-pixel offset of this frame (render draw, ADR 0003).
	if sim.has_method("shake_offset"):
		Effects.shake_px = sim.shake_offset()


## Advances `ticks` ticks without waiting for rendering (capture tool).
## `mask_at`: Callable(tick_index) -> mask.
## `mouse_at` (optional): Callable(tick_index) -> [mask, dx, dy] mouse
## (mmask/mdx/mdy columns of the v2 golden masters).
func advance(ticks: int, mask_at: Callable, mouse_at := Callable()) -> void:
	for i in ticks:
		if sim == null or sim.level_ended():
			break
		if mouse_at.is_valid():
			var m: Array = mouse_at.call(i)
			sim.set_mouse(m[0], m[1], m[2])
		sim.step(mask_at.call(i), 0)
		_music_tick()
		_ach_tick()
		if _hud_vga != null:
			_hud_vga.tick(sim)
		if _playfield_bg != null:
			# Image filters (water, lava, blurs): blend each frame with the previous one.
			sim.playfield_filters_tick(Game.data_dir)
	_refresh()


func _refresh() -> void:
	_update_layers()
	_update_enemies()
	_update_shots()
	_update_effects_and_sounds()
	_player_node.queue_redraw()
	_explosion_node.queue_redraw()
	_hud.queue_redraw()
	_update_mode_hud()
	if _hud_vga != null:
		_hud_vga.refresh(sim)
	if _debug_text.visible and sim != null:
		_debug_text.text = "tick %d  curloc %d" % [sim.tick(), sim.cur_loc()]  # locale-exempt: debug overlay


# --- Scenery ------------------------------------------------------------------


## stats.c:stats_display / achievements.c:achievements_display: the darkened
## band of the play area (JE_barShade ×2). Indexed rendering: composed in
## the image (playfield_background); HD (AI) mode: black veil of the same
## density (two passes = quarter brightness) under the dashboard.
func _update_overlays() -> void:
	var rows := hotkeys.overlays.shade_rows()
	if ClassDB.class_has_method("TyrianSim", "set_playfield_shade"):
		ClassDB.class_call_static("TyrianSim", "set_playfield_shade", rows.x, rows.y)
	if _hud_vga != null:
		_hud_vga.overlays = hotkeys.overlays
		_hud_vga.overlay_sim = sim
	var dim_needed := rows.y >= rows.x and _playfield_bg == null
	if dim_needed and _overlay_dim == null:
		_overlay_dim = ColorRect.new()
		_overlay_dim.name = "OverlayDim"
		_overlay_dim.color = Color(0, 0, 0, 0.75)
		_overlay_dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_overlay_dim.z_index = Z.HUD - 1
		add_child(_overlay_dim)
	if _overlay_dim != null:
		_overlay_dim.visible = dim_needed
		if dim_needed:
			_overlay_dim.position = Vector2(0, rows.x)
			_overlay_dim.size = Vector2(HudRenderer.PLAY_W, rows.y - rows.x + 1)


var _overlay_dim: ColorRect


## The scenery follows exactly the position published by the Rust core
## (tyrian-sim: LayerScroll, from backgrnd.c:draw_background_1/2/3).
func _update_layers() -> void:
	_update_overlays()
	if _playfield_bg != null:
		_playfield_bg.update(sim, map)
	_update_hd_overlays()
	_update_hd_blend()
	for i in 3:
		var l := map.layer(i)
		if l != null and sim != null:
			l.position = -Vector2(sim.layer_scroll(i))
			var hd := l.get_node_or_null("HdMap") as HdMapLayer
			if hd != null and hd.visible:
				hd.set_cells(sim.hd_layer_cells(i))


## Indexed background: 8-bit HUD palette in single player; in two-player
## mode (HUD in nodes), palette of pic 6 (tyrian2.c:JE_main, JE_loadPic(6): same
## palette 5 as pic 3). Only with the original tiles (no HD atlas,
## ADR 0100).
func _start_playfield_background() -> void:
	if map == null or map.layer(0) == null or DisplaySettings.hd_sprites() or sim == null:
		return
	var vga: RefCounted = _hud_vga.vga if _hud_vga != null else _two_player_palette()
	if vga == null:
		return
	_playfield_bg = PlayfieldBackground.new()
	_playfield_bg.name = "PlayfieldBackground"
	_playfield_bg.z_index = Z.LAYER1
	add_child(_playfield_bg)
	if not _playfield_bg.start(sim, map, vga):
		_playfield_bg.queue_free()
		_playfield_bg = null


## TyrianVga whose hardware palette is that of pic 6 (two players), or null.
func _two_player_palette() -> RefCounted:
	if not _two_players() or not ClassDB.class_exists("TyrianVga"):
		return null
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	return vga if vga.load_pic(0, HUD_PIC_TWO_PLAYERS, true) else null


# --- Enemies ----------------------------------------------------------------


func _sprite_from_pool(layer: int, used: int) -> Sprite2D:
	var pool: Array = _enemy_pool[layer]
	if used < pool.size():
		return pool[used]
	var s := Sprite2D.new()
	s.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_enemy_root[layer].add_child(s)
	pool.append(s)
	return s


func _update_enemies() -> void:
	if sim != null and sim.has_method("hd_enemy_sprites"):
		_update_enemies_drawn(sim.hd_enemy_sprites())
		return
	_update_enemies_from(sim.enemies() if sim != null else PackedInt32Array())


## Enemy pieces that the C drew at this iteration
## (TyrianSim.hd_enemy_sprites, records of 7 integers: layer, sheet,
## x, y, image, mode, filter), not the state after the tick: an enemy outside
## the horizontal window or above the screen, pruned by JE_drawEnemy,
## does not appear. Darkened trails (mode 2): composed by
## PlayfieldBackground, not by these nodes.
func _update_enemies_drawn(data: PackedInt32Array) -> void:
	var used := {0: 0, 1: 0, 2: 0, 3: 0}
	for i in range(0, data.size() - 6, 7):
		if data[i + 5] == 2:
			continue
		var layer := clampi(data[i], 0, 3)
		var sheet := TyrianAssets.sheet(TyrianAssets.enemy_sheet_name(data[i + 1]))
		if sheet == null:
			continue
		var s := _sprite_from_pool(layer, used[layer])
		if sheet.apply(s, data[i + 4], data[i + 2] + SIM_X_TO_SCREEN, data[i + 3]):
			used[layer] += 1
	for layer in 4:
		var pool: Array = _enemy_pool[layer]
		for k in range(used[layer], pool.size()):
			pool[k].visible = false


## `data`: flattened array of TyrianSim.enemies() (ENEMY_STRIDE = 8).
func _update_enemies_from(data: PackedInt32Array) -> void:
	var stride := 8
	var used := {0: 0, 1: 0, 2: 0, 3: 0}
	var i := 0
	while i + stride - 1 < data.size():
		var x := data[i + 1] + SIM_X_TO_SCREEN
		var y := data[i + 2]
		var sheet := TyrianAssets.sheet(TyrianAssets.enemy_sheet_name(data[i + 3]))
		var frame := data[i + 4]
		var size := data[i + 5]
		var layer := clampi(data[i + 6], 0, 3)
		if sheet != null:
			if size >= 2:
				# Large enemy: 4 images (blit_enemy, enemy_render.c: +0, +1, +19, +20).
				var parts := [[0, 0, 0], [1, 12, 0], [19, 0, 14], [20, 12, 14]]
				for p in parts:
					var s := _sprite_from_pool(layer, used[layer])
					if sheet.apply(s, frame + p[0], x + p[1], y + p[2]):
						used[layer] += 1
			else:
				var s := _sprite_from_pool(layer, used[layer])
				if sheet.apply(s, frame, x, y):
					used[layer] += 1
		i += stride
	for layer in 4:
		var pool: Array = _enemy_pool[layer]
		for k in range(used[layer], pool.size()):
			pool[k].visible = false


# --- Shots and explosions -----------------------------------------------------


func _update_shots() -> void:
	# The extension's buffers only know the original sheets: a shot
	# drawn by a mod PNG is not in them (invisible). With such a mod enabled,
	# the path below, which draws the mods' PNGs, is taken.
	if sim != null and sim.has_method("shot_instances") and not Mods.has_textures("shots"):
		# Buffers built by the extension (docs/performance.md).
		_player_shots[0].update_from_sim(sim, false, 0, 500, SIM_X_TO_SCREEN)
		_player_shots[1].update_from_sim(sim, false, 501, 65535, SIM_X_TO_SCREEN)
		_enemy_shots[0].update_from_sim(sim, true, 0, 500, SIM_X_TO_SCREEN)
		_enemy_shots[1].update_from_sim(sim, true, 501, 65535, SIM_X_TO_SCREEN)
		return
	_update_shots_from(
		sim.player_shots() if sim != null else PackedInt32Array(),
		sim.enemy_shots() if sim != null else PackedInt32Array()
	)


## Flattened arrays (x, y, sprite) of TyrianSim.player_shots()/enemy_shots().
func _update_shots_from(ps: PackedInt32Array, es: PackedInt32Array) -> void:
	# Shots drawn by a mod PNG do not go through the original sheets.
	var low := func(s: int) -> bool: return s <= 500 and Mods.texture("shots", s) == null
	var high := func(s: int) -> bool: return s > 500 and Mods.texture("shots", s) == null
	var shift := func(a: PackedInt32Array) -> PackedInt32Array:
		var out := a.duplicate()
		for k in range(0, out.size(), 3):
			out[k] += SIM_X_TO_SCREEN
		return out
	ps = shift.call(ps)
	es = shift.call(es)
	_mod_shot_items.clear()
	for a in [ps, es]:
		for k in range(0, a.size() - 2, 3):
			var tex := Mods.texture("shots", a[k + 2])
			if tex != null:
				_mod_shot_items.append([tex, Vector2(a[k], a[k + 1])])
	_mod_shots.queue_redraw()
	_player_shots[0].update_from(ps, 3, low)
	_player_shots[1].update_from(ps, 3, high)
	_enemy_shots[0].update_from(es, 3, low)
	_enemy_shots[1].update_from(es, 3, high)


func _draw_explosions() -> void:
	if sim == null or explosion_sheet == null:
		return
	var ex: PackedInt32Array = sim.explosions_to_draw()
	for k in range(0, ex.size() - 2, 3):
		explosion_sheet.draw(_explosion_node, ex[k + 2] + 1, ex[k] + SIM_X_TO_SCREEN, ex[k + 1])


## Level loop sounds: simulation queue (TyrianSim.take_sounds,
## loudness.c:JE_flushSoundQueue) with channel, x position and damage magnitude;
## otherwise (extension without a queue) deduced from state changes.
func _update_effects_and_sounds() -> void:
	if sim == null:
		return
	if sim.has_method("take_sounds"):
		var q: PackedInt32Array = sim.take_sounds()
		for k in range(0, q.size() - 3, 4):
			Sfx.play_queued(q[k], q[k + 1], q[k + 2], q[k + 3])
		return
	var counts: Vector3i = (
		sim.counts()
		if sim.has_method("counts")
		else Vector3i(sim.player_shots().size() / 3, sim.enemy_shots().size() / 3, sim.explosions().size() / 3)
	)
	if counts.x > _last_counts.x:
		Sfx.play(Sfx.WEAPON_1)
	if counts.z > _last_counts.z:
		Sfx.play(Sfx.EXPLOSION_9)
	_last_counts = counts


# --- Ship and dashboard ---------------------------------------------


func _draw_player() -> void:
	if sim == null or ship_sheet == null:
		return
	var p: PackedInt32Array = sim.player(0)
	if p.size() < 8 or p[4] == 0:
		return
	# player_movement.c: image = banking*2 + shipGr, blit_sprite2x2 at (x - 5, y - 7).
	var bank := clampi(p[6], -2, 2) * 2
	var index := ship_graphic + bank
	var modulate := Color(1, 1, 1, 0.6) if p[5] > 0 and (Engine.get_physics_frames() / 2) % 2 == 0 else Color.WHITE
	if _mod_ship_texture != null:
		var t := Mods.texture("ships", _mod_ship_graphic + bank)
		_player_node.draw_texture(
			t if t != null else _mod_ship_texture, Vector2(p[0] - 5 + SIM_X_TO_SCREEN, p[1] - 7), modulate
		)
		return
	ship_sheet.draw_2x2(_player_node, index, p[0] - 5 + SIM_X_TO_SCREEN, p[1] - 7, modulate)


func _draw_hud() -> void:
	if _hud_vga != null:
		# Solo HUD and Endless HUD drawn by HudRenderer (pixel-perfect VGA).
		return
	var h: Dictionary = sim.mode_hud() if sim != null else {}
	var split: bool = h.get("two_player", false) and not h.get("galaga", false)
	# tyrian2.c:JE_main: JE_loadPic(twoPlayerMode ? 6 : 3).
	var tex := TyrianAssets.pic(HUD_PIC_TWO_PLAYERS if h.get("two_player", false) else HUD_PIC)
	if tex == null:
		return
	# Only the side bar (56 px) and the bottom band are opaque.
	_hud.draw_texture_rect_region(
		tex,
		Rect2(PLAY_AREA_SIZE.x, 0, 320 - PLAY_AREA_SIZE.x, 200),
		Rect2(PLAY_AREA_SIZE.x, 0, 320 - PLAY_AREA_SIZE.x, 200)
	)
	_hud.draw_texture_rect_region(
		tex,
		Rect2(0, PLAY_AREA_SIZE.y, PLAY_AREA_SIZE.x, 200 - PLAY_AREA_SIZE.y),
		Rect2(0, PLAY_AREA_SIZE.y, PLAY_AREA_SIZE.x, 200 - PLAY_AREA_SIZE.y)
	)
	if sim == null:
		return
	var p: PackedInt32Array = sim.player(0)
	if p.size() < 8:
		return
	# varz.c:JE_drawShield/JE_drawArmor -> nortvars.c:JE_dBar3: one 2 px notch per point.
	if split:
		# Two players (outside Galaga): bars at y = 60 + 134 i, value
		# roundf(v * 0.8f); v * 8 is even, never an exact half -> (8 v + 5) / 10.
		for i in 2:
			var q: PackedInt32Array = sim.player(i)
			_draw_bar(270, 60 + 134 * i, (q[3] * 8 + 5) / 10, Color8(64, 96, 252))
			_draw_bar(307, 60 + 134 * i, (mini(q[2], 28) * 8 + 5) / 10, Color8(232, 168, 32))
		# Endless two players: endless_draw_hud overlays.
		if EndlessSession.active():
			_draw_endless_hud()
		return
	_draw_bar(270, 194, p[3], Color8(64, 96, 252))
	_draw_bar(307, 194, mini(p[2], 28), Color8(232, 168, 32))
	if EndlessSession.active():
		_draw_endless_hud()


## Start of the wave intro banner in real-clock ms (0: not yet
## started). endless.c:endless_on_level_loaded: s_intro_until_ticks =
## SDL_GetTicks() + ENDLESS_INTRO_MS, taken when the level loop starts
## (tyrian2.c:740, after the fade-in); slow motion and pause do not
## lengthen it (endless.c:1300 compares with SDL_GetTicks).
var _endless_intro_start_ms := 0


## Real milliseconds elapsed since the banner started; 0 until the
## first frame of the level has run.
func _endless_intro_elapsed_ms() -> int:
	if _endless_intro_start_ms == 0:
		return 0
	return Time.get_ticks_msec() - _endless_intro_start_ms


## fonthand.c:JE_textShade(FULL_SHADE): four black copies offset by one
## pixel (left, right, top, bottom) then the text, `font` font of
## TyrianText.SHEETS (2 = TINY_FONT, 1 = SMALL_FONT_SHAPES).
func _text_shade(x: int, y: int, text: String, font: int, color: Color) -> void:
	var sheet := TyrianAssets.sheet(TyrianText.SHEETS[font])
	if sheet == null:
		return
	for d in [Vector2(-1, 0), Vector2(1, 0), Vector2(0, -1), Vector2(0, 1), Vector2.ZERO]:
		var off: Vector2 = d
		var cx: float = float(x) + off.x
		for ch in text:
			var code := ch.unicode_at(0)
			if code == 32:
				cx += 6
				continue
			var id := TyrianText.sprite_id(code)
			if id < 0 or not sheet.has(id):
				continue
			sheet.draw(_hud, id, cx, y + off.y, Color.BLACK if off != Vector2.ZERO else color)
			cx += sheet.width(id) + 1


## combos.c:combo_tiers (locale key, multiplier), same order as the Sim.
const COMBO_TIERS := [
	["combo_impossible", 10],
	["combo_mythical", 5],
	["combo_legendary", 4],
	["combo_godlike", 4],
	["combo_unstoppable", 3],
	["combo_frenzy", 3],
	["combo_rampage", 2],
	["combo_combo", 2]
]
var _combo_banner := 0
var _combo_banner_until := 0


## endless.c:endless_draw_hud + mainint.c:JE_inGameDisplays in Endless.
func _draw_endless_hud() -> void:
	# endless.c:endless_draw_hud: "if (g_anomaly_eyes_closed) return" (ENH-15).
	if EndlessSession.eyes_closed():
		return
	var st: PackedInt64Array = sim.endless_state() if sim.has_method("endless_state") else PackedInt64Array()
	# "WAVE n": TINY_FONT, JE_textShade(..., 15, 4, FULL_SHADE), right-aligned to
	# the play area (PLAY_AREA_WIDTH - 6 - width), y = 3 in single player.
	var wave := EndlessSession.hud_text()
	# endless.c:endless_draw_hud: "wave_y = twoPlayerMode ? 30 : 3".
	var wave_y := 30 if EndlessSession.two_player else 3
	_text_shade(PLAY_AREA_SIZE.x - 6 - TyrianText.measure(wave, 2), wave_y, wave, 2, Color8(252, 252, 252))
	# Combo multiplier "x%d" centered, y = 6, JE_textShade(.., 9, 6) if > 1.
	var combo: PackedInt32Array = sim.combo_state() if sim.has_method("combo_state") else PackedInt32Array()
	if combo.size() >= 3:
		if combo[2] >= 0:
			_combo_banner = combo[2]
			_combo_banner_until = Time.get_ticks_msec() + 2000
		if combo[1] > 1:
			var m := "x%d" % combo[1]
			_text_shade(320 / 2 - TyrianText.measure(m, 2) / 2, 6, m, 2, Color8(160, 200, 252))
	# combos.c:combo_on_kill: "NAME xM" at the tier (message_bar_queue).
	if Time.get_ticks_msec() < _combo_banner_until:
		var tier: Array = COMBO_TIERS[_combo_banner]
		var msg := "%s x%d" % [TyrianText.L(tier[0]), tier[1]]
		_text_shade(160 - TyrianText.measure(msg, 2) / 2, 165, msg, 2, Color8(252, 252, 160))
	# mainint.c:JE_inGameDisplays: in Endless the counter shows endless_score()
	# (shared score, only once) at (30, 175), JE_textShade(.., 2, 4, FULL_SHADE).
	var score := EndlessSession.score() + (st[2] if st.size() >= 3 else 0)
	_text_shade(30, 175, str(score), 2, Color8(160, 224, 160))
	# Lives: icon 285 of spriteSheet9 (main_08_player_ships) per reserve life (endless_lives_of - 1),
	# x = 30 then +12, y = 35 with a special weapon (15 otherwise); beyond 4,
	# one icon and the number at (45, y + 3).
	# Two players: player 2 at x = 270 towards the left (-12), number at 250, y = 15.
	var icons := TyrianAssets.sheet("main_08_player_ships")  # spriteSheet9
	for p in 2 if EndlessSession.two_player else 1:
		var lives := int(st[p]) if st.size() > p else EndlessSession.lives()
		var extra := maxi(lives - 1, 0)
		var y := 35 if p == 0 and Game.loadout.size() > 9 and Game.loadout[9] > 0 else 15
		var x0 := 30 if p == 0 else 270
		var step := 12 if p == 0 else -12
		if icons != null:
			if extra >= 5:
				icons.draw(_hud, 285, x0, y)
				_text_shade(45 if p == 0 else 250, y + 3, str(extra), 2, Color8(252, 252, 252))
			else:
				for i in extra:
					icons.draw(_hud, 285, x0 + step * i, y)
	# Intro banner (2 s): "WAVE n" then "ENTERING name", SMALL_FONT_SHAPES
	# centered (JE_dString), at h/2 - 18 and h/2 + 6.
	var lines := HudRenderer.wave_intro_lines(_endless_intro_elapsed_ms())
	if not lines[0].is_empty():
		for pair: Array in [[lines[0], 200 / 2 - 18], [lines[1], 200 / 2 + 6]]:
			var t: String = pair[0]
			_text_shade(160 - TyrianText.measure(t, 1) / 2, int(pair[1]), t, 1, Color8(252, 252, 252))


func _draw_bar(x: int, y: int, value: int, color: Color) -> void:
	for z in range(0, maxi(0, value) + 1):
		var top := y - 1 - z * 2
		if top < 1:
			break
		_hud.draw_rect(Rect2(x, top, 9, 2), color.darkened(0.3 * float(z % 4) / 4.0))


# --- Achievements (achievements.c) -------------------------------------------------

## Achievement names in the order of the ACH_* enum (achievements_screen.gd).
const ACHIEVEMENT_LIST := preload("res://scripts/achievements_screen.gd").ACHIEVEMENTS
## achievements.c: ACH_NOTIFY_DURATION_MS, ACH_NOTIFY_QUEUE_MAX.
const ACH_NOTIFY_MS := 3000
const ACH_NOTIFY_QUEUE_MAX := 8
var _ach_active := false
var _ach_level_done := false
## Banner displayed: achievement index (-1 = none), start (game clock), queue.
var _ach_notify_id := -1
var _ach_notify_start := 0
var _ach_queue: Array[int] = []


## Player's save; tools (capture, replay): achievements all locked, nothing written.
func _ach_dat_path() -> String:
	if Capture.active or Bench.replay_assist or CampaignBot.replay_assist:
		return ""
	return UserData.path(CProfile.FILE_NAME)


func _ach_start() -> void:
	_ach_active = sim != null and sim.has_method("ach_level_start")
	if _ach_active:
		# Playback of a .rep (ach_spectator_mode): no unlock.
		# achievements.c: no unlock during the demo (play_demo).
		sim.ach_level_start(_ach_dat_path(), replaying or demo)
		# endless.c:886 / 1822: stats_run_start at the start (or resume) of the Endless run.
		if EndlessSession.run_start_pending and EndlessSession.active():
			EndlessSession.run_start_pending = false
			if not (replaying or demo):
				sim.ach_run_start()
		if Game.game_start_uncounted and not (replaying or demo):
			Game.game_start_uncounted = false
			sim.ach_count_game_start()
		# achievements_check: Night Player is judged on the local time (0:00 – 4:59).
		if sim.has_method("ach_set_local_hour"):
			sim.ach_set_local_hour(Game.local_hour())
		_ach_set_context()
		# Achievements unlocked between two levels: banners pending.
		var pending: Array[int] = Game.ach_pending.duplicate()
		Game.ach_pending.clear()
		for id in pending:
			_ach_enqueue(id)


## Globals read by achievements_check: language (Polyglot), Endless wave,
## active mods (Modded Run).
func _ach_set_context() -> void:
	var wave := -1
	if EndlessSession.active():
		wave = int(EndlessSession.run.wave())
	sim.ach_set_context(Game.language_index(), wave, Game.enabled_mod_count())


func _ach_tick() -> void:
	if not _ach_active:
		return
	for id in sim.ach_after_tick():
		_ach_unlocked(id)
	_ach_update_banner()
	if sim.level_ended() and not _ach_level_done:
		_ach_level_done = true
		if _level_completed():
			# mainint.c:JE_endLevelAni emits EVENT_LEVEL_COMPLETE after the level
			# loop: the banner of these achievements (Survivor...) is only drawn by
			# the loop (enemy_render.c:201), never on the frozen debriefing image.
			for id in sim.ach_level_complete():
				_ach_unlocked(id)
			# Lifetime counters (levels, score, time...) written at every completed level.
			if not _ach_dat_path().is_empty():
				TyrianVga.ach_save_current(_ach_dat_path())


## tyrian2.c:JE_main: EVENT_LEVEL_COMPLETE ("Levels Completed" statistic
## and end-of-level achievements) is only emitted by JE_endLevelAni, called if
## "(!all_players_dead() || normalBonusLevelCurrent || bonusLevelCurrent)".
## A game over ends the level without counting it as successful.
func _level_completed() -> bool:
	var p1: PackedInt32Array = sim.player(0)
	var alive := p1.size() > 4 and p1[4] != 0
	if not alive and _two_players():
		var p2: PackedInt32Array = sim.player(1)
		alive = p2.size() > 4 and p2[4] != 0
	return alive or (Game.campaign_active and Game.bonus_level_current)


## Banner queue: displayed right away if nothing is on screen.
func _ach_enqueue(id: int) -> void:
	if _ach_notify_id >= 0:
		if _ach_queue.size() < ACH_NOTIFY_QUEUE_MAX:
			_ach_queue.append(id)
	else:
		_ach_notify_id = id
		_ach_notify_start = sim.ach_clock_ms()


## achievements.c:achievements_unlock: banner (or queue) and save. No sound
## (divergence 63): the C plays S_SOUL_OF_ZINGLON, a weapon sweep, on channel 0,
## where the front gun's next shot cuts it, so it was heard as a truncated weapon.
func _ach_unlocked(id: int) -> void:
	_ach_enqueue(id)
	var path := _ach_dat_path()
	if not path.is_empty():
		TyrianVga.ach_save_current(path)


## Banner text set at the last `_ach_update_banner` (recorded for tests).
var _banner_text := ""


func _hud_banner_text() -> String:
	return _banner_text


## achievements.c:achievements_draw_notification: 3 s per banner, the
## next one in the queue takes its place (nothing is drawn on that pass).
func _ach_update_banner() -> void:
	var text := ""
	if _ach_notify_id >= 0:
		if sim.ach_clock_ms() - _ach_notify_start > ACH_NOTIFY_MS:
			if _ach_queue.is_empty():
				_ach_notify_id = -1
			else:
				_ach_notify_id = _ach_queue.pop_front()
				_ach_notify_start = sim.ach_clock_ms()
		else:
			var a: Array = ACHIEVEMENT_LIST[_ach_notify_id]
			var name := TyrianText.L(a[0])
			text = TyrianText.L("ach_notify").replace("%s", name)
	# Indexed background active (Original mode): the banner is composed there before the
	# RGBA layer (shot halos on top, like JE_starShowVGA); otherwise
	# the HUD draws it above the play area.
	_banner_text = text
	var has_banner := ClassDB.class_has_method("TyrianSim", "set_playfield_banner")
	var in_playfield := _playfield_bg != null and has_banner
	if has_banner:
		ClassDB.class_call_static("TyrianSim", "set_playfield_banner", text if in_playfield else "")
	if _hud_vga != null:
		_hud_vga.notification = "" if in_playfield else text
		_hud_vga.banner_shown = not text.is_empty()


# --- Music (ADR 0010) -------------------------------------------------------

## levelSong (]L[ of the episode file, 1-based) of the level played.
var _level_song := 0


## tyrian2.c:JE_main, before the loop: fade_and_play_song(levelSong - 1),
## or the song selected in the Jukebox (ingame_song_override).
func _music_start() -> void:
	# Demo: levelSong from the demo.N header (load_next_demo).
	_level_song = _demo_song if demo else Music.level_song(Game.episode, Game.lvl_file_num)
	if sim != null:
		sim.set_music_volume(Music.user_volume)
	Music.set_volume(Music.user_volume)
	# tyrian2.c:394: fade_song() before loading the level.
	Music.fade_song()
	if Music.ingame_song_override >= 0:
		Music.fade_and_play_song(Music.ingame_song_override)
	elif _level_song > 0:
		Music.fade_and_play_song(_level_song - 1)


## After each tick: music orders from the simulation (events 34/35,
## death fade, game over, level end), then the branch of the MUSIC
## FADE block that depends on the player: "else if (!playing && firstGameOver)
## play_song(levelSong - 1)" (tyrian2.c). No fade at the end of the level:
## the C only does fade_song() after the summary (tyrian2.c:320, debrief.gd), on
## leaving the Endless end screens (tyrian2.c:201, endless_end.gd) or
## in the death/abandon branch (_fade_out_before_leaving).
func _music_tick() -> void:
	Music.apply_cues(sim.take_music_cues())
	if sim.music_level_song_branch() and _level_song > 0 and not Music.is_playing():
		Music.play_song(_level_song - 1)
