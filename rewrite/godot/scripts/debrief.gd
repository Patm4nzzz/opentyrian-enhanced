## End-of-level debriefing (mainint.c:JE_endLevelAni), 8-bit rendering
## frame by frame on TyrianScreen (ADR 0020).
##
## Ported: "Completed/Exiting" texts + level name, money, percentage
## of enemies destroyed, cubes (cube animation or "None"), "Press a
## key", with the JE_outTextGlow effect (fonthand.c:407) and its acceleration
## as soon as a key is pressed (frameCountMax = 0).
## Then the detailed statistics screen (mainint.c "Level Statistics
## Screen", Enhanced addition): enemies destroyed, damage per bay (≥ 5%),
## damage taken and shield share, credits earned, level time; it is
## closed with Confirm or Back (Enter / Escape, A / B / Start on the gamepad:
## input_held(INPUT_MENU_CONFIRM / CANCEL) with push_joysticks_as_keyboard).
## Background: the last frame of the level, frozen (frozen_level_screen.gd).
##
## Input: `Game.debrief` (dictionary set by the level scene:
## level_name, cash, enemy_killed, total_enemy, cube_max, all_players_alive,
## bonus_level; level_stats from TyrianSim: damage_by_bay, damage_taken,
## damage_blocked, credits_start). Output: `Game.campaign_after_debrief()`.
extends TyrianScreen

## mainint.c:JE_endLevelAni: textGlowFont = SMALL_FONT_SHAPES (= NORMAL_FONT here).
const GLOW_FONT := NORMAL_FONT
## sndmast.h: S_ITEM = 18, V_LEVEL_END = 36 (JE_playSampleNum, 1-based).
const S_ITEM := 18
const V_LEVEL_END := 36
## mainint.c:2202: play_song(21) (0-based).
const DEBRIEF_TIMED_SONG := 21


## mainint.c:2199: "if (!levelTimer || levelTimerCountdown > 0 || !(episodeNum == 4))"
## plays the voice; otherwise song 21.
static func debrief_plays_song(d: Dictionary, episode: int) -> bool:
	return bool(d.get("level_timer", false)) and int(d.get("timer_countdown", 0)) <= 0 and episode == 4


var _frame_count_max := 4
var _glow_brightness := 6
## True when the screen is waiting for the final key (capture tool).
var idle := false
## Statistics screen displayed, waiting for Confirm / Back.
var stats_shown := false
## Menu action received during the statistics screen (&"": none).
var _stats_action := &""
## Displayed rows [label key, value] (recorded for tests).
var _stats_lines: Array = []
## Texts written by JE_outTextGlow, in order (recorded for tests).
var glow_log: Array = []
## Y coordinate of each `glow_log` text (recorded for tests).
var glow_ys: Array = []
## X coordinate of each `glow_log` text (recorded for tests).
var glow_xs: Array = []
## X coordinate of each cube in the row (recorded for tests).
var cube_xs: Array = []

## mainint.c, "Level Statistics Screen": columns and colors.
const STATS_LABEL_X := 50
const STATS_VALUE_RIGHT_X := 270
const STATS_BAY_KEYS := ["ls_front_gun", "ls_rear_gun", "ls_l_sidekick", "ls_r_sidekick", "ls_special"]


func _ready() -> void:
	# mainint.c:JE_endLevelAni does not call JE_mouseStart: no cursor,
	# a click anywhere speeds up, then continues (JE_anyButton).
	mouse_cursor = false
	_run.call_deferred()


func _any_button() -> bool:
	var seen := _input_seen
	_input_seen = false
	return seen


## Deliberate divergence (the maintainer's decision, 2026-10-10, docs/divergences.md no. 51): the C places these rows at
## fixed x on the left; here each one is centered on the 320 px with its actual width.
func out_text_glow_centered(y: int, s: String) -> void:
	await out_text_glow((320 - TyrianText.width(s, GLOW_FONT)) / 2, y, s)


