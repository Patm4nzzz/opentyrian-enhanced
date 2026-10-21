## Setup menu (setup_menu.c:setupMenu) and its submenus, rendered by the
## shared menu engine (MenuEngine, MENU_TREE style = menu_tree.c:run_menu,
## ADR 0121), frame by frame like the C version (ADR 0020).
##
## The background (image 2 of tyrian.pic, bottom band cleared) is set in
## VGAScreen2 then copied to VGAScreen at each loop iteration
## (vgascreen2_pre_render); the header, rows and help are redrawn
## on top before each `present()` (JE_showVGA).
##
## Nested menus (menu_tree_run called from an action) form a
## stack: each level keeps its selection, like the local variable `sel` of
## run_menu.
##
## Only the options that act on the rewrite are shown; each one
## is applied right away and saved in userdata/setup.cfg (per-option decision
## list: docs/setup-options.md). Key Bindings opens the Controls
## screen (controls_options.gd), the only one that actually changes the keys;
## Joystick, the C gamepad settings (joystick_menu.gd).
extends TyrianScreen

## Row kinds of the shared engine (menu_tree.h: MenuItemKind).
const Kind := MenuEngine.Kind

const SCENE_MODS := "res://scenes/mods.tscn"
const SCENE_JUKEBOX := "res://scenes/jukebox.tscn"

## locale.c:language_names / language_codes.
const LANGUAGE_NAMES := ["English", "Francais", "Espanol", "Deutsch", "Portugues", "Italiano"]
const LANGUAGE_CODES := ["en", "fr", "es", "de", "pt", "it"]

## Menu settings, C default values (bullet_time.c, combos.c,
## locale.c:current_language, config.c:JE_loadConfiguration 191/255,
## loudness.c, nortsong.c, graphics_menu.c: window = 0). Static: they
## survive scene changes like the C globals. All of them are
## saved in SETTINGS_PATH and applied by apply_settings.
static var settings := {
	"bullet_time_enabled": true,
	"combo_enabled": true,
	"auto_fire_enabled": false,
	"current_language": 0,
	"music_volume": 191,
	"fx_volume": 191,
	# config.c:mouseSettings (game_menu.c, Options > Mouse): function of the left,
	# right, middle buttons (1 fire, 2 left sidekick, 3 right, 4 both, 5 rear fire).
	"mouse_buttons": [1, 4, 5],
	"music_disabled": false,
	"samples_disabled": false,
	# Sound mute key (divergence no. 57): separate flag, it touches neither
	# the volumes nor music_disabled / samples_disabled.
	"sound_muted": false,
	"sfx_pitch_variation": true,
	"music_engine": 2,
	"display": 0,
	# Size and position of the window in windowed mode (divergence no. 58):
	# [x, y, width, height], empty until the player has changed it.
	"window_rect": [],
	# Pause menu (mainint.c:JE_inGameSetup): Visual FX preset
	# (visual_fx.c, 0 Off, 1 Basic, 2 Extended = C default) and game
	# speed (config.c:gameSpeed 1..5, 4 = Normal; save_json.c "game_speed").
	"visual_fx": 2,
	"game_speed": 4,
	"stereo_enabled": true,
	"sfx_impact_scaling": true,
	"music_pseudo_stereo": false,
	# hitstop.c, damagenumbers.c, rumble.c, particles.c, mutators.c (defaults
	# of save_json.c: true, true, true, true, Retro+; mutators off).
	"hitstop_enabled": true,
	"damage_numbers_enabled": true,
	"colored_damage_numbers": true,
	"rumble_enabled": true,
	"explosion_style": 1,
	"mutators": 0,
	# visual_fx.c:preset_table: particles and shakes follow the Visual FX
	# preset (Off: false, Basic / Extended: true); saved so that a
	# "Custom" state imported from the C (save_json.c "screen_shake") survives.
	"particles_enabled": true,
	"screenshake_enabled": true,
}
static var SETTINGS_PATH: String = UserData.path("setup.cfg")
const DEFAULTS := {
	"bullet_time_enabled": true, "combo_enabled": true, "auto_fire_enabled": false,
	"current_language": 0, "music_volume": 191, "fx_volume": 191, "mouse_buttons": [1, 4, 5], "music_disabled": false,
	"samples_disabled": false, "sound_muted": false, "sfx_pitch_variation": true, "music_engine": 2, "display": 0,
	"window_rect": [],
	"visual_fx": 2, "game_speed": 4,
	# loudness.c (stereo_enabled, music_pseudo_stereo), nortsong.c (sfx_impact_scaling).
	"stereo_enabled": true, "sfx_impact_scaling": true, "music_pseudo_stereo": false,
	"hitstop_enabled": true, "damage_numbers_enabled": true, "colored_damage_numbers": true,
	"rumble_enabled": true, "explosion_style": 1, "mutators": 0,
	"particles_enabled": true, "screenshake_enabled": true,
}
## Numbering of `music_engine` in the file ([format] section): absent,
## it is the old one (0 Original OPL, 1 MIDI), migrated to the C one
## (sound_menu.c:eng_get, 0 Classic, 1 Authentic, 2 Enhanced).
const MUSIC_ENGINE_FORMAT := 2
## Numbering of `display` ([format] section `display_default`): at 2, the
## default is fullscreen (the maintainer's decision of 2026-10-10, divergence
## no. 56) and the player's choice, windowed included, is always respected.
## Absent (profile from an older version), `display = 0` was only the
## default never saved: migrated once to fullscreen.
const DISPLAY_DEFAULT_FORMAT := 2
## Last state applied (apply_settings only redoes what changed).
static var _applied := {}
## True after the first application of "visual_fx" (see apply_settings).
static var _visual_fx_seen := false


