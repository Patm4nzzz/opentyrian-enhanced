## Session state shared between scenes (`Game` autoload).
##
## Holds no simulation state: that lives in the
## TyrianSim node (Rust core) of the level scene.
extends Node

const SetupScreen := preload("res://scripts/setup.gd")
## Bits of the input mask (src/golden_master.c, tyrian-sim PlayerInput).
const MASK_UP := 1 << 0
const MASK_DOWN := 1 << 1
const MASK_LEFT := 1 << 2
const MASK_RIGHT := 1 << 3
const MASK_FIRE := 1 << 4
const MASK_CHANGE_FIRE := 1 << 5
const MASK_LEFT_SIDEKICK := 1 << 6
const MASK_RIGHT_SIDEKICK := 1 << 7

## Input actions (keyboard, gamepad, players 1 and 2) are defined by
## scripts/controls.gd (TyrianControls, ADR 0051) and configurable (userdata/controls.cfg).

const SCENE_TITLE := "res://scenes/title.tscn"
const SCENE_SHOP := "res://scenes/shop.tscn"
const HighScores := preload("res://scripts/high_scores.gd")
const SCENE_LEVEL := "res://scenes/level.tscn"
const SCENE_SETUP := "res://scenes/setup.tscn"
const SCENE_DEBRIEF := "res://scenes/debrief.tscn"
## Story pages, episode end, credits, high score entry
## (level_load.c:JE_loadMap, mainint.c:JE_nextEpisode).
const SCENE_STORY := "res://scenes/story_screens.tscn"
const SCENE_MODES := "res://scenes/modes_menu.tscn"
const SCENE_SAVE_MENU := "res://scenes/save_menu.tscn"
## title_screen.c:newGame and menus.c (choice of mode, episode, difficulty).
const SCENE_NEW_GAME := "res://scenes/new_game.tscn"
## title_screen.c:newGame: initial_cash[] (full game, episodes 1 to 5).
const INITIAL_CASH := [10000, 15000, 20000, 30000, 20000]
## Destruct mini-game (destruct.c): DESTRUCT code at the title, side modes, --destruct.
const SCENE_DESTRUCT := "res://scenes/destruct.tscn"

## Original `data/` folder (tyrian1.lvl...). Default: <repo>/data.
var data_dir := ""
var episode := 1
var lvl_file_num := 9
## Section of levelsN.dat whose shop is displayed.
var shop_section := 2
## Section of the shop that was left (the C's saveLevel), 0 when not leaving a shop.
var _shop_origin_section := 0
## ]I[ lists of the current shop (TyrianCampaign.run_script, "shop" stop):
## passed to TyrianShop.open, which handles purchases, resales and power levels.
var shop_items_available: Array = []
## Shop data cubes: cubeList (cube numbers, ]?) and cubeMax
## after the script directives (]? caps, ]! sets, ]+ adds), starting from
## the cubes collected in the previous level (debrief.cube_max).
var shop_cube_list := PackedInt32Array()
var shop_cube_max := 0
var difficulty := 2
var rng_seed := 1
## True when "--seed=N" forces the seed (tools, replayable runs).
var seed_forced := false
## Starting scroll position (0 = start of the level).
var start_cur_loc := 0
## Equipment of a new game (mainint.c:JE_initPlayerData).
const NEW_GAME_LOADOUT := [1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]
## Equipment passed to TyrianSim.set_loadout (same order).
var loadout := PackedInt32Array(NEW_GAME_LOADOUT)
## Side mode passed to TyrianSim.set_modes (empty = normal game, ADR 0050).
var modes := PackedInt32Array()
## Additions to `modes` (TyrianSim.set_modes_extra): [P1 money, P2 money,
## initial difficulty, P2 equipment]; empty = default values.
var modes_extra := []
## Masks of the mutators and anomalies chosen in the modes screen.
var mutators := 0


## Achievements profile file; empty for the tools (capture, replay), which must
## not write anything.
func ach_dat_path() -> String:
	if Capture.active or Bench.replay_assist or CampaignBot.replay_assist:
		return ""
	return UserData.path(CProfile.FILE_NAME)


## Command-line help (opentyr.c: --help). The Godot engine options
## (--fullscreen, --resolution, --audio-driver, --max-fps...) go
## before `--`, the game's after.
const GAME_HELP := """OpenTyrian2000 Enhanced: game options (place after "--").

  -h, --help             This help
  -s, --no-sound         Mute all audio
  -t DIR, --data=DIR     Tyrian data folder
  --lang=CODE            Language (en, fr, es, de, pt, it)
  --no-music             Mute the music only
  --hd-sprites           HD sprites (upscaled/ folder required)
  --mods=off | a,b       No mods, or only these mod folders
  --endless, --endless2  Start an endless run (1 or 2 players)
  --destruct             Start the Destruct mini-game
  --level=epN_NN         Start directly on this level
  --seed=N               Random generator seed
  --speed=N              Game speed (1..6)
  --devmode              Ctrl+Shift cheat keys
  -X, --xmas             Force Christmas mode, without the December prompt
  -x, --no-xmas          Disable Christmas mode (and no December prompt)
  --windowed, --fullscreen  Force windowed or fullscreen mode for this launch
  --user-dir=DIR         Absolute path of the player data folder (test tools;
                         default: userdata/ next to the executable)

C options with no equivalent here: -j (gamepad),
-n / -p / -d (network play), --goto, --autopilot, --smoke-test.
Engine options: godot --help.
"""


## locale.c:current_language (0..5): index of the current language code.
func language_index() -> int:
	return maxi(SetupScreen.LANGUAGE_CODES.find(TyrianText.language), 0)


## mod_manager.c:enabled_mod_count: mods active and loaded without error.
func enabled_mod_count() -> int:
	var n := 0
	for m in Mods.list():
		if bool(m.get("enabled", false)) and str(m.get("error", "")).is_empty():
			n += 1
	return n


## Achievements unlocked outside a level (episode end, Arcade Entry): the C's banner
## queue survives the screen change, the next level displays them.
var ach_pending: Array[int] = []


## Unlocks outside a level: profile written, banner at the next level.
func ach_unlocked_outside_level(ids: PackedInt32Array) -> void:
	for id in ids:
		ach_pending.append(id)
	var path := ach_dat_path()
	if not ids.is_empty() and not path.is_empty():
		ClassDB.class_call_static("TyrianVga", "ach_save_current", path)


