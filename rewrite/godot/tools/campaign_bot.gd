## Full campaign bot (autoload `CampaignBot`, inactive without an argument).
##
##   godot --path rewrite/godot -- --campaign-bot=<log.jsonl> [options]
##
## Plays the campaign of episodes 1 to 5 through the REAL game screens: title
## ("New Game" by a key), shop (shop.tscn, menu keys injected as keyboard
## events), level (interactive level.tscn, game inputs through
## Input.action_press / action_release read by
## Game.read_input_mask in level.gd:_physics_process), debriefing (key
## at "Press a key"), episode transitions (]Q[ -> JE_nextEpisode, chained
## by Game.advance_campaign). No call to sim.step outside the game loop.
##
## At every shop: native save (ADR 0129) through Game.save_campaign
## into UserData campaign_bot/saves (slot = episode; same path as
## the shop's Options screen); at the second shop of each
## episode: the JSON file is read back through Game.campaign_slots then
## Game.load_campaign_slot ("Load Game" path), and the resume is checked
## (episode, section, equipment, money, difficulty).
##
## Options:
##   --campaign-bot-assist=0|1    autopilot helpers (default 1: same
##                                helpers as the "assist 1" golden masters,
##                                golden_master.c:apply_assist)
##   --campaign-bot-speed=<n>     speed-up: physics_ticks_per_second = 35*n,
##                                unsynchronized rendering (default 20; 1 = normal)
##   --campaign-bot-realtime      no speed-up, normal rendering (vsync)
##   --campaign-bot-loop          restarts the campaign until --campaign-bot-seconds
##   --campaign-bot-seconds=<n>   target duration: reaching it is a SUCCESS (code 0),
##                                with or without --campaign-bot-loop, as long as no
##                                failure has been recorded (default 0 = unlimited)
##   --campaign-bot-metrics=<csv> one line per second (fps, frame, memory...)
##   --campaign-bot-stall=<s>     blocking screen: no scene, no tick,
##                                no new shop state for s
##                                real seconds (default 60) -> failure
##   --campaign-bot-max-ticks=<n> tick cap per level (default 60000)
##   --campaign-bot-episodes=<n>  last episode to finish (default 5)
##   --campaign-bot-dodge         lateral dodging of shots and enemies above
##                                the ship (_dodge; default: aiming only)
##   --campaign-bot-drive=dodge   driving that plans its moves to
##                                avoid enemies, shots and obstacles while firing
##                                (TyrianDodgeBot, crate tyrian-dodge-bot: reads
##                                the published simulation state, returns a mask
##                                of directions + fire); designed for play without
##                                helpers
##   --campaign-bot-attempts=<n>  game overs allowed per level before declaring it
##                                impassable (default 3; the game over reloads
##                                "LAST LEVEL" and restarts the level)
##   --campaign-bot-buy=<c,...>   categories bought at each shop
##                                (default 4,6,5: rear gun, generator, shield;
##                                3 = front gun; for guns, power is
##                                also bought, see POWER_KEEP)
##   --campaign-bot-start-episode=<n> [--campaign-bot-start-section=<s>]
##                                diagnostic: direct start (Game.start_campaign)
##                                instead of "New Game" (episode 1)
## "** ALE **" and "SQUADRON" mini-games: played once per campaign,
## without helpers, until the game over that returns to their shop (_mini_games_played).
## Without --campaign-bot-start-episode, the game starts like a player: title
## ("New Game"), then the new_game.tscn screens (mode, episode, difficulty)
## confirmed with Enter on their default choices (1 player, episode 1, Normal).
## Exit: Game.quit_cleanly(0) if the campaign or the requested duration is
## reached without a blocking screen, 1 otherwise.
extends Node

const SCENE_DEBRIEF := "res://scenes/debrief.tscn"

var active := false
## Read by level.gd (same role as Capture.replay_assist / Bench.replay_assist).
var replay_assist := true

var _log_path := ""
var _metrics_path := ""
var _speed := 20
var _realtime := false
var _loop := false
var _seconds := 0.0
var _stall_s := 60.0
var _max_ticks := 60000
var _episodes_target := 5
var _max_attempts := 3
## Dodging of shots and enemies (--campaign-bot-dodge, default: no).
var _dodge_on := false
## Planned driving (--campaign-bot-drive=dodge): a single object for the whole
## campaign (it remembers the sprites that are harmless on contact).
var _planner: RefCounted
## Game overs per level (key "episode:name") in the current campaign.
var _game_overs := {}
## 0: new game through the title screen; n: Game.start_campaign(n, 2)
## (like the C episode choice, not ported), to resume an episode.
var _start_episode := 0
## Start section (diagnostic, with --campaign-bot-start-episode).
var _start_section := 1
## Categories bought at each shop (2 ship, 3 front gun, 4 rear
## gun, 5 generator, 6 shield...), in order.
var _buy_plan: Array = [4, 6, 5]

var _log: FileAccess
var _metrics: FileAccess
var _save_path := ""
var _t0 := 0
var _finished := false

# --- Current screen tracking --------------------------------------------
var _scene: Node
var _scene_kind := ""
var _scene_enter_us := 0
var _last_progress_us := 0
var _progress_key := ""
var _next_key_us := 0

# --- Campaign ---------------------------------------------------------------
var _campaigns := 0
var _campaign_start_us := 0
var _episode_seen := 0
var _episode_start_us := 0
var _episodes_done: Array = []
var _shops_in_episode := 0
var _reloaded_episodes := {}
var _shop_handled := false
var _shop_plan: Array = []
var _shop_sub := {}
var _shop_keys := 0
var _levels_played := 0
var _levels_this_campaign := 0
var _shops_total := 0
var _saves_total := 0
var _reloads_ok := 0
var _reloads_failed := 0
var _deaths_total := 0
var _cubes_read := 0
## Data cube already read in the current shop.
var _cube_read_here := false
var _failures: PackedStringArray = []
var _expect_title := false
var _skip_logged := false
## Mini-games (]e[ ENGAGE, ]g[ GALAGA) already played in the current campaign,
## by level name. "** ALE **" (episode 1, section 41) and "SQUADRON"
## (episode 4, section 45) are offered by a "Do you have the
## special ship?" shop next to the normal sequel; with no end event, they
## end at the game over, which returns to that shop (JE_loadGame(11)). The
## bot plays them once (without helpers, see level.gd), then takes the other
## destination.
var _mini_games_played := {}