## Reading of SETTINGS_PATH at launch (Game._ready): only the keys
## present replace the defaults, then everything is applied.
static func load_settings() -> void:
	var cfg := ConfigFile.new()
	var saved := cfg.load(SETTINGS_PATH) == OK
	for key in settings:
		if saved and cfg.has_section_key("setup", key):
			settings[key] = cfg.get_value("setup", key)
	var resolved := resolve_display(cfg, saved, DisplayServer.get_primary_screen())
	settings["display"] = resolved.display
	var forced := -1 if _is_tool_run() else _forced_display()
	if _is_tool_run() or forced >= 0:
		_applied["display"] = settings["display"]  # Setup shows the saved setting
	if saved and cfg.has_section_key("setup", "music_engine") \
			and int(cfg.get_value("format", "music_engine", 1)) < MUSIC_ENGINE_FORMAT:
		settings["music_engine"] = migrate_music_engine(int(settings["music_engine"]))
	if _lang_from_command_line() or not (saved and cfg.has_section_key("setup", "current_language")):
		# Language never chosen or imposed by --lang: the one already in place
		# stays, and the Setup selector shows it.
		settings["current_language"] = maxi(LANGUAGE_CODES.find(TyrianText.language), 0)
		_applied["current_language"] = settings["current_language"]
	if resolved.migrated and not _is_tool_run():
		save_settings()  # writes the marker: the migration only happens once
	# The window gets its place back before any switch to fullscreen: this is the
	# rectangle that the return to windowed mode will restore.
	Effects.apply_saved_window()
	apply_settings()
	if forced >= 0:
		_apply_display(forced)


## Saved language only, read before the main scene is built
## (Game._ready): the title builds its labels in its
## _ready, before load_settings (deferred after the Music / Sfx autoloads).
static func load_language() -> void:
	if _lang_from_command_line():
		return
	var cfg := ConfigFile.new()
	if cfg.load(SETTINGS_PATH) == OK and cfg.has_section_key("setup", "current_language"):
		var i := clampi(int(cfg.get_value("setup", "current_language")), 0, LANGUAGE_CODES.size() - 1)
		TyrianText.language = LANGUAGE_CODES[i]


## --lang=xx (captures) takes precedence over the saved language (Game._ready sets it).
static func _lang_from_command_line() -> bool:
	return Array(OS.get_cmdline_user_args()).any(func(a: String) -> bool: return a.begins_with("--lang="))


## Smallest saved window accepted: the game at 320x200.
const WINDOW_MIN_SIZE := Vector2i(320, 200)
## Height below the top of the window where the mouse can still grab it.
const WINDOW_GRAB_Y := 12


## Saved window rectangle; zero size if there is none or if it is unreadable.
static func saved_window_rect() -> Rect2i:
	var v: Variant = settings.get("window_rect", [])
	if not (v is Array) or (v as Array).size() != 4:
		return Rect2i()
	for n in v:
		if not (n is int or n is float):
			return Rect2i()
	return Rect2i(int(v[0]), int(v[1]), int(v[2]), int(v[3]))


