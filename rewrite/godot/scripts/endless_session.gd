## Endless game in progress (src/endless.c, ADR 0071): static state, it
## survives the reloading of the level scene between two waves.
##
## The logic (level rotation, armor, lives, score) is in tyrian-sim
## (`endless.rs`, respawn in `player.rs`, score in `collision.rs`),
## exposed by the GDExtension object `TyrianEndless` and `TyrianSim.endless_state`.
class_name EndlessSession
extends RefCounted

const SCENE_END := "res://scenes/endless_end.tscn"
const SCENE_PROMPT := "res://scenes/endless_prompt.tscn"
## Combos (combos.c:combo_update): duration of a game loop in ms (35 Hz
## of the rewrite, ADR 0003; the C measures SDL_GetTicks, ~28 ms).
const COMBO_FRAME_MS := 28
## endless.c:ENDLESS_INTRO_MS ("WAVE n / ENTERING name" banner).
const INTRO_MS := 2000
## endless.c:ENDLESS_START_LIVES.
const START_LIVES := 3
## endless.h:ENDLESS_LEADERBOARD_SIZE.
const LEADERBOARD_SIZE := 10
## Leaderboard and resume: JSON files in the UserData folder (the C writes the ELBD/ENDR sections
## of opentyrian.dat, see ADR 0071).
static var BOARD_PATH: String = UserData.path("endless_leaderboard.json")
static var RESUME_PATH: String = UserData.path("endless_run.json")

static var run: RefCounted = null
## True between the start (or resume) of a run and its first level: the level then calls
## ach_run_start (endless.c:886 and 1822, stats_run_start).
static var run_start_pending := false
## Rotation seed and number of levels drawn (resume: the same sequence is
## redrawn; the C, for its part, saves the next ep/lvl in endless_run_save).
static var run_seed := 0
static var picks := 0
## End of game waiting for the screens (final score, wave reached).
static var final_score := 0
static var final_wave := 0
static var final_base := 0
static var final_deck := PackedInt32Array()
static var final_two_player := false
## "Anomaly bonus" percentage of the summary (endless_math.c:endless_summary_bonus_pct).
static var final_bonus_pct := 0
## Run score as the C holds it when quitting via the pause menu: takes into
## account the current wave (ENH-01).
## Player 2 equipment, carried over from wave to wave (ENH-03).
static var loadout2 := PackedInt32Array()
## State carried over from the previous wave (TyrianSim.campaign_carry), empty on the
## first wave: player frictions and held special fire, which the C does not
## reset between two waves.
static var carry := PackedInt32Array()
## First anomaly draw before wave 1 (endless_start): no ENDR save.
static var _first_pick := false
## Two-player game (endless.c:s_last_two_player).
static var two_player := false
## Menu mutators (ModeSetup.mutators), re-read by the score multiplier.
static var mutators := 0
## Choice screen (endless_prompt.gd): "resume" or "pick", cards offered.
static var prompt_mode := ""
static var prompt_picks := PackedInt32Array()
static var _pending_two_player := false
## anomalies.c:anomalies_enabled: false by default (dormant system in 1.0,
## gameplay_menu.c, save_json.c); --endless-anomalies turns it on.
static var anomalies_enabled := OS.get_cmdline_user_args().has("--endless-anomalies")


static func active() -> bool:
	return run != null