## mainint.c:JE_nextEpisode: episode-end achievements (before the episode change).
func ach_episode_done(finished_episode: int, mutators: int) -> void:
	if ClassDB.class_exists("TyrianVga"):
		ach_unlocked_outside_level(
			ClassDB.class_call_static("TyrianVga", "ach_episode_done", ach_dat_path(), finished_episode, mutators)
		)


## Forced local hour (0..23) for Night Player; -1 = system clock (tests).
var local_hour_override := -1


## Local hour for Night Player (achievements.c:achievements_check, localtime()).
## `local_hour_override` (tests) then `--unix-time=N` (frozen captures, like
## time() under fixed_seed_shim.c), otherwise the system clock.
func local_hour() -> int:
	if local_hour_override >= 0:
		return local_hour_override
	if Capture.has_option("unix-time"):
		var bias: int = Time.get_time_zone_from_system()["bias"]
		return int(Time.get_datetime_dict_from_unix_time(int(Capture._options["unix-time"]) + bias * 60)["hour"])
	return int(Time.get_datetime_dict_from_system(false)["hour"])


## B-02: `modes` (10 values of TyrianSim.set_modes) with the Setup mutators added to
## `modes[6]` when `apply` (player's game; replays keep those of the golden master).
func modes_with_mutators(apply := true) -> PackedInt32Array:
	var m := modes.duplicate()
	if apply and m.size() >= 7:
		m[6] = int(m[6]) | mutators
	return m


var anomalies := 0
## Setup > Gameplay (setup.gd, userdata/setup.cfg): combos.c:combo_enabled
## (Endless combos) and combos.c:auto_fire_enabled (permanent fire of
## both players, player_movement.c: button[0] || auto_fire_enabled).
var combos_enabled := true
## stats.c:level_stats_reset (start_ticks): start time of the current level
## (Time.get_ticks_msec), read by the debriefing statistics screen.
var level_start_ms := 0
var auto_fire := false
## Setup > Graphics > Visual FX: `config.c:processorType` of the effects preset
## (1 = Off/Classic, 3 = Basic/High, 4 = Extended/Wild by default). Read by the
## simulation when loading the level (level.gd -> TyrianSim.set_processor_type).
var processor_type := 4

## --- Campaign loop (tyrian2.c:JE_main + level_load.c:JE_loadMap)
## True when levels chain according to levelsN.dat (otherwise the end of a
## level returns to the title).
var campaign_active := false
## mainLevel / nextLevel (sections of levelsN.dat, 1-based).
var main_level := 0
var next_level := 0
## initialDifficulty (player's choice); `difficulty` = difficultyLevel.
var initial_difficulty := 2
## stats_count_game_start: one more game, counted at the first level (achievement tracking only exists then).
var game_start_uncounted := false
var level_name := ""
## config.c:lastLevelName: name of the last COMPLETED level ("NEW GAME" at the start,
## "Completed" at the end of an episode). Saves write it, not `level_name`
## (current level): F5 in the middle of a level therefore keeps the previous level.
var last_level_name := ""
## Shop map (]G[): sections offered by "Next Level".
var map_sections := PackedInt32Array()
## Planets of these destinations (]G[, mapPlanet, 1-based: key planet_<n-1>).
var map_planets := PackedInt32Array()
## level_load.c ]G: starting planet of the "Next Level" map (mapOrigin, 1-based).
var map_origin := 1
## Summary set by the level scene for the debriefing.
var debrief := {}
## True when the level scene left its last image in the shared
## VGA (JE_endLevelAni writes over the frozen VGAScreen).
var debrief_frozen := false
## State of player[] inherited from the previous level (TyrianSim.campaign_carry),
## empty at the first level of a game.
var campaign_carry := PackedInt32Array()
## ]x[ (extraGame): the current shop offers a mini-game; the
## "Next Level" menu then shows "Bonus Game" (game_menu.c, miscText[67]).
var extra_game := false
## Current level preceded by ]e[ (ENGAGE): ** ALE ** mini-game, SQUADRON or
## TIME WAR (TyrianCampaign.run_script, "mini_game" key).
var mini_game := false
## config.c:trentWin, set when the Super Arcade loop reveals NortShipZ (level_load.c:630);
## the title then shows the "Sleep well, Trent" ending screen and quits (opentyr.c:423).
var trent_win := false
## level_load.c: bonusLevelCurrent || normalBonusLevelCurrent ("$" in
## column 28 or 27 of the current level's ]L directive). Death in a
## bonus level: ordinary level end, no game over
## (tyrian2.c:JE_main, see campaign_level_finished).
var bonus_level_current := false
## config.c:JE_saveGame(11) set by ]b[ ("Save Backup") before the shop:
## equipment, money, difficulty and section (saveLevel, ]s[) that
## JE_loadGame(11) restores on leaving a mini-game. Empty = no save.
var campaign_backup := {}
var _campaign: RefCounted

## `.rep` replay of the C version to play back in the level scene (--rep=).
var replay_file := ""
## Shop row to find again on returning from the Options screen (save);
## 0: first row (game_menu.c keeps curSel[curMenu]).
var shop_resume_sel := 0
## game_menu.c:curSel[] of the Options, Load / Save, Keyboard, Joystick and
## Mouse screens: row left on exit, found again on coming back during the same
## shop visit (reset to the first row on entering
## JE_itemScreen, game_menu.c:335-337). Key: screen name.
var shop_menu_rows := {}

## Mode of the current game chosen at the title (menus.c:gameplaySelect, title
## codes): values of TyrianSim.set_modes and set_modes_extra reapplied at
## each campaign level (empty = full one-player game).
var campaign_modes := PackedInt32Array()
var campaign_modes_extra := []
## config.c:timeBattleSelection (1..3) in Timed Battle, 0 otherwise.
var time_battle_selection := 0
## "LAST LEVEL" save of a two-player game (the C writes it to
## slot 22, which the rewrite does not have): kept in memory.
var _two_player_last_level := {}

## --- Title demo (title_screen.c, play_demo) ---
## varz.c:demo_num (0 at launch; load_next_demo advances it from 1 to 5).
var demo_num := 0
## Demo to load by the next level scene (0 = normal game).
var demo_pending := 0
## C state that survives from one game to the next and that the demo reads:
## player[0].cash (neither the demo nor JE_initPlayerData reset it to zero),
## initialDifficulty (0 at launch), friction counters of player[].
var demo_cash := 0
var c_initial_difficulty := 0
var demo_carry := PackedInt32Array()


