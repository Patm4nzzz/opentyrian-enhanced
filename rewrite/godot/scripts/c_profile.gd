## Profile of the C version (`opentyrian.dat`): lookup at its standard
## location and one-time import, at first launch, of the settings and
## high scores (saves are imported by game.gd:_saves_dir).
## Nothing is overwritten: a file already present in the player's data
## folder (UserData) is left as is.
class_name CProfile

const FILE_NAME := "opentyrian.dat"
## config.c:get_user_directory: on Linux $XDG_CONFIG_HOME/opentyrian2000 or
## ~/.config/opentyrian2000; on Windows, the C executable's folder.
const LINUX_DIR_NAME := "opentyrian2000"
const LANGUAGE_CODES := ["en", "fr", "es", "de", "pt", "it"]
const SCALER_NAMES := ["None", "2x", "Scale2x", "hq2x", "3x", "Scale3x", "hq3x", "4x", "hq4x"]
const SCALING_MODE_NAMES := ["Center", "Integer", "Fit 8:5", "Fit 4:3", "Fit 16:9", "Fill"]
const CRT_MODE_NAMES := ["off", "scanlines", "crt_plus", "crt_premium", "crt_aperture", "crt_soft"]
const EXPLOSION_NAMES := ["classic", "retro_plus", "modern"]
## config.c:processorType (video.detail of the C profile, 1..6) -> Visual FX level of the
## Godot game (0 Off = 1, 1 Basic = 3, 2 Extended = 4, setup.gd:VISUAL_FX_PROCESSOR_TYPE).
## Levels without a level of their own take the closest one: 2 -> Basic, 5 -> Basic, 6 -> Extended.
const DETAIL_TO_VISUAL_FX := {1: 0, 2: 1, 3: 1, 4: 2, 5: 1, 6: 2}
## save_json.c:key_setting_names and enhanced_key_names.
const PLAYER_COMMANDS := ["up", "down", "left", "right", "fire", "change_fire", "left_sidekick", "right_sidekick"]
const ENHANCED_KEYS := [
	"achievements", "stats", "quick_save", "dps", "damage_numbers", "quick_load", "replay", "screenshot", "bullet_time"
]
## save_json.c:load_mutators_json, in the bit order of modes::MUT_*.
const MUTATOR_NAMES := [
	"fast_enemies", "armored", "bullet_hell", "glass_cannon", "no_powerups", "one_life", "mirror", "risk_reward"
]
## hd_sprite.c:sprite_mode_strings and ArtMode of display_settings.gd (ORIGINAL 0, SHARP 1, HD 2).
const SPRITE_MODE_NAMES := ["original", "remastered", "upscaled"]
const SPRITE_MODE_ART := [0, 2, 1]
const KEY_CODE_MASK := 0x3FFFFFFF
## SDL names that OS.find_keycode_from_string does not understand.
const SDL_KEY_ALIASES := {
	"Return": KEY_ENTER,
	"Left Ctrl": KEY_CTRL,
	"Right Ctrl": KEY_CTRL,
	"Left Alt": KEY_ALT,
	"Right Alt": KEY_ALT,
	"Left Shift": KEY_SHIFT,
	"Right Shift": KEY_SHIFT,
	"Escape": KEY_ESCAPE,
	"Space": KEY_SPACE,
	"Tab": KEY_TAB,
	"Backspace": KEY_BACKSPACE,
	"PageUp": KEY_PAGEUP,
	"PageDown": KEY_PAGEDOWN,
}


## Paths where to look for the C profile, in order of preference.
static func candidate_paths() -> Array[String]:
	var out: Array[String] = []
	var xdg := OS.get_environment("XDG_CONFIG_HOME")
	var home := OS.get_environment("HOME")
	if OS.get_name() == "Windows":
		# The C writes next to its executable: the Godot game unzipped in the same
		# place, or in the neighboring folder.
		var exe_dir := OS.get_executable_path().get_base_dir()
		out.append(exe_dir.path_join(FILE_NAME))
		out.append(exe_dir.get_base_dir().path_join(FILE_NAME))
		for sibling in ["OpenTyrian2000", "opentyrian2000"]:
			out.append(exe_dir.get_base_dir().path_join(sibling).path_join(FILE_NAME))
	else:
		# config.c:get_user_directory: XDG_CONFIG_HOME if non-empty, otherwise HOME.
		if not xdg.is_empty():
			out.append(xdg.path_join(LINUX_DIR_NAME).path_join(FILE_NAME))
		elif not home.is_empty():
			out.append(home.path_join(".config").path_join(LINUX_DIR_NAME).path_join(FILE_NAME))
	return out