## endless_start (one player) then first level. Seed: Game.rng_seed
## (the C takes time(NULL); the rewrite keeps a game replayable).
## `goto_level` false: only prepares Game (capture tool, which loads
## the level scene itself).
static func start(goto_level := true, two_p := false) -> bool:
	if not ClassDB.class_exists("TyrianEndless"):
		push_error("GDExtension tyrian-godot missing: Endless mode unavailable")
		return false
	# endless.c:786-791 (endless_start): modes of the previous game reset to zero;
	# otherwise a previous Timed Battle gave "Lives Bonus" in Endless.
	Game.reset_game_modes()
	run_start_pending = true
	run = ClassDB.instantiate("TyrianEndless")
	run_seed = Game.rng_seed
	picks = 0
	carry = PackedInt32Array()
	two_player = two_p
	# anomaly_deck_init(SDL_GetTicks()): deck seed drawn from the game seed.
	run.start_run(Game.data_dir, run_seed, 0, two_p, run_seed ^ 0x5eed)
	_apply_loadout()
	# endless.c:endless_start: "first-wave pick" before the first level, if
	# anomalies are active and the deck is not full (ENH-08).
	if goto_level and anomalies_enabled:
		var cards: PackedInt32Array = run.pick_three(1)
		if not cards.is_empty():
			_first_pick = true
			prompt_mode = "pick"
			prompt_picks = cards
			Game.goto_scene(SCENE_PROMPT)
			return true
	return _goto_next(goto_level)


## endless_start: starting equipment (one player, or two, ship_for[]).
static func _apply_loadout() -> void:
	loadout2 = PackedInt32Array()
	if two_player:
		var both: PackedInt32Array = run.start_loadouts_2p()
		Game.loadout = both.slice(0, 11)
		loadout2 = both.slice(11, 22) if both.size() >= 22 else PackedInt32Array()
	else:
		Game.loadout = run.start_loadout()


## menus.c (MENU_ITEM_1/2_PLAYER_ENDLESS): "endless_resume_prompt() &&
## endless_resume()" then endless_start(two_p). Choice screen only if
## a game is saved.
static func enter(two_p := false) -> bool:
	if not ClassDB.class_exists("TyrianEndless"):
		return false
	# endless.c:endless_start: "rng_seed((uint64_t)time(NULL))", each game
	# started by the player draws a different level sequence; "--seed=N" forces it.
	if not Game.seed_forced:
		Game.rng_seed = player_seed()
	if resume_info().is_empty():
		return start(true, two_p)
	_pending_two_player = two_p
	prompt_mode = "resume"
	Game.goto_scene(SCENE_PROMPT)
	return true


## Seed of a game started from the menu: the time in seconds, like time(NULL).
static func player_seed() -> int:
	return int(Time.get_unix_time_from_system())


## Return from the resume screen.
static func after_resume_prompt(resume_run: bool) -> void:
	if resume_run and resume():
		return
	start(true, _pending_two_player)


static func stop() -> void:
	run = null
	loadout2 = PackedInt32Array()
	_first_pick = false


## g_anomaly_eyes_closed (anomalies.c, effect 0x020): hides the Endless overlays.
static func eyes_closed() -> bool:
	return run != null and (int(run.anomaly_effects()) & 0x20) != 0


static func score() -> int:
	return int(run.score()) if run != null else 0


static func lives() -> int:
	return int(run.lives()) if run != null else 0


static func level_name() -> String:
	return str(run.level_name()) if run != null else ""


## Prepares Game for the next level of the rotation and loads the scene.
static func _goto_next(goto_level := true) -> bool:
	if not run.next_level():
		# endless.c:endless_on_level_end: rotation exhausted, end of game.
		clear_resume()
		stop()
		if goto_level:
			Game.goto_scene(Game.SCENE_TITLE)
		return false
	picks += 1
	Game.difficulty = run.difficulty()
	Game.level_name = level_name()
	print(
		(
			"endless: wave %d, episode %d section %d level %d, armor x%08x"
			% [run.wave(), run.episode(), run.section(), run.lvl_file_num(), run.armor_mult_bits()]
		)
	)
	Game.start_level_at(run.episode(), run.lvl_file_num(), 0, {"load_scene": goto_level})
	return true