## True if window `rect` is usable: big enough, and its top edge (the
## title bar, by which it is moved) falls on one of the connected screens
## (`screens`, rectangles in pixels). Otherwise the screen was unplugged or its
## resolution changed, and the window would be out of reach.
static func window_rect_on_screen(rect: Rect2i, screens: Array) -> bool:
	if rect.size.x < WINDOW_MIN_SIZE.x or rect.size.y < WINDOW_MIN_SIZE.y:
		return false
	var grab := Vector2i(rect.position.x + rect.size.x / 2, rect.position.y + WINDOW_GRAB_Y)
	for s in screens:
		if (s as Rect2i).has_point(grab):
			return true
	return false


static func connected_screens() -> Array:
	var out := []
	for i in DisplayServer.get_screen_count():
		out.append(Rect2i(DisplayServer.screen_get_position(i), DisplayServer.screen_get_size(i)))
	return out


## Saves the window (in windowed mode): sampled at each change and on exit.
static func remember_window(rect: Rect2i) -> void:
	if _is_tool_run():
		return
	settings["window_rect"] = [rect.position.x, rect.position.y, rect.size.x, rect.size.y]
	save_settings()


## Starting screen (graphics_menu.c:disp_get; the C's save_json.c "video.fullscreen"
## which starts windowed, here fullscreen by default): returns
## {display, migrated}. 0 = windowed, n = fullscreen on screen n.
##  - no saved setting: fullscreen on the main screen;
##  - `display = 0` without [format] display_default marker: default never
##    chosen from an older version, fullscreen only once (migrated);
##  - otherwise, the player's value.
static func resolve_display(cfg: ConfigFile, saved: bool, primary_screen: int) -> Dictionary:
	var fullscreen := primary_screen + 1
	if not saved or not cfg.has_section_key("setup", "display"):
		return {"display": fullscreen, "migrated": false}
	var display := int(cfg.get_value("setup", "display"))
	if display == 0 and int(cfg.get_value("format", "display_default", 0)) < DISPLAY_DEFAULT_FORMAT:
		return {"display": fullscreen, "migrated": true}
	return {"display": display, "migrated": false}


## Alt+Enter (keyboard.c:toggle_fullscreen changes fullscreen_display, which the C
## saves on exit): the mode obtained is retained and saved right away.
static func remember_display(display: int) -> void:
	if _is_tool_run():
		return
	settings["display"] = display
	_applied["display"] = display
	save_settings()


## Tools (capture, replay, benchmarks): they write no setting and keep the window.
static func _is_tool_run() -> bool:
	return Capture.active or Bench.replay_assist or CampaignBot.replay_assist


## Mode imposed for this launch, without touching the saved setting:
## "-- --windowed" or "-- --fullscreen" (game options, after "--"), or
## the Godot engine's --fullscreen option, already applied to the window at the time
## of reading (Godot removes its own options from the visible command
## line: its --windowed, identical to the engine default, is indistinguishable,
## hence the game's option). Returns -1 (nothing imposed), 0 (windowed) or n (fullscreen).
static func _forced_display() -> int:
	var user_args := OS.get_cmdline_user_args()
	if user_args.has("--windowed"):
		return 0
	if user_args.has("--fullscreen"):
		return DisplayServer.get_primary_screen() + 1
	if DisplayServer.get_name() != "headless" and Effects.is_fullscreen():
		return DisplayServer.window_get_current_screen() + 1
	return -1


static func save_settings() -> void:
	var cfg := ConfigFile.new()
	for key in settings:
		cfg.set_value("setup", key, settings[key])
	cfg.set_value("format", "music_engine", MUSIC_ENGINE_FORMAT)
	cfg.set_value("format", "display_default", DISPLAY_DEFAULT_FORMAT)
	var err := cfg.save(SETTINGS_PATH)
	if err != OK:
		push_warning("setup.cfg not written (%s)" % error_string(err))


## Old numbering (0 Original OPL, 1 MIDI) -> the C one.
static func migrate_music_engine(old: int) -> int:
	return Music.ENGINE_ENHANCED if old == 1 else Music.ENGINE_CLASSIC


## C default settings (tests, missing file).
static func reset_settings() -> void:
	settings = DEFAULTS.duplicate()
	_applied = {}
	_visual_fx_seen = false


## Sound effects active: neither muted in Setup, nor by the mute key.
static func _apply_sfx_enabled() -> void:
	Sfx.enabled = not (settings.samples_disabled or settings.sound_muted)
	if not Sfx.enabled:
		Sfx.stop_all()