# --- Level -----------------------------------------------------------------
var _level_tick := 0
var _level_deaths := 0
var _was_alive := true
## Hits taken in the level (shield + armor drop, player()[2..3]) and
## ticks of the latest ones, to tell where a level without helpers is lost.
var _hits := 0
var _hit_ticks: Array = []
var _last_hp := -1
var _level_info := {}
var _held := 0
var _sweep_dir := 1
var _stalled := 0
var _no_damage := 0
var _last_armor_sum := -1
var _sweep_left := 0
var _lock_index := 0
var _lock_slot := -1
var _lock_armor := 0
var _lock_ticks := 0
var _climb_y := 0
var _last_scroll := Vector2i(-1, -1)
## video.h : PLAY_AREA_HEIGHT.
const PLAY_AREA_HEIGHT := 184

# --- Measurements ------------------------------------------------------------------
var _last_frame_us := 0
var _frame_ms: Array[float] = []
var _metrics_next_us := 0


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--campaign-bot="):
			_log_path = arg.trim_prefix("--campaign-bot=")
			active = true
		elif arg.begins_with("--campaign-bot-assist="):
			replay_assist = arg.trim_prefix("--campaign-bot-assist=") != "0"
		elif arg.begins_with("--campaign-bot-speed="):
			_speed = maxi(1, int(arg.trim_prefix("--campaign-bot-speed=")))
		elif arg == "--campaign-bot-realtime":
			_realtime = true
		elif arg == "--campaign-bot-loop":
			_loop = true
		elif arg.begins_with("--campaign-bot-seconds="):
			_seconds = float(arg.trim_prefix("--campaign-bot-seconds="))
		elif arg.begins_with("--campaign-bot-metrics="):
			_metrics_path = arg.trim_prefix("--campaign-bot-metrics=")
		elif arg.begins_with("--campaign-bot-stall="):
			_stall_s = float(arg.trim_prefix("--campaign-bot-stall="))
		elif arg.begins_with("--campaign-bot-max-ticks="):
			_max_ticks = int(arg.trim_prefix("--campaign-bot-max-ticks="))
		elif arg.begins_with("--campaign-bot-start-episode="):
			_start_episode = clampi(int(arg.trim_prefix("--campaign-bot-start-episode=")), 1, 5)
		elif arg.begins_with("--campaign-bot-start-section="):
			_start_section = maxi(1, int(arg.trim_prefix("--campaign-bot-start-section=")))
		elif arg.begins_with("--campaign-bot-buy="):
			_buy_plan = Array(arg.trim_prefix("--campaign-bot-buy=").split(",", false)).map(func(v): return int(v))
		elif arg == "--campaign-bot-dodge":
			_dodge_on = true
		elif arg == "--campaign-bot-drive=dodge":
			_planner = ClassDB.instantiate("TyrianDodgeBot")
		elif arg.begins_with("--campaign-bot-attempts="):
			_max_attempts = maxi(1, int(arg.trim_prefix("--campaign-bot-attempts=")))
		elif arg.begins_with("--campaign-bot-episodes="):
			_episodes_target = clampi(int(arg.trim_prefix("--campaign-bot-episodes=")), 1, 5)
	set_process(false)
	set_physics_process(false)
	if not active:
		# Outside the bot, no helper (level.gd reads replay_assist).
		replay_assist = false
		return
	# The inputs are set before the level scene's _physics_process.
	process_physics_priority = -1000
	process_priority = -1000
	process_mode = Node.PROCESS_MODE_ALWAYS
	_start.call_deferred()


func _start() -> void:
	_log = FileAccess.open(_log_path, FileAccess.WRITE)
	if _log == null:
		printerr("campaign_bot: cannot open log: %s" % _log_path)
		Game.quit_cleanly(1)
		return
	if not _metrics_path.is_empty():
		_metrics = FileAccess.open(_metrics_path, FileAccess.WRITE)
		if _metrics == null:
			printerr("campaign_bot: cannot open metrics: %s" % _metrics_path)
			Game.quit_cleanly(1)
			return
		(
			_metrics
			. store_line(
				# gdlint: ignore=max-line-length
				"t_s,fps,frame_ms_mean,frame_ms_min,frame_ms_max,memory_static,object_count,node_count,orphan_nodes,video_mem,scene,episode,section"
			)
		)
	# The bot's saves are kept separate: the player's are neither read nor written.
	Game.saves_dir = UserData.path("campaign_bot/saves")
	_save_path = ProjectSettings.globalize_path(Game.saves_dir)
	DirAccess.make_dir_recursive_absolute(_save_path)
	for f in DirAccess.get_files_at(_save_path):
		DirAccess.remove_absolute(_save_path.path_join(f))
	if _realtime:
		_speed = 1
	else:
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
		Engine.max_fps = 0
		Engine.physics_ticks_per_second = 35 * _speed
		Engine.max_physics_steps_per_frame = maxi(8, _speed * 2)
	_t0 = Time.get_ticks_usec()
	_last_progress_us = _t0
	_last_frame_us = _t0
	_metrics_next_us = _t0 + 1000000
	_campaign_start_us = _t0
	_emit(
		{
			"event": "start",
			"assist": replay_assist,
			"dodge": _dodge_on,
			"drive": "dodge" if _planner != null else "aim",
			"attempts": _max_attempts,
			"speed": _speed,
			"realtime": _realtime,
			"loop": _loop,
			"seconds": _seconds,
			"physics_tps": Engine.physics_ticks_per_second,
			"save": _save_path,
			"os": OS.get_name(),
			"renderer": RenderingServer.get_current_rendering_driver_name(),
			"data_dir": Game.data_dir,
			"exported": not OS.has_feature("editor")
		}
	)
	print(
		(
			"campaign_bot: start (assists %s, speed ×%d, loop %s, duration %.0f s)"
			% [replay_assist, _speed, _loop, _seconds]
		)
	)
	set_process(true)
	set_physics_process(true)


func _now_s() -> float:
	return (Time.get_ticks_usec() - _t0) / 1e6


func _emit(d: Dictionary) -> void:
	if _log == null or not _log.is_open():
		return
	d["t"] = snappedf(_now_s(), 0.001)
	if not d.has("scene"):
		d["scene"] = _scene_kind
	d["episode"] = Game.episode
	d["section"] = Game.main_level
	d["campaign"] = _campaigns + 1
	_log.store_line(JSON.stringify(d))
	_log.flush()


func _kind(n: Node) -> String:
	if n == null:
		return ""
	var p := n.scene_file_path
	if p.is_empty():
		return n.name
	return p.get_file().get_basename()


# --- Loop -------------------------------------------------------------------