## fonthand.c:JE_outTextGlow.
func out_text_glow(x: int, y: int, s: String) -> void:
	glow_log.append(s)
	glow_ys.append(y)
	glow_xs.append(x)
	var c := 15
	for d in [[-1, 0], [0, -1], [1, 0], [0, 1]]:
		TyrianText.out_text_adjust(self, x + d[0], y + d[1], s, 0, -12, GLOW_FONT, false)
	if _frame_count_max > 0:
		for z in range(1, 13):
			TyrianText.out_text_adjust(self, x, y, s, c, z - 10, GLOW_FONT, false)
			if _any_button():
				_frame_count_max = 0
			present()
			await wait_units(_frame_count_max)
	var start := 6 if _frame_count_max == 0 else 12
	for z in range(start, _glow_brightness - 1, -1):
		TyrianText.out_text_adjust(self, x, y, s, c, z - 10, GLOW_FONT, false)
		if _any_button():
			_frame_count_max = 0
		present()
		await wait_units(_frame_count_max)
	_glow_brightness = 6


## Deliberate divergence (the maintainer's decision, 2026-10-10, docs/divergences.md no. 51): the C places the cubes at
## x = 20 + 30 * n; here the whole row (30 px spacing, last cube's width included) is
## centered on the 320 px.
func cube_row_xs(count: int) -> Array:
	var row_width: int = 30 * (count - 1) + vga.sprite_size(OPTION_SHAPES, 25).x
	var x0 := (320 - row_width) / 2
	var xs := []
	for i in count:
		xs.append(x0 + 30 * i)
	return xs


## mainint.c:JE_drawCube: shadow (two blit_sprite_dark) then tinted cube.
func draw_cube(x: int, y: int, filter: int, brightness: int) -> void:
	vga.blit_sprite_dark(VGA_SCREEN, x + 4, y + 4, OPTION_SHAPES, 25, false)
	vga.blit_sprite_dark(VGA_SCREEN, x + 3, y + 3, OPTION_SHAPES, 25, false)
	vga.blit_sprite_hv(VGA_SCREEN, x, y, OPTION_SHAPES, 25, filter, brightness)