## Sets each setting modified since the last call into the game.
static func apply_settings() -> void:
	for key in settings:
		if _applied.get(key) == settings[key]:
			continue
		_applied[key] = settings[key]
		match key:
			"bullet_time_enabled":
				Effects.bullet_time_allowed = settings[key]
			"combo_enabled":
				Game.combos_enabled = settings[key]
			"auto_fire_enabled":
				Game.auto_fire = settings[key]
			"current_language":
				TyrianText.language = LANGUAGE_CODES[clampi(settings[key], 0, LANGUAGE_CODES.size() - 1)]
			"music_volume", "music_disabled", "sound_muted":
				Music.user_volume = settings.music_volume
				Music.muted = settings.music_disabled or settings.sound_muted
				Music.set_volume(settings.music_volume)
				if key == "sound_muted":
					_apply_sfx_enabled()
			"fx_volume":
				Sfx.volume = settings[key]
			"samples_disabled":
				_apply_sfx_enabled()
			"sfx_pitch_variation":
				Sfx.pitch_variation = settings[key]
			"music_engine":
				Music.set_engine(settings[key])
			"display":
				_apply_display(settings[key])
			"visual_fx":
				# visual_fx.c:visual_fx_apply_preset then JE_initProcessorType:
				# the preset writes processorType, from which wild /
				# background2 / filtrationAvail and the background speed derive. The
				# simulation reads it at level load (level.gd).
				Game.processor_type = VISUAL_FX_PROCESSOR_TYPE[
					clampi(int(settings[key]), 0, VISUAL_FX_PROCESSOR_TYPE.size() - 1)]
				# visual_fx_apply_preset also writes particles_enabled and
				# screenshake_enabled (not at load: the saved state,
				# possibly Custom, is the player's).
				if _visual_fx_seen:
					settings.particles_enabled = int(settings[key]) >= 1
					settings.screenshake_enabled = int(settings[key]) >= 1
					_applied.erase("screenshake_enabled")
				_visual_fx_seen = true
				_apply_effects()
				if ClassDB.class_has_method("TyrianSim", "set_advanced_lighting"):
					ClassDB.class_call_static("TyrianSim", "set_advanced_lighting", int(settings[key]) == 2)
			"game_speed":
				pass  # applied at the start of each level (GameSpeed.apply)
			"stereo_enabled":
				Sfx.stereo = settings[key]
				Music.stereo = settings[key]
			"sfx_impact_scaling":
				Sfx.impact_scaling = settings[key]
			"music_pseudo_stereo":
				Music.widen = settings[key]
			"hitstop_enabled", "damage_numbers_enabled", "colored_damage_numbers", "rumble_enabled", "explosion_style", \
					"particles_enabled", "screenshake_enabled":
				_apply_effects()
			"mutators":
				Game.mutators = int(settings[key])


## Setup effects applied right away to the current level (C globals,
## read every frame); level.gd also sets them at each load.
static func _apply_effects() -> void:
	var tree := Engine.get_main_loop() as SceneTree
	if tree == null:
		return
	for level in tree.get_nodes_in_group(&"tyrian_level"):
		level.apply_effects()


## Player effects for a TyrianSim: hitstop.c (Freeze Frames),
## particles.c (Explosions, particles_enabled: true by default),
## damagenumbers.c (0 off, 1 on, 2 colored), rumble.c.
static func apply_effects_to(sim: Object) -> void:
	if sim == null or not sim.has_method("set_effects"):
		return
	var dn := 0 if not settings.damage_numbers_enabled else (2 if settings.colored_damage_numbers else 1)
	sim.set_effects(settings.hitstop_enabled, int(settings.explosion_style), dn, settings.rumble_enabled,
			bool(settings.get("particles_enabled", true)))
	# visual_fx.c: screenshake_enabled and processorType of the preset.
	if sim.has_method("set_screen_fx"):
		var preset := clampi(int(settings.visual_fx), 0, VISUAL_FX_PROCESSOR_TYPE.size() - 1)
		sim.set_screen_fx(bool(settings.get("screenshake_enabled", true)), VISUAL_FX_PROCESSOR_TYPE[preset])


## input_map.c:input_action_toggle_damage_numbers (F7 in level, mainint.c).
static func toggle_damage_numbers() -> void:
	settings.damage_numbers_enabled = not settings.damage_numbers_enabled
	apply_settings()
	save_settings()


## graphics_menu.c:disp_set: 0 = windowed, n = fullscreen on screen n.
static func _apply_display(display: int) -> void:
	if DisplayServer.get_name() == "headless":
		return
	if display <= 0:
		Effects.set_fullscreen(false)
		return
	DisplayServer.window_set_current_screen(clampi(display - 1, 0, DisplayServer.get_screen_count() - 1))
	Effects.set_fullscreen(true)