func _ready() -> void:
	_install_input_actions()
	# Windows mouse setting read at startup (a reg query blocks for a few
	# tens of ms) and not at the first mouse movement in a level.
	MouseSystemScale.ensure_loaded()
	data_dir = _default_data_dir()
	# Profile of the C version (settings, high scores): one-time import, before
	# reading the saved language.
	var imported := CProfile.import_settings_and_scores(campaign())
	if CProfile.import_achievements():
		imported.append("achievements and statistics")
	if not imported.is_empty():
		print("C profile imported: ", ", ".join(imported))
	# Language saved right away: the title (main scene) builds
	# its labels in its _ready. The rest of Setup after the Music / Sfx / Effects
	# autoloads (setup.gd).
	SetupScreen.load_language()
	(func() -> void: SetupScreen.load_settings()).call_deferred()
	var user_args := OS.get_cmdline_user_args()
	if user_args.has("-h") or user_args.has("--help"):
		# `--help` alone is taken by the engine: the game options are read
		# after `--` (OpenTyrian2000-Enhanced -- --help), as the C does.
		print(GAME_HELP)
		get_tree().quit()
		return
	for i in user_args.size():
		var arg: String = user_args[i]
		if arg.begins_with("--data="):
			data_dir = arg.trim_prefix("--data=")
		elif arg == "-t" and i + 1 < user_args.size():
			# Short equivalent of --data=DIR (opentyr.c: -t, --data).
			data_dir = user_args[i + 1]
		elif arg.begins_with("--lang="):
			# Captures: takes precedence over the file, set before the main scene.
			TyrianText.language = arg.trim_prefix("--lang=")
		elif arg.begins_with("--level="):
			var id := parse_level_id(arg.trim_prefix("--level="))
			if not id.is_empty():
				episode = id[0]
				lvl_file_num = id[1]
		elif arg.begins_with("--start-loc="):
			start_cur_loc = int(arg.trim_prefix("--start-loc="))
		elif arg.begins_with("--seed="):
			rng_seed = int(arg.trim_prefix("--seed="))
			seed_forced = true
		elif arg.begins_with("--rep="):
			replay_file = arg.trim_prefix("--rep=")
	Christmas.apply_command_line()
	# "Play from here" (editor plugin): the level scene is already the
	# main scene launched by the editor, so it is not reloaded.
	var req := LevelMap.take_start_request()
	if not req.is_empty():
		start_level_at(
			int(req.get("episode", episode)),
			int(req.get("lvl_file_num", lvl_file_num)),
			int(req.get("cur_loc", 0)),
			{"load_scene": false}
		)
	# --endless: equivalent of the C's --goto=endless (opentyr.c). With
	# --capture, the game is prepared right away and the capture tool
	# loads the level scene itself.
	var args := OS.get_cmdline_user_args()
	# --destruct: equivalent of the C's --goto=destruct (opentyr.c); with
	# --capture, the capture tool loads the scene itself.
	if args.has("--destruct") and not Array(args).any(func(a: String) -> bool: return a.begins_with("--capture=")):
		(func() -> void: goto_scene(SCENE_DESTRUCT)).call_deferred()
	if args.has("--endless") or args.has("--endless2"):
		# --endless2: equivalent of --goto=endless2 (two players).
		var two := args.has("--endless2")
		if Array(args).any(func(a: String) -> bool: return a.begins_with("--capture=")):
			EndlessSession.start(false, two)
		else:
			EndlessSession.start.call_deferred(true, two)


## `ep1_09` -> [1, 9] (episode, level); [] if the name does not follow this format.
static func parse_level_id(id: String) -> PackedInt32Array:
	var parts := id.trim_prefix("ep").split("_")
	if parts.size() != 2:
		return PackedInt32Array()
	return PackedInt32Array([int(parts[0]), int(parts[1])])


## Only way to launch a level at a scroll position (ADR 0126):
## Level editor screen, the editor's "Play from here", campaign and endless
## mode. `cur_loc` > 0: the level scene calls TyrianSim.fast_forward_to.
## The rest of the game (difficulty, equipment, modes, campaign) is set
## by the caller before the call.
## options: "load_scene" (default true) — false when the level scene
## is already loaded or will be loaded by the caller (editor, capture tool).
## mainint.c:1124-1130 (JE_initPlayerData): modes of the previous game reset to zero
## (Timed Battle, Super Arcade, Super Tyrian, two players, mini-game).
func reset_game_modes() -> void:
	campaign_active = false
	mini_game = false
	campaign_modes = PackedInt32Array()
	campaign_modes_extra = []
	time_battle_selection = 0
	modes = PackedInt32Array()
	modes_extra = []


func start_level_at(ep: int, level_file: int, cur_loc: int, options := {}) -> void:
	episode = ep
	lvl_file_num = level_file
	start_cur_loc = cur_loc
	if options.get("load_scene", true):
		goto_scene(SCENE_LEVEL)


func level_id() -> String:
	return "ep%d_%02d" % [episode, lvl_file_num]


func _default_data_dir() -> String:
	var env := OS.get_environment("TYRIAN_DATA")
	if not env.is_empty():
		return env
	# Exported game (package from scripts/package.sh): data/ next to the executable
	# (Contents/Resources/data in the macOS bundle: GamePaths).
	if not OS.has_feature("editor"):
		var beside := GamePaths.shipped("data")
		if DirAccess.dir_exists_absolute(beside):
			return beside
	# res:// = <repo>/rewrite/godot -> <repo>/data
	return ProjectSettings.globalize_path("res://").path_join("../../data").simplify_path()


func _install_input_actions() -> void:
	TyrianControls.install()


## Player 1 mask (keyboard, gamepad 1); Auto-Fire adds the fire bit.
func read_input_mask() -> int:
	return TyrianControls.read_mask(1) | (MASK_FIRE if auto_fire else 0)


## Player 2 mask (own keys, gamepad 2) — two-player modes.
func read_input_mask_p2() -> int:
	return TyrianControls.read_mask(2) | (MASK_FIRE if auto_fire else 0)


## Shutdown requested by quit_cleanly (first code kept).
var _quit_requested := false


## Quits the program after releasing the audio streams and letting the
## audio server finish its reads: quitting directly leaves OGG
## resources held at shutdown (ERROR "resources still in use").
## The first requested code is kept: a failure is not erased by a
## success requested during the wait.
## Window close button or SIGTERM: the C goes through JE_tyrianHalt, so the profile
## (achievements and lifetime counters) is written before quitting.
func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		Effects.save_window_now()
		var dat := ach_dat_path()
		if not dat.is_empty() and ClassDB.class_exists("TyrianVga"):
			ClassDB.class_call_static("TyrianVga", "ach_save_current", dat)