func _run() -> void:
	var d: Dictionary = Game.debrief
	_frame_count_max = 4
	# The C draws over the last game frame (frozen VGAScreen): the
	# level scene left it in the shared VGA. Without it (extension
	# absent, capture tool), fall back to a black screen with the game palette.
	if not Game.debrief_frozen:
		vga.clear(VGA_SCREEN, 0)
		vga.load_palette(0)
		vga.set_palette(0, 255)
	Game.debrief_frozen = false
	vga.set_colors(Color8(255, 255, 255), 254, 254)
	# mainint.c:2199-2202: the "Level completed" voice, except in episode 4 when
	# the level timer reached 0 (TIME WAR and timed levels):
	# play_song(21) instead.
	if debrief_plays_song(d, Game.episode):
		Music.play_song(DEBRIEF_TIMED_SONG)
	else:
		Sfx.play(V_LEVEL_END)

	var level_name := str(d.get("level_name", ""))
	if d.get("bonus_level", false):
		await out_text_glow_centered(20, TyrianText.L("miscText_16"))
	elif d.get("all_players_alive", true):
		await out_text_glow_centered(20, "%s %s" % [TyrianText.L("miscText_26"), level_name])
	else:
		await out_text_glow_centered(20, "%s %s" % [TyrianText.L("misc_exiting"), level_name])

	# mainint.c:2218-2223: two players, one money row per player (miscText 40 and 41).
	var two_player := (
		Game.two_player_game() or Game.side_mode_flag(0) != 0 or (EndlessSession.active() and EndlessSession.two_player)
	)
	if two_player:
		await out_text_glow_centered(50, "%s %d" % [TyrianText.L("miscText_40"), int(d.get("cash", 0))])
		await out_text_glow_centered(70, "%s %d" % [TyrianText.L("miscText_41"), int(d.get("cash2", 0))])
	else:
		await out_text_glow_centered(50, "%s %d" % [TyrianText.L("miscText_27"), int(d.get("cash", 0))])

	# Timed Battle: "Time Bonus" = (levelTimerCountdown / 10) * 100, added to the money (B-06).
	var timed: bool = Game.time_battle_selection > 0 or Game.side_mode_flag(5) > 0
	var bonus_cash := 0
	if timed:
		var time_bonus := (int(d.get("timer_countdown", 0)) / 10) * 100
		await out_text_glow_centered(75, "%s %d" % [TyrianText.L("miscTextB_6"), time_bonus])
		bonus_cash += time_bonus

	var total := int(d.get("total_enemy", 0))
	# roundf(enemyKilled * 100 / totalEnemy): integer division before roundf.
	var kill_pct := 0 if total == 0 else (int(d.get("enemy_killed", 0)) * 100) / total
	await out_text_glow_centered(90, "%s %d%%" % [TyrianText.L("misc_destroyed"), kill_pct])

	if timed:
		# One animated life icon per remaining life, then "Lives Bonus" = lives × 1000 (B-06);
		# no Cubes block in timed battle.
		var lives := int(d.get("lives", 0))
		for life in range(1, lives + 1):
			Sfx.play(S_ITEM)
			var lx := 20 + 15 * life
			for i in range(-15, 11):
				vga.blit_sprite_hv(VGA_SCREEN, lx, 115, OPTION_SHAPES, 46, 0x9, i)
				if _any_button():
					_frame_count_max = 0
				present()
				await wait_units(_frame_count_max)
			for i in range(10, -1, -1):
				vga.blit_sprite_hv(VGA_SCREEN, lx, 115, OPTION_SHAPES, 46, 0x9, i)
				if _any_button():
					_frame_count_max = 0
				present()
				await wait_units(_frame_count_max)
		var lives_bonus := lives * 1000
		await out_text_glow_centered(120, "%s %d" % [TyrianText.L("miscTextB_7"), lives_bonus])
		bonus_cash += lives_bonus
		# player[0].cash += x: end-of-battle money (leaderboard score).
		Game.loadout[10] = int(Game.loadout[10]) + bonus_cash
		# mainint.c:2425-2429: "Credits" of the statistics screen = cash
		# (bonus included) - credits_start.
		d["cash"] = int(d.get("cash", 0)) + bonus_cash
	elif not (
		Game.two_player_game()
		or Game._two_player_or_action()
		or Game.side_mode_flag(0) != 0
		or Game.side_mode_flag(2) != 0
		or EndlessSession.active()
	):
		# mainint.c: "else if (!onePlayerAction && !twoPlayerMode)": Cubes block.
		await out_text_glow_centered(120, TyrianText.L("miscText_3"))
		var cube_max := mini(int(d.get("cube_max", 0)), 4)
		if cube_max > 0:
			if _frame_count_max != 0:
				_frame_count_max = 1
			cube_xs = cube_row_xs(cube_max)
			for cube in range(1, cube_max + 1):
				Sfx.play(S_ITEM)
				var x: int = cube_xs[cube - 1]
				draw_cube(x, 135, 9, 0)
				present()
				for i in range(-15, 11):
					vga.blit_sprite_hv(VGA_SCREEN, x, 135, OPTION_SHAPES, 25, 0x9, i)
					if _any_button():
						_frame_count_max = 0
					present()
					await wait_units(_frame_count_max)
				for i in range(10, -1, -1):
					vga.blit_sprite_hv(VGA_SCREEN, x, 135, OPTION_SHAPES, 25, 0x9, i)
					if _any_button():
						_frame_count_max = 0
					present()
					await wait_units(_frame_count_max)
		else:
			await out_text_glow_centered(135, TyrianText.L("miscText_14"))

	var was_animating := _frame_count_max != 0
	if was_animating:
		_frame_count_max = 6
	# mainint.c:2336: "Press a key" moves up 10 px in two-player mode.
	await out_text_glow_centered(150 if two_player else 160, TyrianText.L("misc_press_key"))

	idle = true
	# Capture tool: --hold keeps the first screen displayed for the screenshot.
	while Capture.active and Capture.has_option("hold") and is_inside_tree():
		await wait_units(1)
	if not Capture.active:
		_input_seen = false
		# is_inside_tree: wait_units returns immediately when the screen has
		# left the tree (level exited, scene replaced); without this exit the
		# loop would spin forever, never yielding to the engine.
		while is_inside_tree() and not _any_button() and not (_frame_count_max == 0 and was_animating):
			await wait_units(1)
	idle = false
	await _level_statistics(d)
	# tyrian2.c:320: fade_song() after JE_endLevelAni (the end-of-level
	# song plays throughout the summary).
	Music.fade_song()
	await fade_black(15)
	# Endless (endless.c:endless_on_level_end): next wave after JE_endLevelAni.
	if EndlessSession.active():
		EndlessSession.after_debrief()
		return
	Game.campaign_after_debrief()