## Stack of open menus and navigation (shared engine).
var menus := MenuEngine.new(self)
## True when the menu is waiting for a key (for the capture tool).
var idle := false
var _busy := true
## Row selected on opening (return from a screen opened from
## Setup: run_menu keeps its selection during the action); reset to 0 afterwards.
static var entry_row := 0
const KEYBINDINGS_ROW := 3


func _ready() -> void:
	_run.call_deferred()


func _run() -> void:
	await restore_setup_background()
	_busy = false
	var row := entry_row
	entry_row = 0
	_open("setup_header", _setup_items(), row)


## Entry into run_menu (menu_tree_run); exit from the root: title.
func _open(header: String, items: Array, sel := 0) -> void:
	menus.open(header, items, {"on_back": _leave, "sel": sel})


## setup_menu.c:restore_setup_background.
func restore_setup_background() -> void:
	vga.load_pic(VGA_SCREEN2, 2, false)
	vga.fill_rectangle_xy(VGA_SCREEN2, 0, 192, 319, 199, 0)  # fill_rectangle_wh(0, 192, 320, 8)
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	await fade_palette(10, 0, 255)


# --- Menu tables (same rows, same order as the C code) -------------------------

func _item(label: String, help: String, kind: int, extra := {}) -> Dictionary:
	return MenuEngine.item(label, help, kind, extra)


## Toggle on a variable of `settings` (TOGGLE / *_ACCESSORS macros).
func _toggle(label: String, help: String, var_name: String) -> Dictionary:
	return _item(label, help, Kind.TOGGLE, {
		"get": func() -> bool: return settings[var_name],
		"set": func(v: bool) -> void: settings[var_name] = v,
	})


## setup_menu.c: Mods entry -> mod manager (mod_manager.c).
func _open_mods() -> void:
	idle = false
	_busy = true
	Game.goto_scene(SCENE_MODS)


## setup_menu.c: Jukebox entry -> jukebox.c (jukebox.gd scene, which comes back here).
func _open_jukebox() -> void:
	idle = false
	_busy = true
	Game.goto_scene(SCENE_JUKEBOX)


## setup_menu.c:setup_items.
func _setup_items() -> Array:
	return [
		_item("setup_graphics", "setup_graphics_desc", Kind.ACTION,
			{"action": func() -> void: _open("gfx_header", _graphics_items())}),
		_item("setup_sound", "setup_sound_desc", Kind.ACTION,
			{"action": func() -> void: _open("snd_header", _sound_items())}),
		_item("setup_gameplay", "setup_gameplay_desc", Kind.ACTION,
			{"action": func() -> void: _open("gp_header", _gameplay_items())}),
		_item("setup_keybindings", "setup_keybindings_desc", Kind.ACTION, {"action": _open_controls}),
		_item("setup_joystick", "setup_joystick_desc", Kind.ACTION, {"action": _open_joystick}),
		_item("setup_mods", "setup_mods_desc", Kind.ACTION, {"action": _open_mods}),
		_item("setup_jukebox", "setup_jukebox_desc", Kind.ACTION, {"action": _open_jukebox}),
		_item("setup_done", "setup_done_main", Kind.BACK),
	]


## Key Bindings: Controls screen (keys and gamepad buttons of both
## players, F1 help and capture cancel); Esc returns to Setup from there.
func _open_controls() -> void:
	idle = false
	_busy = true
	ControlsOptions.return_scene = Game.SCENE_SETUP
	# run_menu keeps its selection during the action: back on "Key Bindings".
	entry_row = KEYBINDINGS_ROW
	Game.goto_scene(ControlsOptions.SCENE_PATH)


## setup_menu.c: Joystick entry -> joystick_menu.c:joystick_menu_run,
## Setup submenu (capture prompt drawn on top).
var joystick := JoystickMenu.new(self)


## Gamepad assignment capture (joystick_menu.c:110-116): prompt
## presented without JE_mouseStart, no pointer.
func pointer_shown() -> bool:
	return super() and not joystick.capturing()


func _open_joystick() -> void:
	menus.open("joym_header", joystick.items(), {"decorate": joystick.decorate})


## Gamepad assignment capture (joystick_menu.c:assign_act): raw
## events go to the capture, not to navigation.
func handle_event(event: InputEvent) -> void:
	if joystick.capturing() and idle:
		if joystick.capture(event):
			menus.draw()
		get_viewport().set_input_as_handled()
		return
	super.handle_event(event)