func quit_cleanly(code := 0) -> void:
	if _quit_requested:
		return
	_quit_requested = true
	Effects.save_window_now()
	# opentyr.c:JE_tyrianHalt → JE_saveConfiguration: lifetime counters written.
	var dat := ach_dat_path()
	if not dat.is_empty() and ClassDB.class_exists("TyrianVga"):
		ClassDB.class_call_static("TyrianVga", "ach_save_current", dat)
	Music.release_streams()
	Sfx.release_streams()
	var tree := get_tree()
	for i in 3:
		await tree.process_frame
	await tree.create_timer(0.1).timeout
	tree.quit(code)


## Scenes live in the game viewport of the Effects autoload.
func goto_scene(path: String) -> Node:
	return Effects.show_scene(path)


## TyrianCampaign (GDExtension) loaded on demand.
func campaign() -> RefCounted:
	if _campaign == null and ClassDB.class_exists("TyrianCampaign"):
		_campaign = ClassDB.instantiate("TyrianCampaign")
		_campaign.load(data_dir)
	if _campaign != null and Mods.api != null:
		_campaign.set_mods(Mods.api)
	return _campaign


## title_screen.c:replay_browser, replay accepted: playback of the `.rep` in
## the level scene (level, equipment, seeds and mutators come from the
## file, TyrianSim.load_replay). No campaign, Endless or
## side-mode state must get mixed in; the end of playback returns to the title.
func start_replay(path: String) -> void:
	EndlessSession.stop()
	reset_game_modes()
	campaign_active = false
	mini_game = false
	modes = PackedInt32Array()
	modes_extra = []
	start_cur_loc = 0
	replay_file = path
	goto_scene(SCENE_LEVEL)


## mainint.c:JE_initPlayerData + JE_initEpisode: new game at
## episode `ep`, then JE_loadMap from the first section.
##
## `options` (title_screen.c:newGame, newSuperArcadeGame, newSuperTyrianGame):
## - loadout: starting equipment (otherwise that of JE_initPlayerData, initial
##   money initial_cash[episode]);
## - difficulty: difficultyLevel if different from initialDifficulty (two
##   players: difficultyLevel++);
## - modes / modes_extra: TyrianSim.set_modes / set_modes_extra of each
##   level (arcade, two players, Timed Battle, Super Arcade, Super Tyrian);
## - time_battle: timeBattleSelection (Timed Battle).
func start_campaign(ep: int, init_difficulty: int, options := {}) -> void:
	# game_menu.c:337: curSel = 2 on each entry of JE_itemScreen.
	shop_resume_sel = 0
	EndlessSession.stop()
	game_start_uncounted = true
	campaign_active = true
	episode = ep
	initial_episode = ep
	game_has_repeated = false
	initial_difficulty = init_difficulty
	c_initial_difficulty = init_difficulty
	difficulty = int(options.get("difficulty", init_difficulty))
	campaign_modes = options.get("modes", PackedInt32Array())
	campaign_modes_extra = options.get("modes_extra", [])
	time_battle_selection = int(options.get("time_battle", 0))
	_two_player_last_level = {}
	# mainint.c:JE_initPlayerData: secretHint = (rng_next() % 3) + 1.
	secret_hint = randi() % 3 + 1
	modes = campaign_modes
	modes_extra = campaign_modes_extra
	replay_file = ""
	demo_pending = 0
	# "starter" sections of the active mods (mainint.c:JE_initPlayerData); without mods, the C equipment.
	var cash: int = INITIAL_CASH[clampi(ep, 1, 5) - 1]
	loadout = options.get("loadout", Mods.starter_loadout(PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, cash])))
	start_cur_loc = 0
	main_level = 1
	# mainint.c:JE_initPlayerData: lastLevelName = newGameName (10 characters at most).
	level_name = TyrianText.L("save_new_game").left(10)
	last_level_name = level_name
	campaign_carry = PackedInt32Array()
	campaign_backup = {}
	mini_game = false
	advance_campaign()


## level_load.c:JE_loadMap (script part) from `main_level`, then
## opening of the corresponding scene.
## Level name (L directive of the episode script) of section `section`,
## for the HUD name box during a replay; empty if not found.
func replay_level_name(ep: int, section: int, difficulty: int) -> String:
	var c := campaign()
	if c == null:
		return ""
	# `section` = mainLevel at the start of the level: the script may stop first
	# on a shop (section 2 -> shop, then TYRIAN in section 3).
	for start in [section, section + 1, section + 2]:
		var r: Dictionary = c.run_script(ep, start, difficulty, 1, true, 0, false, 0)
		if str(r.get("stop", "")) == "level":
			return str(r.get("level_name", ""))
	return ""


func advance_campaign() -> void:
	var c := campaign()
	if c == null:
		push_error("GDExtension tyrian-godot missing: TyrianCampaign unavailable")
		goto_scene(SCENE_TITLE)
		return
	# level_load.c:JE_loadMap "saveLevel = mainLevel": the section where JE_loadMap
	# started. The shop choice is made INSIDE this loop in the C; here
	# it precedes advance_campaign, hence the shop section kept.
	var start_section := _shop_origin_section if _shop_origin_section > 0 else main_level
	_shop_origin_section = 0
	# ]2: twoPlayerMode || onePlayerAction; ]T / ]q: Timed Battle.
	var r: Dictionary = c.run_script(
		episode,
		main_level,
		initial_difficulty,
		loadout[0],
		debrief.get("all_players_alive", true),
		int(debrief.get("cube_max", 0)),
		_two_player_or_action(),
		time_battle_selection
	)
	r["start_section"] = start_section
	# Story pages (]P ]W ]C ]F ]A...), episode end (]Q[) and timed
	# battle end (]q[): the story scene shows them in the file's
	# order, then hands back to campaign_script_done.
	if _needs_story_screens(r):
		story_script = r
		goto_scene(SCENE_STORY)
		return
	_apply_script(r, true)


## True if the script requests a screen (level_load.c: display directives,
## "case 'Q'", "case 'q'" in timed battle).
func _needs_story_screens(r: Dictionary) -> bool:
	var stop := str(r.get("stop", ""))
	if stop == "episode_end" or (stop == "title" and time_battle_selection > 0):
		return true
	for e: Dictionary in r.get("screens", []):
		var d := str(e.get("directive", ""))
		if d.length() >= 2 and d[1] in ["P", "W", "U", "V", "R", "C", "B", "F", "A"]:
			return true
	return false