## Confirm / Back during the statistics screen (TyrianScreen._unhandled_input:
## keyboard and gamepad through TyrianControls actions).
func handle_action(action: StringName) -> void:
	if stats_shown and action in [TyrianControls.ACCEPT, TyrianControls.BACK]:
		_stats_action = action


## "label ... value" row (STAT_LINE macro): value right-aligned.
func _stat_line(key: String, value: String, y: int) -> int:
	_stats_lines.append([key, value])
	var label := TyrianText.L(key)
	TyrianText.text_shade(self, STATS_LABEL_X, y, label, 15, 2, TyrianText.FULL_SHADE)
	var tw := TyrianText.width(value, SMALL_FONT)
	TyrianText.text_shade(self, STATS_VALUE_RIGHT_X - tw, y, value, 15, 2, TyrianText.FULL_SHADE)
	return y + 11


## Section title (SECTION macro).
func _stat_section(title: String, y: int) -> int:
	y += 3
	TyrianText.text_shade(self, STATS_LABEL_X - 6, y, title, 14, 4, TyrianText.FULL_SHADE)
	return y + 12


## mainint.c:JE_endLevelAni, "Level Statistics Screen" (if !constantPlay).
func _level_statistics(d: Dictionary) -> void:
	# JE_loadPic(VGAScreen, 2, true): menu background and its palette, black bottom band.
	_stats_lines.clear()
	vga.load_pic(VGA_SCREEN, 2, true)
	vga.fill_rectangle_xy(VGA_SCREEN, 0, 192, 319, 199, 0)
	var y := 10
	TyrianText.draw_font_hv_shadow(self, 160, y, TyrianText.L("ls_title"), NORMAL_FONT, CENTERED, 15, -3, false, 2)
	y += 22
	# COMBAT
	y = _stat_section(TyrianText.L("ls_combat"), y)
	var killed := int(d.get("enemy_killed", 0))
	var total := int(d.get("total_enemy", 0))
	y = _stat_line("ls_enemies", "%d / %d  (%d%%)" % [killed, total, killed * 100 / total if total > 0 else 0], y)
	# Per-weapon breakdown: bays ≥ 5% only.
	var bays: PackedInt32Array = d.get("damage_by_bay", PackedInt32Array())
	var total_dmg := 0
	for v in bays:
		total_dmg += v
	if total_dmg > 0:
		for i in mini(bays.size(), STATS_BAY_KEYS.size()):
			if bays[i] == 0:
				continue
			var pct := bays[i] * 100 / total_dmg
			if pct < 5:
				continue
			y = _stat_line(STATS_BAY_KEYS[i], "%d%%" % pct, y)
	# SURVIVAL: row always displayed, even at zero.
	y = _stat_section(TyrianText.L("ls_survival"), y)
	var taken := int(d.get("damage_taken", 0))
	var taken_text := "0"
	if taken > 0:
		taken_text = "%d  (%s %d%%)" % [taken, TyrianText.L("ls_shield"), int(d.get("damage_blocked", 0)) * 100 / taken]
	y = _stat_line("ls_damage_taken", taken_text, y)
	# RESULTS
	y = _stat_section(TyrianText.L("ls_results"), y)
	var earned := int(d.get("cash", 0)) - int(d.get("credits_start", d.get("cash", 0)))
	y = _stat_line("ls_credits", "%s%d" % ["+" if earned >= 0 else "", earned], y)
	# SDL_GetTicks() - level_stats.start_ticks: real time since the start of the level.
	var secs := maxi(Time.get_ticks_msec() - Game.level_start_ms, 0) / 1000
	y = _stat_line("ls_time", "%d:%02d" % [secs / 60, secs % 60], y)
	# Exit prompt centered at the bottom, like the debriefing.
	var press_key := TyrianText.L("misc_press_key")
	TyrianText.text_shade(
		self, TyrianText.font_center(press_key, SMALL_FONT), 182, press_key, 14, 4, TyrianText.FULL_SHADE
	)
	present()
	stats_shown = true
	_stats_action = &""
	if not Capture.active:
		while is_inside_tree() and _stats_action == &"":
			await wait_units(1)
	stats_shown = false