func _process(_delta: float) -> void:
	if _finished:
		return
	var now := Time.get_ticks_usec()
	_frame_ms.append((now - _last_frame_us) / 1000.0)
	_last_frame_us = now
	if _metrics != null and now >= _metrics_next_us:
		_write_metrics()
		_metrics_next_us += 1000000
		if now > _metrics_next_us:
			_metrics_next_us = now + 1000000
	if _seconds > 0 and _now_s() >= _seconds:
		if _loop:
			_finish(_failures.is_empty(), "duration reached (%d complete campaign(s))" % _campaigns)
		else:
			# Requested duration reached = success (as with the loop); only the
			# failures already recorded (blocking screen, impassable level) count.
			_finish(_failures.is_empty(), "duration reached (%.0f s)" % _seconds)
		return
	var cur: Node = Effects.current_game_scene()
	if cur != _scene:
		_on_scene_change(cur)
		if _finished:
			return
	if (now - _last_progress_us) / 1e6 > _stall_s:
		_fail("blocking screen: %s without progress for %.0f s (%s)" % [_scene_kind, _stall_s, _progress_key])
		return
	if now < _next_key_us:
		return
	match _scene_kind:
		"title":
			_drive_title()
		"shop":
			_drive_shop()
		"debrief":
			_drive_debrief()
		"story_screens":
			_drive_story()
		"new_game":
			_drive_new_game()
		"level":
			pass
		"":
			pass
		_:
			_fail("unexpected screen: %s" % _scene_kind)


func _progress(key: String) -> void:
	if key != _progress_key:
		_progress_key = key
		_last_progress_us = Time.get_ticks_usec()


func _on_scene_change(cur: Node) -> void:
	var prev := _scene_kind
	var dur := (Time.get_ticks_usec() - _scene_enter_us) / 1e6
	if prev == "level":
		_level_left()
	_scene = cur
	_scene_kind = _kind(cur)
	_scene_enter_us = Time.get_ticks_usec()
	_progress("scene:%d" % (cur.get_instance_id() if cur else 0))
	_next_key_us = 0
	if cur == null:
		return
	_emit(
		{
			"event": "scene",
			"from": prev,
			"prev_duration_s": snappedf(dur, 0.001),
			"cash": Game.loadout[10] if Game.loadout.size() > 10 else 0
		}
	)
	# Episode transition (JE_nextEpisode): episode changed since the last scene.
	if Game.campaign_active and _episode_seen != 0 and Game.episode != _episode_seen:
		_episode_finished(_episode_seen, Game.episode)
		if _finished or _expect_title:
			return
	if Game.campaign_active and _episode_seen == 0:
		_episode_seen = Game.episode
		_episode_start_us = Time.get_ticks_usec()
	match _scene_kind:
		"level":
			_level_entered()
		"shop":
			_shop_handled = false
			_skip_logged = false
			_shop_plan = []
			_shop_sub = {}
			_shop_keys = 0
			_cube_read_here = false
		"title":
			if _campaigns > 0 or _levels_played > 0:
				# Return to the title outside the campaign end (loop end handled elsewhere).
				if not _expect_title:
					_fail("unexpected return to the title screen")
			_expect_title = false


func _episode_finished(old_ep: int, new_ep: int) -> void:
	var dur := (Time.get_ticks_usec() - _episode_start_us) / 1e6
	_episodes_done.append(old_ep)
	_emit(
		{
			"event": "episode_end",
			"finished_episode": old_ep,
			"next_episode": new_ep,
			"duration_s": snappedf(dur, 0.001),
			"levels": _levels_this_campaign,
			"cash": Game.loadout[10]
		}
	)
	print("campaign_bot: episode %d finished in %.1f s → episode %d" % [old_ep, dur, new_ep])
	_episode_seen = new_ep
	_episode_start_us = Time.get_ticks_usec()
	_shops_in_episode = 0
	if old_ep >= _episodes_target:
		_campaign_complete()


func _campaign_complete() -> void:
	_campaigns += 1
	var dur := (Time.get_ticks_usec() - _campaign_start_us) / 1e6
	_emit(
		{
			"event": "campaign_end",
			"campaigns": _campaigns,
			"duration_s": snappedf(dur, 0.001),
			"levels": _levels_this_campaign,
			"deaths": _deaths_total
		}
	)
	print("campaign_bot: campaign %d finished in %.1f s (%d levels)" % [_campaigns, dur, _levels_this_campaign])
	if _loop and (_seconds <= 0 or _now_s() < _seconds):
		# New campaign through the title screen, like a player.
		_levels_this_campaign = 0
		_campaign_start_us = Time.get_ticks_usec()
		_episode_seen = 0
		_reloaded_episodes.clear()
		_mini_games_played.clear()
		_game_overs.clear()
		Game.campaign_active = false
		_expect_title = true
		Game.goto_scene(Game.SCENE_TITLE)
		return
	_finish(_failures.is_empty(), "campaign finished")


# --- Title ----------------------------------------------------------------------


func _drive_title() -> void:
	# Direct start at an episode: without waiting for the menu. Otherwise, the key
	# that cuts the logos short may land on the already displayed menu and start
	# "New Game" (episode 1) instead (seen in loaded parallel runs).
	if _start_episode > 0 and _campaigns == 0:
		# Direct start at an episode (diagnostic): the game begins like
		# "New Game" but at the requested episode, Normal difficulty.
		_emit({"event": "new_game", "start_episode": _start_episode})
		_scene.set("idle", false)
		_episode_seen = 0
		if _start_section <= 1:
			Game.start_campaign(_start_episode, 2)
		else:
			# Same state as start_campaign, script resumed at the wanted section.
			Game.campaign_active = true
			Game.episode = _start_episode
			Game.initial_difficulty = 2
			Game.difficulty = 2
			Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 10000])
			Game.start_cur_loc = 0
			Game.campaign_carry = PackedInt32Array()
			Game.main_level = _start_section
			Game.advance_campaign()
		return
	# During the logos and animation: a key cuts them short (wait_delayorinput).
	if _scene.get("idle") != true:
		_tap(KEY_SPACE, 0.25)
		return
	var sel: int = _scene.menus.selection() if _scene.get("menus") != null and not _scene.menus.stack.is_empty() else 0
	if sel != 0:
		_tap(KEY_UP, 0.05)
		return
	_emit({"event": "new_game"})
	_tap(KEY_ENTER, 0.5)


# --- New game choices ---------------------------------------------