## End of the script screens (story scene): the continuation of JE_loadMap.
func campaign_script_done(r: Dictionary) -> void:
	_apply_script(r, false)


## Continuation of JE_loadMap after the script: map, autosave, then
## level, shop or return to the title. `play_songs`: false when the story
## scene has already played the ]M in file order.
func _apply_script(r: Dictionary, play_songs: bool) -> void:
	var start_section := int(r.get("start_section", main_level))
	if r.has("map") and not (r["map"] as PackedInt32Array).is_empty():
		var m: PackedInt32Array = r["map"]
		map_sections = PackedInt32Array()
		map_planets = PackedInt32Array()
		for i in range(1, m.size(), 2):
			map_planets.append(m[i - 1])
			map_sections.append(m[i])
		map_origin = int(r.get("map_origin", 1))
	if play_songs:
		for song in r.get("songs", PackedInt32Array()):
			Music.play_song(song - 1)  # ]M: play_song(temp - 1)
	if r.get("autosave", false):
		# ]b[: JE_saveGame(11), with saveLevel set by ]s[ (current section).
		var save_level := int(r.get("save_level", -1))
		campaign_backup = {
			"episode": episode,
			"main_level": save_level if save_level > 0 else int(r.get("section", main_level)),
			"loadout": loadout.duplicate(),
			"difficulty": difficulty,
			"level_name": level_name
		}
		# Like the C, the "LAST LEVEL" save on disk is also written:
		# quitting during the mini-game resumes at this shop.
		save_campaign(AUTOSAVE_SLOT, TyrianText.L("save_last_level"), int(campaign_backup["main_level"]))
	# tyrian2.c: extraGame falls back on each JE_loadMap, ]x[ raises it again.
	extra_game = bool(r.get("extra_game", false))
	match str(r.get("stop", "eof")):
		"level":
			# tyrian2.c:JE_main "Save backup game" before the level: saveLevel
			# (starting section of the script or ]s[), lastLevelName still that
			# of the previous level. Not before a mini-game ("!doNotSaveBackup":
			# ]e[ replaces the equipment with the arcade Stalker).
			var save_level := int(r.get("save_level", -1))
			if not r.has("mini_game"):
				_save_last_level(save_level if save_level > 0 else start_section)
			main_level = int(r["section"])
			next_level = int(r["next_level"])
			level_name = str(r["level_name"])
			bonus_level_current = bool(r.get("bonus", false)) or bool(r.get("normal_bonus", false))
			_set_mini_game(r.get("mini_game", {}))
			# JE_loadMap: a campaign level always starts at the beginning.
			start_level_at(episode, int(r["lvl_file_num"]), 0)
		"shop":
			set_shop(r, int(debrief.get("cube_max", 0)))
			goto_scene(SCENE_SHOP)
		_:
			# ]q (Timed Battle), end of file: return to the title.
			campaign_active = false
			goto_scene(SCENE_TITLE)


## One-player "arcade" or two-player mode (`]2` of levelsN.dat).
func _two_player_or_action() -> bool:
	return campaign_modes.size() >= 10 and (campaign_modes[0] != 0 or campaign_modes[2] != 0)


## Two-player game (menus.c, MENU_ITEM_2_PLAYER_ARCADE).
func two_player_game() -> bool:
	return campaign_modes.size() >= 10 and campaign_modes[0] != 0


## Setting of a side mode started from the "Side modes" screen (modes_menu.gd): it
## sets `modes` without a campaign (campaign_active false, campaign_modes empty), so the end
## screens did not see it. 0 outside a side mode. `modes` indices: 0 two players, 2 one-player
## arcade, 5 Timed Battle (1 or 2).
func side_mode_flag(index: int) -> int:
	if campaign_active or EndlessSession.active() or modes.size() < 10:
		return 0
	return int(modes[index])


## Shop menu of the mode (game_menu.c:JE_itemScreen, starting curMenu):
## 9 MENU_2_PLAYER_ARCADE, 10 MENU_1_PLAYER_ARCADE, 13 MENU_SUPER_TYRIAN,
## 0 MENU_FULL_GAME.
func shop_start_menu() -> int:
	if campaign_modes.size() < 10:
		return 0
	var menu := 0
	if campaign_modes[0] != 0:
		menu = 9
	if campaign_modes[2] != 0:
		menu = 10
	if campaign_modes[4] != 0:
		menu = 13
	return menu


## tyrian2.c:JE_main "Save backup game" (JE_saveGame(twoPlayerMode ? 22 : 11)).
func _save_last_level(section: int) -> void:
	if two_player_game():
		last_level_section = section
		_two_player_last_level = {
			"episode": episode,
			"section": section,
			"last_level": last_level_name,
			"difficulty": difficulty,
			"initial_difficulty": initial_difficulty,
			"cubes": int(debrief.get("cube_max", 0)),
			"loadout": loadout.duplicate()
		}
		return
	save_campaign(AUTOSAVE_SLOT, TyrianText.L("save_last_level"), section)


## level_load.c, ]e[ (ENGAGE) and ]g[ (GALAGA): forced equipment (Stalker
## 21.126, Atomic RailGun, money at zero), one player with lives, Galaga for
## ** ALE ** and SQUADRON. Outside a mini-game, normal game (modes cleared).
func _set_mini_game(mg: Dictionary) -> void:
	mini_game = not mg.is_empty()
	if not mini_game:
		modes = campaign_modes
		modes_extra = campaign_modes_extra
		return
	modes = mg["modes"]
	modes_extra = [0, 0, int(mg["initial_difficulty"]), mg["loadout2"]]
	loadout = mg["loadout"]


## tyrian2.c:JE_main, exit from an ENGAGE level (doNotSaveBackup):
## JE_loadGame(11) restores the equipment, money, difficulty and section
## of the ]b[ save; superTyrian and onePlayerAction fall back.
func _restore_backup() -> void:
	mini_game = false
	modes = campaign_modes
	modes_extra = campaign_modes_extra
	if campaign_backup.is_empty():
		return
	episode = int(campaign_backup["episode"])
	loadout = (campaign_backup["loadout"] as PackedInt32Array).duplicate()
	difficulty = int(campaign_backup["difficulty"])
	main_level = int(campaign_backup["main_level"])