## C profile to import: first existing candidate, never the Godot game's own
## file (UserData, same file name), even through `--user-dir` pointing
## at the executable's folder.
static func find_c_only() -> String:
	return first_c_profile(candidate_paths(), UserData.path(FILE_NAME))


static func first_c_profile(candidates: Array[String], own: String) -> String:
	for p in candidates:
		if p.simplify_path() != own.simplify_path() and FileAccess.file_exists(p):
			return p
	return ""


## One-time import of achievements and lifetime statistics (OTYE block): added to those
## already earned in Godot, never the reverse; the C file stays untouched.
## Marker separate from the rest: a profile already imported before this fix still receives
## its achievements. Returns true if the Godot file was written.
static func import_achievements() -> bool:
	var marker := UserData.dir().path_join("c_achievements_imported")
	if FileAccess.file_exists(marker):
		return false
	var path := find_c_only()
	if path.is_empty():
		return false
	DirAccess.make_dir_recursive_absolute(UserData.dir())
	var dest := UserData.dir().path_join(FILE_NAME)
	# 1 = written, 0 = nothing to add, -1 = failure: the marker is only set if
	# the import succeeded or had nothing to do, a failure is retried at the next launch.
	var status: int = ClassDB.class_call_static("TyrianVga", "import_c_achievements", path, dest)
	if status < 0:
		push_warning("Import of C profile achievements failed (%s), will retry on next launch" % path)
		return false
	var f := FileAccess.open(marker, FileAccess.WRITE)
	if f != null:
		f.store_string(path)
	return status == 1


## One-time import of settings and scores; returns what was written (for the
## log). `campaign`: TyrianCampaign (container reading).
static func import_settings_and_scores(campaign: RefCounted) -> Array[String]:
	var done: Array[String] = []
	var marker := UserData.dir().path_join("c_profile_imported")
	if campaign == null or FileAccess.file_exists(marker):
		return done
	var path := find_c_only()
	if path.is_empty():
		return done
	var profile: Dictionary = campaign.read_c_profile(path)
	if profile.is_empty():
		# Like the C (opentyrian_dat.c: "container digest mismatch..."), nothing on
		# screen; the trace goes to the log. The C file stays untouched.
		print("C profile unreadable, ignored: ", path)
		return done
	var user := UserData.dir()
	DirAccess.make_dir_recursive_absolute(user)
	var text := str(profile.get("settings_json", ""))
	var json: Variant = JSON.parse_string(text) if not text.is_empty() else null
	var settings: Dictionary = json if json is Dictionary else {}
	if not FileAccess.file_exists(user.path_join("setup.cfg")) and _write_setup(settings, user.path_join("setup.cfg")):
		done.append("settings")
	if (
		not FileAccess.file_exists(user.path_join("display.cfg"))
		and _write_display(settings, user.path_join("display.cfg"))
	):
		done.append("display")
	if (
		not FileAccess.file_exists(user.path_join("highscores.json"))
		and _write_scores(profile.get("high_scores", []), user.path_join("highscores.json"))
	):
		done.append("high scores")
	if (
		not FileAccess.file_exists(user.path_join("controls.cfg"))
		and _write_controls(settings, user.path_join("controls.cfg"))
	):
		done.append("keys")
	var f := FileAccess.open(marker, FileAccess.WRITE)
	if f != null:
		f.store_string(path)
	return done


static func _section(root: Dictionary, key: String) -> Dictionary:
	var v: Variant = root.get(key)
	return v if v is Dictionary else {}