## new_game.tscn (menus.c:gameplaySelect, episodeSelect, difficultySelect):
## Enter on the default choices = 1 player, full game, episode 1,
## Normal difficulty. One key per screen, once the menu has stopped.
func _drive_new_game() -> void:
	if _scene.get("idle") != true or _scene.get("_busy") == true:
		return
	var screen := str(_scene.get("current_screen"))
	_progress("new_game:" + screen)
	_emit({"event": "new_game_choice", "screen": screen})
	_tap(KEY_ENTER, 0.5)


# --- Debriefing ---------------------------------------------------------------


func _drive_debrief() -> void:
	# A key right on entry speeds up the animation (frameCountMax = 0), another
	# at "Press a key" leaves it; in real time, we wait for the screen.
	if _scene.get("stats_shown") == true:
		# Statistics screen (mainint.c): Confirm closes it.
		_tap(KEY_ENTER, 0.5)
		_progress("debrief:stats")
	elif _scene.get("idle") == true:
		_tap(KEY_SPACE, 0.5)
		_progress("debrief:key")
	elif not _realtime:
		_tap(KEY_SPACE, 0.3)


# --- Story pages, episode end, high score entry -----------------


func _drive_story() -> void:
	if _scene.get("name_entry") == true:
		_tap(KEY_ENTER, 0.5)
		_progress("story:name")
	elif _scene.get("idle") == true:
		_tap(KEY_SPACE, 0.5)
		_progress("story:key")
	elif not _realtime:
		_tap(KEY_SPACE, 0.3)


# --- Shop -------------------------------------------------------------------


func _drive_shop() -> void:
	var shop: Node = _scene
	var st: Dictionary = shop.get("_state")
	if shop.get("_shop") == null or st.is_empty():
		_fail("shop without a TyrianShop session (campaign inactive?)")
		return
	if not _shop_handled:
		_shop_handled = true
		_shops_total += 1
		_shops_in_episode += 1
		_emit(
			{
				"event": "shop",
				"shop_section": Game.shop_section,
				"cash": int(st.get("cash", 0)),
				"loadout": Array(Game.loadout),
				"destinations": Array(Game.map_sections)
			}
		)
		if not _save_and_maybe_reload():
			return
		# Categories bought: rear gun (side fire), generator,
		# shield. The front gun stays the starting Pulse-Cannon: its
		# fan, at the helpers' power 11, reaches the turrets on the
		# edges; the Mega Cannon (narrow beam, the most expensive purchase
		# of episode 1) left SURFACE (episode 4) frozen indefinitely.
		_shop_plan = _buy_plan.duplicate()
		_shop_sub = {}
		if _realtime:
			_next_key_us = Time.get_ticks_usec() + 400000
			return
	_progress("shop:%d:%d:%d:%d" % [st["menu"], st["sel"], st.get("category", 0), _shop_keys])
	if _shop_keys > 400:
		_fail("shop: more than 400 keys without leaving (menu %d sel %d)" % [st["menu"], st["sel"]])
		return
	var menu := int(st["menu"])
	var sel := int(st["sel"])
	match menu:
		0:  # MENU_FULL_GAME : 2 = Data, 4 = Upgrade Ship, 6 = Play Next Level.
			var want := 4 if not _shop_plan.is_empty() else 6
			if int(st.get("cube_max", 0)) > 0 and not _cube_read_here:
				want = 2
			_nav_to(sel, want)
		1:  # MENU_UPGRADES: 2..8 categories, 9 = Done.
			var want: int = _shop_plan[0] if not _shop_plan.is_empty() else 9
			_nav_to(sel, want)
			if sel == want and not _shop_plan.is_empty():
				_shop_sub = {"cat": want, "target": -1, "chosen": false}
		3:  # MENU_PLAY_NEXT_LEVEL: 2 = first destination.
			var choice := _destination_choice()
			if sel == 2 + choice:
				_emit(
					{
						"event": "shop_leave",
						"choice": choice,
						"destination": Game.map_sections[choice] if choice < Game.map_sections.size() else -1,
						"cash": int(st.get("cash", 0)),
						"loadout": Array(st.get("loadout", PackedInt32Array()))
					}
				)
			_nav_to(sel, 2 + choice)
		4:
			_drive_category(st)
		7:  # MENU_DATA_CUBES ("Data"): the bot reads the first cube once per
			# shop (Enter -> MENU_DATA_CUBE_SUB), then Escape -> Game Menu. An
			# extra Enter key on "Data" also leads there: Escape leaves it.
			if not _cube_read_here and int(st.get("cube_max", 0)) > 0:
				_cube_read_here = true
				_cubes_read += 1
				var titles: Array = Array(st.get("cube_titles", []))
				_emit(
					{
						"event": "data_cube",
						"cube_max": int(st.get("cube_max", 0)),
						"title": titles[0] if not titles.is_empty() else ""
					}
				)
				_nav_to(sel, 2)
			else:
				_key(KEY_ESCAPE)
		8:  # MENU_DATA_CUBE_SUB: back to the cube list.
			_key(KEY_ESCAPE)


## Chosen destination (index in Game.map_sections): the first one, unless
## it leads to a mini-game already played in this campaign; examined by
## TyrianCampaign.run_script, with no effect on the game.
func _destination_choice() -> int:
	var c: RefCounted = Game.campaign()
	for i in Game.map_sections.size():
		var r: Dictionary = c.run_script(
			Game.episode, Game.map_sections[i], Game.initial_difficulty, Game.loadout[0], true
		)
		var name := str(r.get("level_name", "")).strip_edges()
		if r.has("mini_game") and _mini_games_played.has(name):
			continue
		if i > 0 and not _skip_logged:
			_skip_logged = true
			_emit(
				{
					"event": "skip_destination",
					"skipped": Array(Game.map_sections.slice(0, i)),
					"chosen": Game.map_sections[i]
				}
			)
		return i
	return 0


## Arrows up to `want`, then Enter.
func _nav_to(sel: int, want: int) -> void:
	if sel < want:
		_key(KEY_DOWN)
	elif sel > want:
		_key(KEY_UP)
	else:
		_key(KEY_ENTER)


## Open category: the most expensive item the budget allows (two
## thirds of the money for the front gun, the rest afterwards), then "Done".
## Share of the money (in %, on entering the category) kept after buying
## power: front gun 40%, rear gun 60%.
const POWER_KEEP := {3: 40, 4: 60}