## Shop returned by TyrianCampaign.run_script ("shop" stop): section,
## ]I[ lists and data cubes; `cubes` = cubeMax before the directives.
func set_shop(r: Dictionary, cubes: int) -> void:
	main_level = int(r["section"])
	shop_section = main_level
	shop_items_available = r.get("items_available", [])
	var list: PackedInt32Array = r.get("cube_list", PackedInt32Array())
	if not list.is_empty():
		shop_cube_list = list
	shop_cube_max = int(r.get("cube_max", -1)) if int(r.get("cube_max", -1)) >= 0 else clampi(cubes, 0, 4)


## Exit from the shop ("Next Level", game_menu.c MENU_PLAY_NEXT_LEVEL):
## mainLevel = mapSection[choice]; a single destination → it.
func campaign_leave_shop(choice := 0, shop_loadout := PackedInt32Array()) -> void:
	# Shop purchases (TyrianShop.loadout): equipment and money for the next level.
	if shop_loadout.size() >= 11:
		loadout = shop_loadout
	_shop_origin_section = main_level
	# Shop lifetime counters (credits spent, weapons bought) written.
	if not ach_dat_path().is_empty() and ClassDB.class_exists("TyrianVga"):
		ClassDB.class_call_static("TyrianVga", "ach_save_current", ach_dat_path())
	if map_sections.is_empty():
		main_level += 1
	else:
		main_level = map_sections[clampi(choice, 0, map_sections.size() - 1)]
	advance_campaign()


## End of mainint.c:JE_nextEpisode (story scene): next episode at its
## first section; Super Tyrian game finished: return to the title.
func campaign_next_episode(_finished_episode: int, to_title: bool) -> void:
	# game_menu.c:337: curSel = 2 on each entry of JE_itemScreen.
	shop_resume_sel = 0
	if to_title:
		campaign_active = false
		goto_scene(SCENE_TITLE)
		return
	main_level = 1
	_shop_origin_section = 0
	advance_campaign()


## mainint.c:JE_totalScore of player 1: money plus equipment value
## (item tables of the current episode).
func total_score() -> int:
	var c := campaign()
	var v: int = c.total_score(episode, loadout) if c != null else -1
	return v if v >= 0 else int(loadout[10])


## mainint.c:JE_nextEpisode, return to episode 1: one time in six, SuperCarrot
## with the front and rear Banana Blast, power 1.
func give_super_carrot() -> void:
	loadout[0] = 2
	loadout[1] = 23
	loadout[2] = 1
	loadout[3] = 24
	loadout[4] = 1
	if (
		campaign_modes_extra.size() >= 4
		and campaign_modes_extra[3] is PackedInt32Array
		and (campaign_modes_extra[3] as PackedInt32Array).size() >= 4
	):
		campaign_modes_extra[3][3] = 24


## Called by the level scene when `reallyEndLevel` is reached.
func campaign_level_finished(stats: Dictionary) -> void:
	campaign_carry = stats.get("carry", PackedInt32Array())
	if mini_game and not bool(stats.get("all_players_alive", true)):
		# ** ALE ** and SQUADRON have no end event: they end
		# at game over. tyrian2.c: no JE_endLevelAni; JE_loadGame(11)
		# then JE_loadMap from saveLevel, the shop that offered the game.
		# Players stay dead for this JE_loadMap (]l: all_players_alive()),
		# as in the C where JE_loadGame does not revive them.
		_restore_backup()
		debrief = {"all_players_alive": false, "cube_max": int(debrief.get("cube_max", 0))}
		advance_campaign()
		return
	# tyrian2.c:JE_main: "(!all_players_dead() || normalBonusLevelCurrent ||
	# bonusLevelCurrent) && !playerEndLevel" leads to the debriefing and the next
	# level; otherwise (death in a normal level), game over.
	if not bool(stats.get("any_player_alive", stats.get("all_players_alive", true))) and not bonus_level_current:
		if time_battle_selection > 0:
			# tyrian2.c:JE_main: "if (timedBattleMode) { mainLevel = 0; return; }".
			campaign_active = false
			goto_scene(SCENE_TITLE)
			return
		_campaign_game_over()
		return
	debrief = stats.duplicate()
	debrief["level_name"] = level_name
	# player_collide.c:318 "nextLevel = evalue - 10000": a collected warp
	# replaces the next section of the script (secret level, tyrian2.c:287).
	if bool(stats.get("bonus_level", false)) and int(stats.get("next_level", 0)) > 0:
		next_level = int(stats["next_level"])
	# mainint.c:JE_endLevelAni: strcpy(lastLevelName, levelName).
	last_level_name = level_name
	var c := campaign()
	if c != null:
		# mainint.c:JE_endLevelAni: adjust_difficulty (difficulty restored
		# first: "difficultyLevel = oldDifficultyLevel").
		difficulty = c.adjust_difficulty(difficulty, initial_difficulty, int(stats.get("total_score", 0)))
	loadout[10] = int(stats.get("cash", loadout[10]))
	# player[1].cash: player 2's money carries over to the next level.
	if two_player_game() and stats.has("cash2") and campaign_modes_extra.size() >= 2:
		campaign_modes_extra[1] = int(stats["cash2"])
	# Actual end-of-level equipment (weapon power gained in play included):
	# it is what the next shop resells (player[0].items, JE_itemScreen).
	var end_loadout: PackedInt32Array = stats.get("loadout", PackedInt32Array())
	if end_loadout.size() >= 11:
		end_loadout[9] = loadout[9]
		loadout = end_loadout
	goto_scene(SCENE_DEBRIEF)


## Side-mode level finished alive: debriefing (JE_endLevelAni) then,
## with no campaign to continue, return to the title (campaign_after_debrief).
func side_mode_level_finished(stats: Dictionary) -> void:
	debrief = stats.duplicate()
	debrief["level_name"] = level_name
	goto_scene(SCENE_DEBRIEF)


## tyrian2.c:JE_main, death in a normal level (neither bonus nor mini-game):
## no debriefing; JE_loadGame(11) reloads "LAST LEVEL", written before
## the level, then JE_loadMap restarts from its section (saveLevel = section where
## JE_loadMap started, level_load.c: the shop): return to that
## shop with the equipment, money and difficulty from before.
## Players stay dead for this JE_loadMap (]l), as for the mini-games.
## Slot 11 unreadable or missing: return to the title (game lost).
func _campaign_game_over() -> void:
	if two_player_game():
		# JE_loadGame(22): save of both players, kept in memory.
		if _two_player_last_level.is_empty():
			campaign_active = false
			goto_scene(SCENE_TITLE)
			return
		load_campaign_slot(_two_player_last_level, false)
		return
	var slots := campaign_slots()
	var s = slots[AUTOSAVE_SLOT - 1] if slots.size() >= AUTOSAVE_SLOT else null
	if s == null or str(s.get("error", "")) != "":
		push_warning(
			"Game over: slot %d (%s) unreadable, returning to title" % [AUTOSAVE_SLOT, TyrianText.L("save_last_level")]
		)
		campaign_active = false
		goto_scene(SCENE_TITLE)
		return
	load_campaign_slot(s, false)