## Called by the level scene before TyrianSim.load_level.
static func configure(sim: Node) -> void:
	sim.set_endless_armor_mult(run.armor_mult_bits() if run != null else -1)
	if sim.has_method("set_endless_lives"):
		if run != null:
			sim.set_endless_lives(lives(), int(run.lives_p2()) if two_player else START_LIVES)
		else:
			sim.set_endless_lives(-1, -1)
	if sim.has_method("set_endless_carry"):
		sim.set_endless_carry(carry if run != null else PackedInt32Array())
	if sim.has_method("set_endless_loadout2"):
		sim.set_endless_loadout2(loadout2 if two_player and run != null else PackedInt32Array())
	if sim.has_method("set_endless_options"):
		# endless_start(two_player); anomaly_deck_reapply: deck effects.
		sim.set_endless_options(two_player and run != null, int(run.anomaly_effects()) if run != null else 0)
		if run != null and sim.has_method("set_endless_initial_difficulty"):
			sim.set_endless_initial_difficulty(int(run.initial_difficulty()))
		# combos.c: combo_enabled (true by default, gameplay_menu.c);
		# combo_reset on every endless_start / fresh level (fresh Sim).
		var ms := COMBO_FRAME_MS if run != null and Game.combos_enabled else 0
		# Golden master replay (capture tool): combos like the C.
		if Capture.replay_combo_ms >= 0:
			ms = Capture.replay_combo_ms
		sim.set_combos(ms)


## End of level (endless.c:endless_on_level_end). Lives are read back
## from the Sim (endless_try_respawn already consumed the lost ones there) and the
## wave score is added to the total. Lives exhausted (lives_exhausted):
## end screens. Otherwise debriefing (JE_endLevelAni) then next wave.
static func level_finished(sim: Node, level: Node = null) -> void:
	var st: PackedInt64Array = sim.endless_state() if sim.has_method("endless_state") else PackedInt64Array()
	var l := int(st[0]) if st.size() >= 1 else 0
	run.end_wave_2p(l, int(st[1]) if st.size() >= 2 else 0, st[2] if st.size() >= 3 else 0)
	# endless.c:lives_exhausted: with two players, ends when no player has a life left.
	if run.run_over():
		_game_over()
		return
	var stats: Dictionary = sim.level_stats()
	carry = sim.campaign_carry()
	# endless.c → JE_endLevelAni (mainint.c:2160): adjust_difficulty() before the next
	# wave; two players: sum of both wallets (mainint.c:1733) (ENH-02).
	var total := int(stats.get("total_score", 0))
	if two_player:
		total = int(sim.player_dict(0).get("cash", 0)) + int(sim.player_dict(1).get("cash", 0))
	run.adjust_difficulty(total)
	# Equipment gained during the wave (weapon power, collected weapons, money): the C does not
	# reset player[].items except at endless_start (ENH-03).
	var end_lo: PackedInt32Array = stats.get("loadout", PackedInt32Array())
	if end_lo.size() >= 11:
		Game.loadout = end_lo
	var end_lo2: PackedInt32Array = stats.get("loadout2", PackedInt32Array())
	if two_player and end_lo2.size() >= 11:
		loadout2 = end_lo2
	last_armor = stats.get("armor", PackedInt32Array())
	# mainint.c:2160-2166: in Endless, JE_endLevelAni has no shop, no cubes, no
	# summary or statistics; we go straight on to the next wave.
	after_debrief(level)


## Armors of both players at the end of the wave (ENDR snapshot).
static var last_armor := PackedInt32Array()


## Pause menu, "Quit" confirmed: endless.c:endless_on_level_end without
## endLevel or lives exhausted -> end of game (score, leaderboard, title).
static func quit_run(sim: Node = null) -> void:
	bank_current_wave(sim)
	_game_over()


## endless.c:on_enemy_killed increments s_score live: the current wave's score
## counts for the name, leaderboard and summary when quitting via the pause menu (ENH-01).
static func bank_current_wave(sim: Node) -> void:
	if sim == null or run == null or not sim.has_method("endless_state"):
		return
	var st: PackedInt64Array = sim.endless_state()
	if st.size() >= 3:
		run.end_wave_2p(int(st[0]), int(st[1]), st[2])