func _drive_category(st: Dictionary) -> void:
	var sel := int(st["sel"])
	var choices := int(st["menu_choices"])
	var items: PackedInt32Array = st["items"]
	var costs: PackedInt32Array = st["costs"]
	var cash := int(st["cash"])
	if int(_shop_sub.get("target", -1)) < 0:
		var cat := int(st["category"])
		# Front or rear gun bought first: two thirds of the money.
		var budget := cash * 2 / 3 if cat in [3, 4] and _shop_plan.size() > 2 else cash
		var owned := int(st["owned_item"])
		var owned_cost := 0
		for i in items.size():
			if items[i] == owned:
				owned_cost = costs[i]
		var best := choices  # Done
		var best_cost := owned_cost
		for i in items.size():
			if costs[i] <= budget and costs[i] > best_cost and items[i] != 0:
				best = i + 2
				best_cost = costs[i]
		_shop_sub["target"] = best
		_shop_sub["item"] = items[best - 2] if best < choices else owned
		_shop_sub["tries"] = 0
	var target := int(_shop_sub["target"])
	_shop_sub["tries"] = int(_shop_sub["tries"]) + 1
	if int(_shop_sub["tries"]) > 40:
		target = choices  # unreachable item (skipping items that are too expensive): Done
		_shop_sub["target"] = choices
		_shop_sub["chosen"] = true
	if not bool(_shop_sub.get("chosen", false)) and target < choices:
		if sel == target:
			_key(KEY_ENTER)  # shop_choose_row
			_shop_sub["chosen"] = true
			_shop_sub["floor"] = (
				cash * POWER_KEEP[int(st["category"])] / 100 if POWER_KEEP.has(int(st["category"])) else -1
			)
		else:
			_key(KEY_DOWN if sel < target else KEY_UP)
		return
	# Chosen gun: power bought through Right (shop_key Right), like a
	# player, as long as it is allowed, affordable and the remaining money
	# exceeds the category reserve (POWER_KEEP).
	if (
		sel == target
		and target < choices
		and int(_shop_sub.get("floor", -1)) >= 0
		and bool(st.get("right_power", false))
		and bool(st.get("right_power_afford", false))
		and int(st.get("cash_left", 0)) - int(st.get("upgrade_cost", 0)) >= int(_shop_sub["floor"])
	):
		_emit(
			{
				"event": "shop_power",
				"category": int(st["category"]),
				"item": int(_shop_sub.get("item", 0)),
				"cost": int(st.get("upgrade_cost", 0)),
				"cash_left": int(st.get("cash_left", 0))
			}
		)
		_key(KEY_RIGHT)
		return
	# Towards "Done" (last row) then Enter: shop_commit_done.
	if sel != choices:
		_key(KEY_UP if sel == 2 else KEY_DOWN)
		return
	_emit(
		{
			"event": "shop_buy",
			"category": int(st["category"]),
			"item": int(_shop_sub.get("item", 0)),
			"cash_before": cash
		}
	)
	_key(KEY_ENTER)
	_shop_plan.pop_front()
	_shop_sub = {}


## Game save (JE_saveGame) at the shop; at the second
## shop of each episode, reload and check of the resume.
## false if the screen changed (reload) or on failure.
func _save_and_maybe_reload() -> bool:
	var c: RefCounted = Game.campaign()
	var slot := clampi(Game.episode, 1, Game.SAVE_SLOTS - 1)
	var ok: bool = Game.save_campaign(slot, "BOT E%d" % Game.episode)
	_saves_total += 1
	var file := _save_path.path_join("slot_%02d.json" % slot)
	_emit(
		{
			"event": "save",
			"slot": slot,
			"ok": ok,
			"file": file,
			"main_level": Game.main_level,
			"level_name": Game.level_name,
			"error": "" if ok else str(c.last_error)
		}
	)
	if not ok:
		_fail("cannot save: %s" % c.last_error)
		return false
	if _shops_in_episode != 2 or _reloaded_episodes.has(Game.episode):
		return true
	_reloaded_episodes[Game.episode] = true
	var slots: Array = Game.campaign_slots()
	var s = slots[slot - 1] if slot - 1 < slots.size() else null
	if s == null or str(s.get("error", "")) != "":
		_reloads_failed += 1
		_fail("reload: slot %d unreadable (%s)" % [slot, "missing" if s == null else s["error"]])
		return false
	var before := {
		"episode": Game.episode,
		"section": Game.shop_section,
		"loadout": Game.loadout.duplicate(),
		"difficulty": Game.difficulty
	}
	# "Lost" game (return to the title without quitting the program) then
	# Load Game: same path as load_screen.gd -> Game.load_campaign_slot.
	Game.campaign_active = false
	Game.loadout = PackedInt32Array(Game.NEW_GAME_LOADOUT)
	Game.episode = 1
	Game.main_level = 0
	Game.difficulty = 0
	Game.load_campaign_slot(s)
	var after: Node = Effects.current_game_scene()
	var ok2: bool = (
		after != null
		and _kind(after) == "shop"
		and Game.episode == before["episode"]
		and Game.shop_section == before["section"]
		and Game.loadout == before["loadout"]
		and Game.difficulty == before["difficulty"]
	)
	_emit(
		{
			"event": "reload",
			"slot": slot,
			"ok": ok2,
			"file": file,
			"scene_after": _kind(after),
			"episode_after": Game.episode,
			"section_before": before["section"],
			"section_after": Game.shop_section,
			"loadout_before": Array(before["loadout"]),
			"loadout_after": Array(Game.loadout),
			"difficulty_before": before["difficulty"],
			"difficulty_after": Game.difficulty
		}
	)
	if not ok2:
		_reloads_failed += 1
		_fail(
			"reload: incorrect resume (section %d → %d, ep. %d)" % [before["section"], Game.shop_section, Game.episode]
		)
		return false
	_reloads_ok += 1
	print("campaign_bot: reload ep. %d section %d OK (%s)" % [Game.episode, Game.shop_section, file])
	# The new shop scene will be handled (without a new save
	# counted as a shop) on the next turn.
	_shops_in_episode += 1
	_shops_total -= 1
	return false


# --- Level -----------------------------------------------------------------------


func _level_entered() -> void:
	_level_tick = 0
	_level_deaths = 0
	_was_alive = true
	_hits = 0
	_hit_ticks = []
	_last_hp = -1
	_stalled = 0
	_no_damage = 0
	_sweep_left = 0
	_last_armor_sum = -1
	_last_scroll = Vector2i(-1, -1)
	if _planner != null:
		# Attempt no. (game overs already suffered on this level): other setting.
		_planner.set_attempt(int(_game_overs.get("%d:%s" % [Game.episode, Game.level_name.strip_edges()], 0)))
	if Game.mini_game:
		_mini_games_played[Game.level_name.strip_edges()] = true
		_emit({"event": "mini_game", "level": Game.level_name, "modes": Array(Game.modes)})
	_level_info = {
		"lvl_file_num": Game.lvl_file_num,
		"level_name": Game.level_name,
		"start_us": Time.get_ticks_usec(),
		"mini_game": Game.mini_game,
		"bonus": Game.bonus_level_current,
		"assist": _scene.get("assist"),
		"interactive": _scene.get("interactive")
	}
	_emit(
		{
			"event": "level_start",
			"level": Game.level_name,
			"lvl_file_num": Game.lvl_file_num,
			"difficulty": Game.difficulty,
			"assist": _scene.get("assist"),
			"interactive": _scene.get("interactive"),
			"loadout": Array(Game.loadout)
		}
	)
	if _scene.get("interactive") != true:
		_fail("non-interactive level")