## Labels of gameplay_menu.c:dmg_label (off / on / colored).
const DAMAGE_NUMBER_LABELS := ["off", "on", "colored"]
## Mutators, order of mutators_menu.c (bits of Game.mutators).
const MUTATOR_KEYS := ["mut_fast_enemies", "mut_armored", "mut_bullet_hell", "mut_glass_cannon",
	"mut_no_powerups", "mut_one_life", "mut_mirror", "mut_risk_reward"]


## gameplay_menu.c:gameplay_items, in the C order (Anomalies stays
## dormant, like in the C code).
func _gameplay_items() -> Array:
	return [
		_toggle("gp_bullet_time", "gp_bullet_time_desc", "bullet_time_enabled"),
		_toggle("gp_freeze_frames", "gp_freeze_frames_desc", "hitstop_enabled"),
		# gameplay_menu.c:dmg_get / dmg_set: two booleans behind a selector.
		_item("gp_damage_numbers", "gp_damage_numbers_desc", Kind.PICKER, {
			"get": func() -> int:
				if not settings.damage_numbers_enabled:
					return 0
				return 2 if settings.colored_damage_numbers else 1,
			"set": func(i: int) -> void:
				settings.damage_numbers_enabled = i > 0
				settings.colored_damage_numbers = i == 2,
			"count": DAMAGE_NUMBER_LABELS.size(),
			"labels": DAMAGE_NUMBER_LABELS,
		}),
		_toggle("gp_combo", "gp_combo_desc", "combo_enabled"),
		_toggle("gp_auto_fire", "gp_auto_fire_desc", "auto_fire_enabled"),
		_toggle("gp_rumble", "gp_rumble_desc", "rumble_enabled"),
		# lang_set reloads the language: the following strings are in the new one.
		_item("gp_language", "gp_language_desc", Kind.PICKER, {
			"get": func() -> int: return settings.current_language,
			"set": func(i: int) -> void:
				settings.current_language = i
				apply_settings()
				# apply_settings aligns _applied: handle_action would no longer see a
				# change, so the chosen language must be saved here.
				save_settings(),
			"count": LANGUAGE_NAMES.size(),
			"labels": LANGUAGE_NAMES,
		}),
		_item("gp_mutators", "gp_mutators_desc", Kind.ACTION,
			{"action": func() -> void: _open("mut_header", _mutators_items())}),
		_item("setup_done", "setup_done_back", Kind.BACK),
	]


## graphics_menu.c:visual_fx.c:preset_table — processorType of each preset
## (Off = Classic 1, Basic = High 3, Extended = Wild 4).
const VISUAL_FX_PROCESSOR_TYPE := [1, 3, 4]
## graphics_menu.c:visual_fx_preset_label — three cyclable notches (Custom
## remains displayable, never chosen).
const VISUAL_FX_LABELS := ["fx_preset_off", "fx_preset_basic", "fx_preset_extended"]
## graphics_menu.c:SpriteMode -> rewrite modes (ArtMode).
## Same setting as the Display screen (F10), a single source of truth:
## Original = original pixels, Remastered = HD (AI), Upscaled = xBRZ filter.
## (preload: a constant cannot read the autoload at load time.)
const DISPLAY_SETTINGS := preload("res://scripts/display_settings.gd")
const SPRITE_MODES := [DISPLAY_SETTINGS.ArtMode.ORIGINAL, DISPLAY_SETTINGS.ArtMode.HD, DISPLAY_SETTINGS.ArtMode.SHARP]
const SPRITE_KEYS := ["gfx_sprite_original", "gfx_sprite_remastered", "gfx_sprite_upscaled"]


## mutators_menu.c:mutators_items: eight toggles on Game.mutators (the
## C mut_* globals, read by every game, campaign included, and by
## the side modes screen) then Reset All (mutators_init).
func _mutators_items() -> Array:
	var items := []
	for i in MUTATOR_KEYS.size():
		items.append(_item(MUTATOR_KEYS[i], MUTATOR_KEYS[i] + "_desc", Kind.TOGGLE, {
			"get": func() -> bool: return Game.mutators & (1 << i) != 0,
			"set": func(v: bool) -> void: _set_mutators((Game.mutators | (1 << i)) if v else (Game.mutators & ~(1 << i))),
		}))
	items.append(_item("mut_reset_all", "mut_reset_all_desc", Kind.ACTION, {"action": func() -> void: _set_mutators(0)}))
	items.append(_item("setup_done", "setup_done_back", Kind.BACK))
	return items


## Game.mutators is also modified by the modes screen: set directly,
## then saved.
static func _set_mutators(mask: int) -> void:
	Game.mutators = mask
	settings.mutators = mask
	_applied["mutators"] = mask
	save_settings()