static func _enum_index(names: Array, value: Variant, fallback: int) -> int:
	var i := names.find(str(value))
	return i if i >= 0 else fallback


## save_json.c:mouse_setting_names / mouse_setting_values.
const MOUSE_SETTING_NAMES := ["left_mouse", "right_mouse", "middle_mouse"]
const MOUSE_SETTING_VALUES := [
	"fire main weapon", "fire left sidekick", "fire right sidekick", "fire both sidekicks", "change rear mode"
]


## Settings shared with the setup.cfg format ([setup], keys of setup.gd:settings).
static func _write_setup(root: Dictionary, path: String) -> bool:
	if root.is_empty():
		return false
	var audio := _section(root, "audio")
	var gameplay := _section(root, "gameplay")
	var feat := _section(root, "features")
	var values := {}
	# video.fullscreen of the C (video.c:fullscreen_display, -1 = windowed): an explicit
	# value >= 0 is kept (screen + 1 here); -1 is the C default, replaced
	# by the rewrite's default (fullscreen, setup.gd:resolve_display).
	var video := _section(root, "video")
	if video.has("fullscreen") and int(video["fullscreen"]) >= 0:
		values["display"] = int(video["fullscreen"]) + 1
	if feat.has("language"):
		values["current_language"] = _enum_index(LANGUAGE_CODES, feat["language"], 0)
	if audio.has("music_volume"):
		values["music_volume"] = clampi(int(audio["music_volume"]), 0, 255)
	if audio.has("fx_volume"):
		values["fx_volume"] = clampi(int(audio["fx_volume"]), 0, 255)
	if audio.has("stereo"):
		values["stereo_enabled"] = bool(audio["stereo"])
	if audio.has("music_widen"):
		values["music_pseudo_stereo"] = bool(audio["music_widen"])
	if gameplay.has("game_speed"):
		values["game_speed"] = clampi(int(gameplay["game_speed"]), 1, 5)
	if video.has("detail") and DETAIL_TO_VISUAL_FX.has(int(video["detail"])):
		values["visual_fx"] = DETAIL_TO_VISUAL_FX[int(video["detail"])]
	var booleans := {
		"bullet_time": "bullet_time_enabled",
		"combo": "combo_enabled",
		"auto_fire": "auto_fire_enabled",
		"damage_numbers": "damage_numbers_enabled",
		"colored_damage": "colored_damage_numbers",
		"freeze_frames": "hitstop_enabled",
		"rumble": "rumble_enabled",
		"particles": "particles_enabled",
		"screen_shake": "screenshake_enabled",
		"sfx_pitch_variation": "sfx_pitch_variation",
		"sfx_impact_scaling": "sfx_impact_scaling"
	}
	for k in booleans:
		if feat.has(k):
			values[booleans[k]] = bool(feat[k])
	# save_json.c:load_mouse: left_mouse / right_mouse / middle_mouse, text -> 1..5.
	# (The C does not save the music / sound mute: nothing to import.)
	var mouse := _section(root, "mouse")
	if not mouse.is_empty():
		var buttons := [1, 4, 5]
		for i in MOUSE_SETTING_NAMES.size():
			var at := MOUSE_SETTING_VALUES.find(str(mouse.get(MOUSE_SETTING_NAMES[i], "")))
			if at >= 0:
				buttons[i] = at + 1
		values["mouse_buttons"] = buttons
	if feat.has("explosion_fx"):
		values["explosion_style"] = _enum_index(EXPLOSION_NAMES, feat["explosion_fx"], 1)
	# save_json.c:load_mutators_json: one boolean per mutator, in the order of the
	# bits of modes::MUT_*.
	var mut := _section(root, "mutators")
	if not mut.is_empty():
		var mask := 0
		for i in MUTATOR_NAMES.size():
			if bool(mut.get(MUTATOR_NAMES[i], false)):
				mask |= 1 << i
		values["mutators"] = mask
	# C music engine: music_backend (classic / enhanced) and opl_backend
	# (dosbox / nuked) give Classic, Authentic or Enhanced (sound_menu.c:eng_get).
	if feat.has("music_backend") or feat.has("opl_backend"):
		values["music_engine"] = music_engine_of(
			str(feat.get("music_backend", "enhanced")), str(feat.get("opl_backend", "dosbox"))
		)
	if values.is_empty():
		return false
	var cfg := ConfigFile.new()
	for k in values:
		cfg.set_value("setup", k, values[k])
	# Music engine numbering: the C one (without this line, the file
	# would be re-read as the old numbering, 0 OPL / 1 MIDI).
	cfg.set_value("format", "music_engine", 2)
	# Missing keys keep their defaults.
	return cfg.save(path) == OK