## Return from the debriefing (continuation of endless_on_level_end after JE_endLevelAni):
## s_levels_completed++, pick_next_level, endless_run_save. `level`: scene of the
## finished level still displayed; without an anomaly draw, tyrian2.c:222-224 fades it
## to black along with the music (fade_song, fade_black(10)) before the next
## wave.
static func after_debrief(level: Node = null) -> void:
	run.level_completed()
	# endless_on_level_end: anomaly draw (1 out of 3) before the
	# next level, if the deck is not full (anomalies_enabled).
	var cards: PackedInt32Array = run.pick_three(int(run.wave()) - 1) if anomalies_enabled else PackedInt32Array()
	if not cards.is_empty():
		prompt_mode = "pick"
		prompt_picks = cards
		Game.goto_scene(SCENE_PROMPT)
		return
	if level != null and is_instance_valid(level) and level.has_method("_fade_out_before_leaving"):
		await level._fade_out_before_leaving()
	if _goto_next():
		save_resume()


## Return from the draw: anomaly_deck_add + anomaly_apply, then next wave.
static func after_pick(card: int) -> void:
	if card >= 0:
		run.add_card(card)
	var first := _first_pick
	_first_pick = false
	if _goto_next() and not first:
		save_resume()


## End of game: name (if qualified), summary, leaderboard (endless_end scene).
static func _game_over() -> void:
	# endless_apply_score_mult(s_score, deck × mutators) (endless_record_score).
	final_base = score()
	final_score = int(run.final_score(mutators))
	final_wave = int(run.wave())
	final_deck = run.deck_cards()
	final_two_player = two_player
	final_bonus_pct = int(run.deck_bonus_pct())
	clear_resume()
	stop()
	Game.goto_scene(SCENE_END)


## HUD text (endless.c:endless_draw_hud, "endless_wave_label").
static func hud_text() -> String:
	if run == null:
		return ""
	return TyrianText.L("endless_wave_label").replace("%d", str(run.wave()))


# --- Leaderboard (endless.c: board_insert, endless_qualifies) ---------------


static func load_board() -> Array:
	var f := FileAccess.open(BOARD_PATH, FileAccess.READ)
	if f == null:
		return []
	var v: Variant = JSON.parse_string(f.get_as_text())
	return v if v is Array else []


## endless.c:endless_qualifies: non-zero score greater than the last rank
## (an empty rank counts as 0).
static func qualifies(s: int, board: Array) -> bool:
	if s <= 0:
		return false
	if board.size() < LEADERBOARD_SIZE:
		return true
	return s > int(board[LEADERBOARD_SIZE - 1].get("score", 0))


## endless.c:board_insert: first rank with a strictly lower score,
## shift down, last rank lost. Writes the table.
static func insert_score(board: Array, entry: Dictionary) -> Array:
	var slot := board.size()
	for i in board.size():
		if int(entry["score"]) > int(board[i].get("score", 0)):
			slot = i
			break
	if slot >= LEADERBOARD_SIZE:
		return board
	board.insert(slot, entry)
	while board.size() > LEADERBOARD_SIZE:
		board.pop_back()
	_keep_corrupted_board()
	var f := FileAccess.open(BOARD_PATH, FileAccess.WRITE)
	if f == null:
		# endless.c:endless_leaderboard_save: ot_dat_flush fails without telling the
		# player (log only); the table stays in memory for the session.
		push_warning("endless: leaderboard not written (%s)" % error_string(FileAccess.get_open_error()))
		return board
	f.store_string(JSON.stringify(board))
	return board


## An unreadable table is never overwritten by a one-row table: the C
## ignores it on load (endless.c:endless_leaderboard_load) but overwrites it at
## the next save, an inherited defect removed; the old file is kept.
static func _keep_corrupted_board() -> void:
	if not FileAccess.file_exists(BOARD_PATH):
		return
	var text := FileAccess.get_file_as_string(BOARD_PATH)
	var json := JSON.new()
	if json.parse(text) == OK and json.data is Array:
		return
	var backup := BOARD_PATH + ".corrupt"
	if FileAccess.file_exists(backup):
		return
	var out := FileAccess.open(backup, FileAccess.WRITE)
	if out != null:
		out.store_string(text)


# --- Resume (endless.c: endless_run_save / endless_resume) ----------------