## graphics_menu.c:graphics_items, in the C order: Display, Scaler,
## Scaling Mode, CRT, Sprites, Visual FX, Explosions.
func _graphics_items() -> Array:
	# graphics_menu.c:disp_count = 1 + SDL_GetNumVideoDisplays; disp_label:
	# "gfx_window" then "<gfx_display_n> i".
	var disp_labels := ["gfx_window"]
	for i in range(1, 1 + DisplayServer.get_screen_count()):
		disp_labels.append("%s %d" % [TyrianText.L("gfx_display_n"), i])
	var sprites := _sprite_items()
	return [
		_item("gfx_display", "gfx_display_desc", Kind.PICKER, {
			"get": func() -> int: return settings.display,
			"set": func(i: int) -> void: settings.display = i,
			"count": disp_labels.size(), "labels": disp_labels, "popup": true,
		}),
			_item("gfx_scaler", "gfx_scaler_desc", Kind.PICKER, {
				"get": func() -> int: return DisplaySettings.scaler(),
				"set": func(i: int) -> void:
					DisplaySettings.set_scaler(i)
					Effects.apply_display_settings(),
				"count": DisplaySettings.SCALER_KEYS.size(),
				"labels": DisplaySettings.SCALER_KEYS, "popup": true,
			}),
			_item("gfx_scaling_mode", "gfx_scaling_mode_desc", Kind.PICKER, {
				"get": func() -> int: return DisplaySettings.scaling_mode(),
				"set": func(i: int) -> void:
					DisplaySettings.set_scaling_mode(i)
					Effects.apply_display_settings(),
				"count": DisplaySettings.SCALING_MODE_KEYS.size(),
				"labels": DisplaySettings.SCALING_MODE_KEYS, "popup": true,
			}),
			_item("gfx_crt", "gfx_crt_desc", Kind.PICKER, {
				"get": func() -> int: return DisplaySettings.crt_mode(),
				"set": func(i: int) -> void: Effects.set_crt_mode(i),
				"count": DisplaySettings.CRT_MODE_KEYS.size(),
				"labels": DisplaySettings.CRT_MODE_KEYS,
			}),
			_item("gfx_sprites", "gfx_sprites_desc", Kind.PICKER, {
				"get": sprites["get"],
				"set": sprites["set"],
				"count": sprites["count"], "labels": sprites["labels"],
			}),
			_item("gfx_visual_fx", "gfx_visual_fx_desc", Kind.PICKER, {
				"get": func() -> int: return settings.visual_fx,
				"set": func(i: int) -> void: settings.visual_fx = i,
				"count": VISUAL_FX_LABELS.size(), "labels": VISUAL_FX_LABELS,
			}),
			# graphics_menu.c:exfx_* (Classic / Retro+ / Modern).
			_item("gfx_exfx", "gfx_exfx_desc", Kind.PICKER, {
				"get": func() -> int: return int(settings.explosion_style),
				"set": func(i: int) -> void: settings.explosion_style = i % 3,
				"count": 3, "labels": ["gfx_exfx_classic", "gfx_exfx_retro", "gfx_exfx_modern"],
			}),
		_item("setup_done", "setup_done_back", Kind.BACK),
	]


## graphics_menu.c:sprite_get/sprite_set — same setting as the Display screen
## (DisplaySettings.art_mode), in the C order (Original, Remastered,
## Upscaled). A mode whose images are missing is removed from the list
## (sprite_set refuses an unavailable mode).
func _sprite_items() -> Dictionary:
	var modes := []
	var labels := []
	for i in SPRITE_MODES.size():
		if SPRITE_MODES[i] == DisplaySettings.ArtMode.HD and not DisplaySettings.hd_available():
			continue
		modes.append(SPRITE_MODES[i])
		labels.append(SPRITE_KEYS[i])
	return {
		"count": modes.size(),
		"labels": labels,
		"get": func() -> int: return maxi(modes.find(DisplaySettings.effective_art_mode()), 0),
		"set": func(i: int) -> void:
			DisplaySettings.set_art_mode(modes[clampi(i, 0, modes.size() - 1)])
			Effects.apply_display_settings(),
	}


## Labels of the music engine (sound_menu.c:eng_label, Music.ENGINE_*).
const MUSIC_ENGINE_LABELS := ["snd_engine_classic", "snd_engine_authentic", "snd_engine_enhanced"]