## Level end recorded (level_end): ticks, money, enemies, cubes, deaths,
## and the end equipment (Game.loadout, front gun [2] and rear
## [4] power included) for the per-level readouts of play without helpers.
func _level_left() -> void:
	_release_all()
	var dur := (Time.get_ticks_usec() - int(_level_info.get("start_us", Time.get_ticks_usec()))) / 1e6
	var d: Dictionary = Game.debrief
	var completed := _kind(Effects.current_game_scene()) == "debrief"
	if completed:
		_levels_played += 1
		_levels_this_campaign += 1
	if not bool(d.get("all_players_alive", true)):
		_level_deaths = maxi(_level_deaths, 1)
	_deaths_total += _level_deaths
	_emit(
		{
			"event": "level_end",
			"scene": "level",
			"completed": completed,
			"level": _level_info.get("level_name", ""),
			"lvl_file_num": _level_info.get("lvl_file_num", 0),
			"tick": _level_tick,
			"duration_s": snappedf(dur, 0.001),
			"cash": int(d.get("cash", 0)),
			"enemy_killed": int(d.get("enemy_killed", 0)),
			"total_enemy": int(d.get("total_enemy", 0)),
			"cube_max": int(d.get("cube_max", 0)),
			"all_players_alive": d.get("all_players_alive", true),
			"deaths": _level_deaths,
			"hits": _hits,
			"last_hit_ticks": _hit_ticks.duplicate(),
			"harmless_known": _planner.harmless_known() if _planner != null else 0,
			"difficulty_after": Game.difficulty,
			"loadout": Array(Game.loadout),
			"front_power": Game.loadout[2] if Game.loadout.size() > 2 else 0,
			"rear_power": Game.loadout[4] if Game.loadout.size() > 4 else 0
		}
	)
	# Death in a bonus level (]L "$"): the campaign continues, no game over.
	if (
		not completed
		and not bool(_level_info.get("mini_game", false))
		and not bool(_level_info.get("bonus", false))
		and Game.campaign_active
	):
		# Game over (tyrian2.c: JE_loadGame(11)): the level restarts with
		# the previous equipment ("LAST LEVEL" save).
		var key := "%d:%s" % [Game.episode, str(_level_info.get("level_name", "")).strip_edges()]
		_game_overs[key] = int(_game_overs.get(key, 0)) + 1
		_emit(
			{
				"event": "game_over",
				"level": _level_info.get("level_name", ""),
				"attempt": _game_overs[key],
				"tick": _level_tick,
				"restart_section": Game.main_level,
				"cash": Game.loadout[10]
			}
		)
		if _game_overs[key] >= _max_attempts:
			_fail.call_deferred("level %s impassable: %d game over" % [key, _game_overs[key]])
	print(
		(
			"campaign_bot: ep. %d %s (lvl %d) %s: %d ticks, %.1f s, cash %d, deaths %d, power %d/%d"
			% [
				Game.episode,
				_level_info.get("level_name", ""),
				_level_info.get("lvl_file_num", 0),
				"finished" if completed else "lost",
				_level_tick,
				dur,
				int(d.get("cash", 0)),
				_level_deaths,
				Game.loadout[2] if Game.loadout.size() > 2 else 0,
				Game.loadout[4] if Game.loadout.size() > 4 else 0
			]
		)
	)


func _physics_process(_delta: float) -> void:
	if _finished:
		return
	# Sped up, several hundred ticks pass between two _process calls: the
	# level is spotted here from its first tick, otherwise the ship would stay
	# without a pilot at the start of the level (ASTEROID1: death at tick 200 at x100).
	var cur: Node = Effects.current_game_scene()
	if cur != _scene and _kind(cur) == "level":
		_on_scene_change(cur)
	if _finished or _scene_kind != "level" or not is_instance_valid(_scene):
		return
	var sim: Node = _scene.get("sim")
	if sim == null:
		_fail("level without TyrianSim")
		return
	var t: int = sim.tick()
	if t != _level_tick:
		_level_tick = t
		_progress("tick:%d" % t)
	if t > _max_ticks:
		_fail("level %s: more than %d ticks without ending" % [Game.level_name, _max_ticks])
		return
	var p: PackedInt32Array = sim.player(0)
	if p.size() >= 8:
		var alive := p[4] != 0
		if _was_alive and not alive:
			_level_deaths += 1
			_emit({"event": "death", "level": Game.level_name, "tick": t})
		_was_alive = alive
		var hp := p[2] + p[3]
		if _last_hp >= 0 and hp < _last_hp - 1 and alive:
			_hits += 1
			_emit_hit(sim, p, t, _last_hp - hp)
			_hit_ticks.append(t)
			if _hit_ticks.size() > 8:
				_hit_ticks.pop_front()
		_last_hp = hp
	_set_mask(_planner.next_mask(sim) if _planner != null else _choose_mask(sim, p, t))


## Hit taken: distance (center to center, px, Manhattan) to the nearest enemy
## shot and enemy, to tell collision from shot.
func _emit_hit(sim: Node, p: PackedInt32Array, t: int, damage: int) -> void:
	var cx := p[0] + 7
	var cy := p[1] + 7
	var near_shot := 9999
	var shots: PackedInt32Array = sim.enemy_shots()
	for i in range(0, shots.size() - 2, 3):
		near_shot = mini(near_shot, absi(shots[i] + 6 - cx) + absi(shots[i + 1] + 7 - cy))
	var near_enemy := 9999
	var e: PackedInt32Array = sim.enemies()
	for i in range(0, e.size() - 7, 8):
		var big := e[i + 5] >= 2
		near_enemy = mini(
			near_enemy, absi(e[i + 1] + (12 if big else 6) - cx) + absi(e[i + 2] + (14 if big else 7) - cy)
		)
	_emit(
		{
			"event": "hit",
			"level": Game.level_name,
			"tick": t,
			"damage": damage,
			"x": p[0],
			"y": p[1],
			"near_shot": near_shot,
			"near_enemy": near_enemy,
			"armor": p[2],
			"shield": p[3]
		}
	)