## Music engine (Music.ENGINE_*) from the C's two settings;
## save_json.c:load_features, without FluidSynth the C default is classic.
static func music_engine_of(music_backend: String, opl_backend: String) -> int:
	if music_backend == "enhanced":
		return 2
	return 1 if opl_backend == "nuked" else 0


## Display (display.cfg): scaler, scaling mode, CRT filter.
static func _write_display(root: Dictionary, path: String) -> bool:
	var video := _section(root, "video")
	var feat := _section(root, "features")
	var cfg := ConfigFile.new()
	var any := false
	if video.has("scaler"):
		cfg.set_value("display", "scaler", _enum_index(SCALER_NAMES, video["scaler"], 5))
		any = true
	if video.has("scaling_mode"):
		cfg.set_value("display", "scaling_mode", _enum_index(SCALING_MODE_NAMES, video["scaling_mode"], 1))
		any = true
	if feat.has("crt_mode"):
		cfg.set_value("display", "crt_mode", _enum_index(CRT_MODE_NAMES, feat["crt_mode"], 0))
		any = true
	# C sprites: original / remastered / upscaled. The C "remastered" images
	# are Godot's HD images, "upscaled" is the sharp rendering (xBRZ).
	if feat.has("sprite_mode"):
		var art := SPRITE_MODE_NAMES.find(str(feat["sprite_mode"]))
		if art >= 0:
			cfg.set_value("display", "art_mode", SPRITE_MODE_ART[art])
			any = true
	return any and cfg.save(path) == OK


## Player 1 keys and common keys of the C (`keyboard` and
## `keyboard.enhanced` of opentyrian.json), written in the controls.cfg format.
## The C's menu keys, mouse buttons and gamepads do not exist
## in Godot. Slow motion (bullet_time) keeps its key: Godot chose
## Left Shift instead of Tab (docs/divergences.md no. 31).
static func _write_controls(root: Dictionary, path: String) -> bool:
	var kb := _section(root, "keyboard")
	if kb.is_empty():
		return false
	var cfg := ConfigFile.new()
	var any := false
	for c in PLAYER_COMMANDS:
		var code := key_from_sdl_name(str(kb.get(c, "")))
		if code != 0:
			cfg.set_value("player1", c + "_keys", [code])
			any = true
	var enh := _section(kb, "enhanced")
	for n in ENHANCED_KEYS:
		if n == "bullet_time":
			continue
		var code := key_from_sdl_name(str(enh.get(n, "")))
		if code != 0:
			cfg.set_value("global", "key_" + n + "_key", code)
			any = true
	return any and cfg.save(path) == OK


## Godot key (physical keycode) from an SDL name (SDL_GetScancodeName);
## 0 if unknown.
static func key_from_sdl_name(name: String) -> int:
	if name.is_empty():
		return 0
	if SDL_KEY_ALIASES.has(name):
		return SDL_KEY_ALIASES[name]
	var code := OS.find_keycode_from_string(name)
	return int(code) & KEY_CODE_MASK if code != KEY_NONE else 0


## High scores (same format as high_scores.gd:save_scores). Empty tables
## (no player score): nothing written, the default scores remain.
static func _write_scores(tables: Variant, path: String) -> bool:
	if not (tables is Array) or tables.size() != 20:
		return false
	var played := false
	for t in tables:
		for e in t:
			if int(e["score"]) > 0:
				played = true
	if not played:
		return false
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		return false
	f.store_string(JSON.stringify({"version": 1, "tables": tables}))
	return true