## sound_menu.c:sound_items, same rows, same order. Classic and Authentic
## are the offline renders of music.mus by the C DOSBox and Nuked
## OPL3 emulators (tools/music-render); Enhanced, the MIDI.
func _sound_items() -> Array:
	return [
		_volume("snd_music", "snd_music_desc", "music_volume", "music_disabled"),
		_volume("snd_sfx", "snd_sfx_desc", "fx_volume", "samples_disabled"),
		_toggle("snd_stereo", "snd_stereo_desc", "stereo_enabled"),
		_toggle("snd_sfx_pitch", "snd_sfx_pitch_desc", "sfx_pitch_variation"),
		_toggle("snd_sfx_impact", "snd_sfx_impact_desc", "sfx_impact_scaling"),
		_item("snd_music_engine", "snd_music_engine_desc", Kind.PICKER, {
			"get": func() -> int: return settings.music_engine,
			"set": func(i: int) -> void: settings.music_engine = i,
			"count": MUSIC_ENGINE_LABELS.size(),
			"labels": MUSIC_ENGINE_LABELS,
		}),
		_toggle("snd_music_widen", "snd_music_widen_desc", "music_pseudo_stereo"),
		_item("setup_done", "setup_done_back", Kind.BACK),
	]


## sound_menu.c: CUSTOM volume row (vol_draw, vol_act).
func _volume(label: String, help: String, volume: String, disabled: String) -> Dictionary:
	return _item(label, help, Kind.CUSTOM, {
		"volume": volume, "disabled": disabled, "draw": _draw_volume,
		# vol_act(dir = 0): mutes / restores the channel.
		"activate": func(_it: Dictionary) -> void:
			Sfx.play(Sfx.SELECT)
			settings[disabled] = not settings[disabled],
		# vol_act → nortsong.c:JE_changeVolume (±12, clamped to 0..255, S_CLINK at the limit).
		"adjust": func(_it: Dictionary, dir: int) -> void:
			Sfx.play(Sfx.CURSOR)
			var v := int(settings[volume]) + dir * 12
			if v < 0 or v > 255:
				Sfx.play(Sfx.CLINK)
			settings[volume] = clampi(v, 0, 255),
		# sound_menu.c:vol_click: click on the label = mute / restore; on
		# the bar (VOL_VALUE_W = 96 px after VOL_NAME_W = 150) = value.
		"click": func(_it: Dictionary, rel_x: int, _rel_y: int) -> void:
			if rel_x < 150:
				Sfx.play(Sfx.SELECT)
				settings[disabled] = not settings[disabled]
				return
			var in_value := rel_x - 150
			if in_value < 0 or in_value >= 96:
				return
			settings[volume] = clampi(in_value * 255 / (96 - 1), 0, 255)
			Sfx.play(Sfx.CURSOR),
	})


## sound_menu.c:vol_draw (CUSTOM row): label grayed out if the channel is
## muted, JE_barDrawShadow bar (one notch per 8 units) and JE_rectangle frame.
func _draw_volume(it: Dictionary, x: int, y: int, selected: bool) -> void:
	var disabled: bool = settings[it.disabled]
	var value := -3 + (2 if selected else 0) + (-4 if disabled else 0)
	TyrianText.draw_font_hv_shadow(self, x, y, TyrianText.L(it.label), NORMAL_FONT, LEFT, 15, value, false, 2)
	var value_x := x + 150  # VOL_NAME_W
	bar_draw_shadow(value_x, y, 1, 170 if disabled else 174, (int(settings[it.volume]) + 4) / 8, 2, 10)
	vga.rectangle(VGA_SCREEN, value_x - 2, y - 2, value_x + 96, y + 11, 242)  # VOL_VALUE_W


# --- Input ---------------------------------------------------------------------

## Navigation of run_menu and picker_popup: shared engine (ADR 0121).
func handle_action(action: StringName) -> void:
	if _busy or not idle:
		return
	menus.handle_action(action)
	if settings != _applied:
		apply_settings()
		save_settings()


## Mouse of run_menu and picker_popup (shared engine), settings applied
## and saved as with the keyboard.
func handle_mouse(event: InputEvent) -> void:
	if joystick.capturing() and idle:
		# joystick.c:detect_joystick_assignment: a click cancels the capture;
		# hover and right click do not touch the menu during the loop.
		if TyrianMouse.is_button_press(event) and joystick.capture(event):
			menus.draw()
		return
	super(event)
	if settings != _applied:
		apply_settings()
		save_settings()


## Exit from run_menu from Setup: back to the title (`restart = true`, without
## fade to black: the title fades directly from the Setup palette to
## its own).
func _leave() -> void:
	idle = false
	_busy = true
	Game.goto_scene(Game.SCENE_TITLE)