## Pause menu, "Quit" confirmed (tyrian2.c:JE_main, playerEndLevel):
## JE_loadGame(11) reloads "LAST LEVEL" and JE_loadMap restarts from its section
## (game menu), players alive; unreadable slot: return to the title.
func campaign_abandon_level() -> void:
	if time_battle_selection > 0:
		# tyrian2.c:330-334: "if (timedBattleMode) { mainLevel = 0; return; }" before
		# JE_loadGame, for quitting as for death: return to the title.
		campaign_active = false
		goto_scene(SCENE_TITLE)
		return
	if two_player_game():
		# tyrian2.c:336: "JE_loadGame(twoPlayerMode ? 22 : 11)"; the two-player save,
		# not the one-player slot 11. Kept in memory (_save_last_level).
		if _two_player_last_level.is_empty():
			campaign_active = false
			goto_scene(SCENE_TITLE)
			return
		# False: keeps the game modes (two players); quitting kills nobody.
		load_campaign_slot(_two_player_last_level, false)
		debrief.erase("all_players_alive")
		return
	var slots := campaign_slots()
	var s = slots[AUTOSAVE_SLOT - 1] if slots.size() >= AUTOSAVE_SLOT else null
	if s == null or str(s.get("error", "")) != "":
		campaign_active = false
		goto_scene(SCENE_TITLE)
		return
	load_campaign_slot(s)


## End of the debriefing: tyrian2.c "mainLevel = nextLevel" then JE_loadMap.
func campaign_after_debrief() -> void:
	if not campaign_active:
		goto_scene(SCENE_TITLE)
		return
	main_level = next_level
	if mini_game:
		# TIME WAR finished alive: the Enhanced C version (tyrian2.c) reloads the
		# save so as not to keep the arcade Stalker, then resumes
		# nextLevel (docs/divergences.md).
		_restore_backup()
		main_level = next_level
	advance_campaign()


## Full game episode 1, Normal, without going through the choice screens
## (tools and campaign tests). The title opens SCENE_NEW_GAME instead
## (title_screen.c:newGame: mode, episode, difficulty).
func start_new_game() -> void:
	start_campaign(1, 2)


## Title → "New Game": choice screens of title_screen.c:newGame.
func open_new_game() -> Node:
	var ng := load("res://scripts/new_game.gd")
	ng.flow = ng.Flow.NEW_GAME
	return goto_scene(SCENE_NEW_GAME)


## title_screen.c, "Demo" entry or 30 s of inactivity: play_demo, then
## JE_main → JE_loadMap → load_next_demo (demo_num from 1 to 5, looping).
## No campaign, Endless, mode or replay state gets mixed in.
func start_demo() -> Node:
	EndlessSession.stop()
	reset_game_modes()
	campaign_active = false
	mini_game = false
	modes = PackedInt32Array()
	modes_extra = []
	replay_file = ""
	start_cur_loc = 0
	demo_num = demo_num % 5 + 1
	demo_pending = demo_num
	return goto_scene(SCENE_LEVEL)


## tyrian2.c:JE_main, end of the demo (file exhausted, player dead or key):
## moveTyrianLogoUp = true, stop_song(), fade_black(10), then the title.
## `cash`, `carry`: player[] state kept for the next demo.
func end_demo(cash: int, carry: PackedInt32Array) -> void:
	demo_pending = 0
	demo_cash = cash
	demo_carry = carry
	load("res://scripts/title.gd").move_tyrian_logo_up = true
	Music.stop_song()
	goto_scene(SCENE_TITLE)


## Native campaign save (ADR 0129): one JSON file per
## slot in this folder (TyrianCampaign.save_campaign_slot). The campaign
## bot redirects it so as not to touch the player's saves.
var saves_dir: String = UserData.path("saves")
## Starting episode of the campaign (highscore.c: initial_episode_num),
## recorded in the saves (config.c: pItems[8]).
var initial_episode := 1
## config.c:gameHasRepeated: the game has already gone through all the episodes, no more
## high score (recorded in the saves).
var game_has_repeated := false
## config.c:secretHint (1..3): series of end-of-episode hints.
var secret_hint := 1
## Result of TyrianCampaign.run_script shown by the story scene.
var story_script := {}
## Old save of the C version (CProfile.find_c_only: game folder, or
## standard C location), imported only once (SAVES_DIR folder absent).
## config.h: 11 one-player slots, the last being the "LAST LEVEL"
## autosave written before each level (tyrian2.c:JE_main),
## named in the current language like the C (L("save_last_level")).
## config.h:SAVE_FILES_NUM: 22 slots (11 one-player + 11 two-player).
const SAVE_SLOTS := 22
## 11 slots per page (1 player / 2 players).
const SAVE_SLOTS_PER_PAGE := 11
## Last one-player slot (autosave).
const AUTOSAVE_SLOT := 11


## Saves folder (system path), created on first use; if it
## did not exist, one-time import of the old C save.
func _saves_dir(c: RefCounted) -> String:
	var dir := ProjectSettings.globalize_path(saves_dir)
	if not DirAccess.dir_exists_absolute(dir):
		DirAccess.make_dir_recursive_absolute(dir)
		var c_profile := CProfile.find_c_only()
		if saves_dir == UserData.path("saves") and not c_profile.is_empty():
			var n: int = c.import_c_save(c_profile, dir)
			print("C save imported: %d slot(s) %s" % [maxi(n, 0), c.last_error])
	return dir


## The 22 slots (null = empty; non-empty "error" = unreadable, refused):
## indices 0..10 = 1 player, 11..21 = 2 players (config.c:(slot-1) > 10).
func campaign_slots() -> Array:
	var c := campaign()
	if c == null:
		var empty := []
		empty.resize(SAVE_SLOTS)
		return empty
	return c.load_campaign_slots(_saves_dir(c))


## config.h:saveLevel: section of the last autosave, resumed
## by the quick save (input_map.c:input_action_quick_save).
var last_level_section := 1