## Constant fire and movement, inspired by autopilot.c:autopilot_drive
## (reference C build pilot):
## - column of the nearest DESTRUCTIBLE enemy (0 < armor < 255)
##   above the ship (Manhattan distance); the C pilot also aims at
##   indestructible scenery (armor 255), which freezes it in front of them;
## - map frozen for 200 ticks (level lock, boss): the sturdiest
##   destructible enemy above (the boss or the lock), and, if the
##   sum of armors has not dropped for 400 ticks, 300 ticks of
##   sweeping the whole width [40, 256] like the C pilot;
## - lower third of the play area.
## Unlike the C pilot, the bot NEVER finishes a level by force.
func _choose_mask(sim: Node, p: PackedInt32Array, _t: int) -> int:
	var mask := Game.MASK_FIRE
	if p.size() < 2:
		return mask
	var px := p[0]
	var py := p[1]
	var e: PackedInt32Array = sim.enemies()
	var best := 1 << 30
	var strongest := -1
	var target_x := px
	var boss_x := px
	var have := false
	var armor_sum := 0
	for i in range(0, e.size() - 7, 8):
		var armor := e[i + 7]
		if armor <= 0 or armor >= 255:
			continue
		armor_sum += armor
		var dy := py - e[i + 2]
		if dy < 0 or dy > PLAY_AREA_HEIGHT:
			continue
		var dist := dy + absi(px - e[i + 1])
		if dist < best:
			best = dist
			target_x = e[i + 1]
			have = true
		if armor > strongest:
			strongest = armor
			boss_x = e[i + 1]
	# Frozen map detector: layer 0 scrolling (backMove in C).
	var scroll: Vector2i = sim.layer_scroll(0)
	if scroll != _last_scroll:
		_last_scroll = scroll
		_stalled = 0
	else:
		_stalled += 1
	if armor_sum != _last_armor_sum:
		_last_armor_sum = armor_sum
		_no_damage = 0
	else:
		_no_damage += 1
	_climb_y = 0
	if _stalled > 200:
		if _no_damage > 400 and _sweep_left <= 0:
			_sweep_left = 300
			_no_damage = 0
		if _sweep_left > 0 or strongest < 0:
			_sweep_left -= 1
			if px <= 48:
				_sweep_dir = 1
			elif px >= 248:
				_sweep_dir = -1
			target_x = px + _sweep_dir * 64
		else:
			# Destructible targets above, from the sturdiest to the least
			# sturdy; move on to the next when the aimed one has lost
			# no armor in 250 ticks (shots absorbed by indestructible
			# scenery, armor 255, placed in front of it).
			var cands: Array = []
			for i in range(0, e.size() - 7, 8):
				if e[i + 7] > 0 and e[i + 7] < 255 and e[i + 2] < py:
					cands.append([e[i + 7], e[i], e[i + 1] + (12 if e[i + 5] >= 2 else 6), e[i + 2]])
			cands.sort_custom(func(u, v): return u[0] > v[0] or (u[0] == v[0] and u[1] < v[1]))
			if cands.is_empty():
				target_x = boss_x
			else:
				var c: Array = cands[_lock_index % cands.size()]
				if c[1] != _lock_slot or c[0] < _lock_armor:
					if c[1] != _lock_slot:
						_lock_slot = c[1]
					_lock_armor = c[0]
					_lock_ticks = 0
				else:
					_lock_ticks += 1
					if _lock_ticks > 250:
						_lock_index += 1
						_lock_ticks = 0
				target_x = c[2]
				# Indestructible scenery (armor 255) between the ship and the
				# target (shot_update.c collision: +-25 px for a large
				# sprite, +-13 otherwise): the ship climbs just under the target
				# to fire over it (DREAD-NOT, episode 4: turrets
				# behind the battleship's hull).
				for i in range(0, e.size() - 7, 8):
					if (
						e[i + 7] >= 255
						and e[i + 2] > c[3]
						and e[i + 2] < py
						and absi(e[i + 1] - c[2]) < (25 if e[i + 5] >= 2 else 13)
					):
						_climb_y = maxi(c[3] + 40, 24)
						break
		have = true
	else:
		_sweep_left = 0
	var dodge := _dodge(sim, e, px, py) if _dodge_on else 0
	if dodge != 0:
		target_x = px + dodge
		have = true
	if have and target_x < px - 3:
		mask |= Game.MASK_LEFT
	elif have and target_x > px + 3:
		mask |= Game.MASK_RIGHT
	if _climb_y > 0:
		if py > _climb_y + 3:
			mask |= Game.MASK_UP
		elif py < _climb_y - 3:
			mask |= Game.MASK_DOWN
	elif py > PLAY_AREA_HEIGHT - 30:
		mask |= Game.MASK_UP
	elif py < PLAY_AREA_HEIGHT - 60:
		mask |= Game.MASK_DOWN
	return mask


## --campaign-bot-dodge (designed for play without helpers): lateral dodging.
## Threats = enemy shots and enemies (destructible or not) whose box
## crosses the ship's column in the band running from DODGE_AHEAD px
## above it to DODGE_BELOW px below it. Ship: 24 x 28 px drawn at
## (x - 5, y - 7) (level.gd:_draw_player), center (x + 7, y + 7); enemy:
## 12 x 14 px, 24 x 28 px if large (level.gd:_update_enemies_from);
## shot: about 12 x 14 px. If the column is threatened, move to the
## least threatened neighboring column (+-DODGE_STEP). Only reads what a player
## sees on screen (presented positions), only acts through directions.
const DODGE_AHEAD := 72
const DODGE_BELOW := 16
const DODGE_STEP := 28
const SHIP_HALF_W := 10


## Points (center x, center y, half-width) of the visible threats.
func _threats(sim: Node, e: PackedInt32Array) -> Array:
	var out: Array = []
	var shots: PackedInt32Array = sim.enemy_shots()
	for i in range(0, shots.size() - 2, 3):
		out.append(Vector3i(shots[i] + 6, shots[i + 1] + 7, 6))
	for i in range(0, e.size() - 7, 8):
		if e[i + 7] <= 0:
			continue
		var big := e[i + 5] >= 2
		out.append(Vector3i(e[i + 1] + (12 if big else 6), e[i + 2] + (14 if big else 7), 12 if big else 6))
	return out