static func save_resume() -> void:
	if run == null:
		return
	var f := FileAccess.open(RESUME_PATH, FileAccess.WRITE)
	if f == null:
		push_warning("endless: resume data not written (%s)" % error_string(FileAccess.get_open_error()))
	else:
		# ENDR (endless.c:endless_run_save): difficulty and initial difficulty, weapons and
		# armor of both players, deck draw state.
		f.store_string(
			JSON.stringify(
				{
					"seed": run_seed,
					"picks": picks,
					"levels_completed": int(run.wave()) - 1,
					"lives": lives(),
					"lives_p2": int(run.lives_p2()),
					"score": score(),
					"two_player": two_player,
					"deck": Array(run.deck_cards()),
					"difficulty": int(run.difficulty()),
					"initial_difficulty": int(run.initial_difficulty()),
					"loadout": Array(Game.loadout),
					"loadout2": Array(loadout2),
					"armor": Array(last_armor),
					"deck_rng": int(run.deck_rng())
				}
			)
		)


## endless.c:endless_run_clear (end of game, rotation exhausted).
static func clear_resume() -> void:
	if FileAccess.file_exists(RESUME_PATH):
		DirAccess.remove_absolute(ProjectSettings.globalize_path(RESUME_PATH))


## Name cut to at most `limit` letters; a cut is marked by a trailing "." (same width).
static func shorten_name(name: String, limit: int) -> String:
	return name if name.length() <= limit else name.substr(0, limit - 1) + "."


## Deck initials for the HUD and the resume prompt: [[letter, rank], ...].
static func deck_initials(cards: Variant) -> Array:
	var letters: Array = []
	for card in cards:
		var nm := TyrianText.L("anom_%s_name" % str(TyrianEndless.card_id(int(card))))
		var ch := nm.substr(0, 1).to_upper() if nm != "" else "?"
		if ch < "A" or ch > "Z":
			ch = "?"
		letters.append([ch, int(TyrianEndless.card_rank(int(card)))])
	return letters


static func resume_info() -> Dictionary:
	var f := FileAccess.open(RESUME_PATH, FileAccess.READ)
	if f == null:
		return {}
	var v: Variant = JSON.parse_string(f.get_as_text())
	return v if v is Dictionary else {}


## endless.c:endless_resume: same seed and same draws (deterministic
## rotation), then waves, lives and score restored.
static func resume(goto_level := true) -> bool:
	var info := resume_info()
	if info.is_empty() or not ClassDB.class_exists("TyrianEndless"):
		return false
	run = ClassDB.instantiate("TyrianEndless")
	run_start_pending = true
	run_seed = int(info["seed"])
	two_player = bool(info.get("two_player", false))
	# Program restarted: nothing is carried over from the previous wave (fresh C statics).
	carry = PackedInt32Array()
	run.start_run(Game.data_dir, run_seed, 0, two_player, run_seed ^ 0x5eed)
	# endless_resume: deck restored (ENDR v2), lives of both players.
	for c in info.get("deck", []):
		run.add_card(int(c))
	for i in int(info["picks"]) - 1:
		if not run.next_level():
			stop()
			return false
	for i in int(info["levels_completed"]):
		run.level_completed()
	run.end_wave_2p(int(info["lives"]), int(info.get("lives_p2", 0)), int(info["score"]))
	picks = int(info["picks"]) - 1
	_apply_loadout()
	# ENDR: weapons, power and money of the run, difficulty, draw state (ENH-03/04).
	if info.has("loadout") and (info["loadout"] as Array).size() >= 11:
		Game.loadout = PackedInt32Array(info["loadout"])
	if two_player and info.has("loadout2") and (info["loadout2"] as Array).size() >= 11:
		loadout2 = PackedInt32Array(info["loadout2"])
	if info.has("armor"):
		last_armor = PackedInt32Array(info["armor"])
	run.restore_difficulty(
		int(info.get("difficulty", run.difficulty())), int(info.get("initial_difficulty", run.initial_difficulty()))
	)
	if info.has("deck_rng"):
		run.set_deck_rng(int(info["deck_rng"]))
	return _goto_next(goto_level)