## config.c:JE_saveGame: game in progress in `slot` (1..11 one player,
## 12..22 two players), resuming at section `section` (the C's saveLevel).
## False if the write fails (message in TyrianCampaign.last_error).
func save_campaign(slot: int, slot_name: String, section := -1, items := PackedInt32Array()) -> bool:
	if slot == AUTOSAVE_SLOT:
		last_level_section = main_level if section < 0 else section
	var c := campaign()
	if c == null:
		return false
	# config.c:JE_saveGame: player 2 equipment and money, devices.
	var two_p := modes.size() >= 1 and modes[0] != 0
	var lo2: PackedInt32Array = (
		modes_extra[3]
		if two_p and modes_extra.size() >= 4 and modes_extra[3] is PackedInt32Array
		else PackedInt32Array()
	)
	var cash2: int = int(modes_extra[1]) if two_p and modes_extra.size() >= 2 else 0
	# Appends cash2 to the end of lo2 (same order as loadout: 11 values).
	var lo2_full := PackedInt32Array()
	if two_p and lo2.size() >= 10:
		lo2_full.append_array(lo2)
		lo2_full.append(cash2)
	var devs := PackedInt32Array([int(modes[8]), int(modes[9])]) if two_p and modes.size() >= 10 else PackedInt32Array()
	var ok: bool = (
		c
		. save_campaign_slot(
			_saves_dir(c),
			{
				"slot": slot,
				"name": slot_name,
				"saved_at": Time.get_datetime_string_from_system(),
				"episode": episode,
				"section": main_level if section < 0 else section,
				"last_level": last_level_name,
				"difficulty": difficulty,
				"initial_difficulty": initial_difficulty,
				"cubes": int(debrief.get("cube_max", 0)),
				"loadout": loadout if items.size() < 11 else items,
				"loadout2": lo2_full,
				"input_devices": devs,
				"initial_episode": initial_episode,
				"game_has_repeated": game_has_repeated,
				"secret_hint": secret_hint,
				"super_arcade_mode": _save_mode_value(),
			}
		)
	)
	if not ok:
		push_warning("Save failed: %s" % c.last_error)
	return ok


## config.c:JE_saveGame: player[0].items.super_arcade_mode = SA_SUPERTYRIAN
## (254) in Super Tyrian, the ship (1..9) in Super Arcade, SA_ARCADE (255)
## for another "onePlayerAction" game, 0 for the full game.
func _save_mode_value() -> int:
	if campaign_modes.size() < 10 or campaign_modes[0] != 0:
		return 0
	if campaign_modes[4] != 0:
		return 254
	if campaign_modes[3] != 0:
		return int(campaign_modes[3])
	if campaign_modes[2] != 0:
		return 255
	return 0


## config.c:JE_loadGame: superTyrian, onePlayerAction and superArcadeMode
## re-read from super_arcade_mode; galagaMode and timedBattleMode fall back.
static func _modes_from_save_value(v: int) -> PackedInt32Array:
	if v == 0:
		return PackedInt32Array()
	var m := PackedInt32Array([0, 0, 1, 0, 0, 0, 0, 0, 0, 0])
	if v == 254:
		m[4] = 1
	elif v <= 9:
		m[3] = v
	return m


## config.c:JE_loadGame from a slot returned by campaign_slots:
## resumes the game at the saved section (its shop). 1 or 2 players.
## `players_alive`: false after a game over.
func load_campaign_slot(slot: Dictionary, players_alive := true) -> void:
	# game_menu.c:337: curSel = 2 on each entry of JE_itemScreen.
	shop_resume_sel = 0
	if players_alive:
		# Game loaded from the menu: full one-player game.
		campaign_modes = PackedInt32Array()
		campaign_modes_extra = []
		time_battle_selection = 0
	modes = campaign_modes
	modes_extra = campaign_modes_extra
	campaign_active = true
	episode = int(slot["episode"])
	initial_difficulty = int(slot["initial_difficulty"])
	difficulty = int(slot["difficulty"])
	loadout = PackedInt32Array(slot["loadout"])
	main_level = int(slot["section"])
	level_name = str(slot["last_level"])
	last_level_name = level_name
	# config.c:JE_loadGame: initial_episode_num (pItems[8]), gameHasRepeated,
	# secretHint; without them, the high score was never offered after
	# "Load Game" of a game started at an episode other than 1.
	initial_episode = int(slot.get("initial_episode", episode))
	game_has_repeated = bool(slot.get("game_has_repeated", false))
	secret_hint = clampi(int(slot.get("secret_hint", secret_hint)), 1, 3)
	debrief = {"cube_max": int(slot.get("cubes", 0))}
	if not players_alive:
		debrief["all_players_alive"] = false
	# config.c:JE_loadGame: modes, player 2 equipment and money, devices.
	var two_p: bool = int(slot.get("players", 1)) == 2
	if two_p:
		var lo2: PackedInt32Array = slot.get("loadout2", PackedInt32Array())
		var devs: PackedInt32Array = slot.get("input_devices", PackedInt32Array([1, 2]))
		# cash2 = last value of loadout2 (P2 money, like loadout[10]).
		var cash2: int = int(lo2[10]) if lo2.size() >= 11 else 0
		# lo2 without money (10 values) for modes_extra.
		var lo2_10 := lo2.slice(0, 10) if lo2.size() >= 10 else PackedInt32Array()
		var d1: int = int(devs[0]) if devs.size() >= 1 else 1
		var d2: int = int(devs[1]) if devs.size() >= 2 else 2
		# modes[0..9] = [two_player=1, galaga=0, one_player=0, super_arcade=0,
		#   super_tyrian=0, timed=0, mutators=0, anomalies=0, device_p1, device_p2].
		modes = PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, d1, d2])
		# modes_extra: [cash_p1, cash_p2, initial_difficulty, loadout2].
		modes_extra = [int(loadout[10]), cash2, initial_difficulty, lo2_10]
		# Game mode (2-player shop, "LAST LEVEL" save of
		# both players): campaign_modes, like start_campaign.
		campaign_modes = modes
		campaign_modes_extra = modes_extra
	else:
		# config.c:JE_loadGame: the one-player mode comes from the save. After a
		# game over, a file without a mode (older) keeps that of the game.
		var sa_save := int(slot.get("super_arcade_mode", 0))
		if players_alive or sa_save != 0:
			campaign_modes = _modes_from_save_value(sa_save)
			campaign_modes_extra = []
		modes = campaign_modes
		modes_extra = []
	campaign_carry = PackedInt32Array()
	campaign_backup = {}
	mini_game = false
	start_cur_loc = 0
	advance_campaign()