func _threat(points: Array, cx: int, cy: int) -> int:
	var n := 0
	for q in points:
		if absi(q.x - cx) <= q.z + SHIP_HALF_W and q.y <= cy + DODGE_BELOW and q.y >= cy - DODGE_AHEAD:
			# Closer = more pressing.
			n += 1 + (2 if q.y >= cy - DODGE_AHEAD / 3 else 0)
	return n


func _dodge(sim: Node, e: PackedInt32Array, px: int, py: int) -> int:
	var points := _threats(sim, e)
	var cx := px + 7
	var cy := py + 7
	if _threat(points, cx, cy) == 0:
		return 0
	var best := 0
	var best_n := 1 << 20
	for step in [-DODGE_STEP, DODGE_STEP, -2 * DODGE_STEP, 2 * DODGE_STEP]:
		if px + step < 40 or px + step > 256:
			continue
		var n := _threat(points, cx + step, cy)
		if n < best_n:
			best_n = n
			best = step
	return best


const MASK_ACTIONS := [
	[Game.MASK_UP, "tyrian_up"],
	[Game.MASK_DOWN, "tyrian_down"],
	[Game.MASK_LEFT, "tyrian_left"],
	[Game.MASK_RIGHT, "tyrian_right"],
	[Game.MASK_FIRE, "tyrian_fire"],
	[Game.MASK_CHANGE_FIRE, "tyrian_change_fire"],
	[Game.MASK_LEFT_SIDEKICK, "tyrian_left_sidekick"],
	[Game.MASK_RIGHT_SIDEKICK, "tyrian_right_sidekick"],
]


## Game inputs through player 1's InputMap actions (read by
## Game.read_input_mask in level.gd:_physics_process).
func _set_mask(mask: int) -> void:
	for a in MASK_ACTIONS:
		var on: bool = (mask & a[0]) != 0
		if on != ((_held & a[0]) != 0):
			if on:
				Input.action_press(a[1])
			else:
				Input.action_release(a[1])
	_held = mask


func _release_all() -> void:
	_set_mask(0)


# --- Menu keys ------------------------------------------------------------


## Real keyboard event (pressed then released) routed by Effects to the screen.
func _tap(code: int, wait_s: float) -> void:
	var ev := InputEventKey.new()
	ev.physical_keycode = code as Key
	ev.keycode = code as Key
	ev.pressed = true
	Input.parse_input_event(ev)
	var up := ev.duplicate() as InputEventKey
	up.pressed = false
	Input.parse_input_event(up)
	_next_key_us = Time.get_ticks_usec() + int(wait_s * 1e6 / (1.0 if _realtime else 4.0))


func _key(code: int) -> void:
	_shop_keys += 1
	_tap(code, 0.15 if _realtime else 0.0)


# --- Measurements and end ---------------------------------------------------------------


func _write_metrics() -> void:
	var mean := 0.0
	var mn := INF
	var mx := 0.0
	for v in _frame_ms:
		mean += v
		mn = minf(mn, v)
		mx = maxf(mx, v)
	if not _frame_ms.is_empty():
		mean /= _frame_ms.size()
	else:
		mn = 0.0
	_frame_ms.clear()
	_metrics.store_line(
		(
			"%.3f,%.1f,%.3f,%.3f,%.3f,%d,%d,%d,%d,%d,%s,%d,%d"
			% [
				_now_s(),
				Engine.get_frames_per_second(),
				mean,
				mn,
				mx,
				int(Performance.get_monitor(Performance.MEMORY_STATIC)),
				int(Performance.get_monitor(Performance.OBJECT_COUNT)),
				int(Performance.get_monitor(Performance.OBJECT_NODE_COUNT)),
				int(Performance.get_monitor(Performance.OBJECT_ORPHAN_NODE_COUNT)),
				int(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED)),
				_scene_kind,
				Game.episode,
				Game.main_level
			]
		)
	)
	_metrics.flush()


func _fail(why: String) -> void:
	if _finished:
		return
	_failures.append(why)
	var extra := {}
	if is_instance_valid(_scene):
		for k in ["idle", "_busy", "_state", "interactive"]:
			var v = _scene.get(k)
			if v != null:
				extra[k] = str(v)
		if _scene_kind == "level" and _scene.get("sim") != null:
			extra["tick"] = _scene.sim.tick()
			extra["cur_loc"] = _scene.sim.cur_loc()
			extra["level_ended"] = _scene.sim.level_ended()
			extra["player"] = Array(_scene.sim.player(0))
			extra["enemies"] = Array(_scene.sim.enemies())
			extra["scroll"] = [_scene.sim.layer_scroll(0), _scene.sim.layer_scroll(1), _scene.sim.layer_scroll(2)]
	# Image of the blocking screen next to the log (unavailable in --headless).
	var shot := _log_path.get_basename() + "-failure.png"
	var img: Image = Effects.game_viewport.get_texture().get_image() if DisplayServer.get_name() != "headless" else null
	if img != null and img.save_png(shot) == OK:
		extra["screenshot"] = shot
	_emit(
		{"event": "failure", "reason": why, "state": extra, "level": Game.level_name, "lvl_file_num": Game.lvl_file_num}
	)
	printerr("campaign_bot: FAILED: %s" % why)
	_finish(false, why)


func _finish(ok: bool, why: String) -> void:
	if _finished:
		return
	_finished = true
	_release_all()
	var summary := {
		"event": "summary",
		"ok": ok,
		"reason": why,
		"campaigns_completed": _campaigns,
		"episodes_completed": _episodes_done,
		"levels_played": _levels_played,
		"shops": _shops_total,
		"saves": _saves_total,
		"cubes_read": _cubes_read,
		"reloads_ok": _reloads_ok,
		"reloads_failed": _reloads_failed,
		"deaths": _deaths_total,
		"failures": Array(_failures),
		"game_overs": _game_overs,
		"duration_s": snappedf(_now_s(), 0.001),
		"assist": replay_assist,
		"dodge": _dodge_on,
		"drive": "dodge" if _planner != null else "aim",
		"speed": _speed,
		"realtime": _realtime
	}
	_emit(summary)
	if _metrics != null:
		if not _frame_ms.is_empty():
			_write_metrics()
		_metrics.close()
	_log.close()
	print(
		(
			"campaign_bot: %s — %s; episodes %s, %d levels, %d shops, %d saves, %d reload(s) OK, %.1f s"
			% [
				"OK" if ok else "FAILED",
				why,
				_episodes_done,
				_levels_played,
				_shops_total,
				_saves_total,
				_reloads_ok,
				_now_s()
			]
		)
	)
	Game.quit_cleanly(0 if ok else 1)
