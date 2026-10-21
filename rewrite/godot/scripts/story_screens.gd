## Story and end-of-episode screens: the "display" part of
## level_load.c:JE_loadMap (directives `]P`, `]W`, `]U`, `]C`, `]F`, `]A`,
## `]M`, `]@` of the episode script, in file order), then, on `]Q[`,
## the score and secret hint page, mainint.c:JE_nextEpisode (best
## score, credits, next episode), the revealed Super Arcade ship, and, on
## `]q[` in timed battle, highscore.c:JE_highScoreCheck.
##
## Opened by Game.advance_campaign with the result of TyrianCampaign.run_script
## (`Game.story_script`); hands control back through Game.campaign_script_done (level,
## shop) or Game.campaign_next_episode.
extends TyrianScreen

const HighScores := preload("res://scripts/high_scores.gd")

## sprite.h:EXTRA_SHAPES (credits images, estsc.shp).
const EXTRA_SHAPES := 7
## pcxmast.c:pcxpal (palette of each tyrian.pic image).
const PCX_PALETTE := [0, 7, 5, 8, 10, 5, 18, 19, 19, 20, 21, 22, 5, 23]
## varz.c:SANextShip, SAShip; varz.h: SA, SA_NORTSHIPZ, SA_DESTRUCT, SA_ENGAGE.
const SA_NEXT_SHIP := [3, 8, 6, 2, 5, 1, 4, 10, 9, 7, 3]
const SA_SHIP := [3, 1, 5, 10, 2, 11, 12, 15, 17]
const SA := 9
const SA_NORTSHIPZ := 7
const SA_DESTRUCT := 10
const SA_ENGAGE := 11
## opentyr.h:DIFFICULTY_ZINGLON.
const DIFFICULTY_ZINGLON := 8
## sndmast.h:S_WARNING.
const S_WARNING := 17
## mainint.c:JE_playCredits: lines_max, play_song(9) at the end.
const CREDITS_LINES_MAX := 126
const SONG_CREDITS_END := 9
## highscore.c: name of 28 characters at most.
const HIGH_SCORE_NAME_MAX := 28

## Text color of level_load.c (C globals, kept from one call of
## JE_loadMap to the next): warningRed (red text at the top), useLastBank (`]@`).
static var warning_red := false
static var use_last_bank := false

## True when the screen is waiting for a key (capture tool, campaign bot).
var idle := false
## True during name entry ("Enter your name").
var name_entry := false

## Context of level_load.c:JE_displayText (fonthand.c).
var _frame_count_max := 0
var _warning_display := false
var _warning_col := 14 * 16 + 5
var _warning_col_change := 1
var _warning_sound_delay := 0
var _delay2_end_ms := 0
## Name entry: keys received since the last read, in order
## (typed character, or TyrianControls action ACCEPT / BACK / DELETE).
var _name_keys: Array = []
var _click := Vector2(-1, -1)


func _ready() -> void:
	mouse_cursor = false
	_run.call_deferred()


func _run() -> void:
	var r: Dictionary = Game.story_script
	# level_load.c, start of JE_loadMap: useLastBank = false.
	use_last_bank = false
	for e: Dictionary in r.get("screens", []):
		if not is_inside_tree():
			return
		await _screen(str(e.get("directive", "")), e.get("text", PackedStringArray()))
	if not is_inside_tree():
		return
	match str(r.get("stop", "")):
		"episode_end":
			await _episode_end(r)
		"title":
			# ]q[ in timed battle: JE_highScoreCheck, then the title.
			if Game.time_battle_selection > 0:
				await high_score_check(true)
			Game.campaign_script_done(r)
		_:
			Game.campaign_script_done(r)


# --- Screen directives (level_load.c:JE_loadMap) --------------------------------


func _screen(directive: String, text: PackedStringArray) -> void:
	if directive.length() < 2:
		return
	var arg := _atoi(directive.substr(3))
	match directive[1]:
		"M":
			Music.play_song(arg - 1)
		"@":
			use_last_bank = not use_last_bank
		"P":
			if arg > 900:
				vga.load_palette(PCX_PALETTE[clampi(arg - 1 - 900, 0, PCX_PALETTE.size() - 1)])
				vga.clear(VGA_SCREEN, 0)
				present()
				await fade_palette(1)
			else:
				if arg == 0:
					vga.load_pcx(VGA_SCREEN, Game.data_dir.path_join("tshp2.pcx"))
				else:
					vga.load_pic(VGA_SCREEN, arg, false)
				present()
				await fade_palette(10)
		"U":
			await _wipe_up(arg)
		"V", "R":
			# Directives absent from the five shipped levelsN.dat (level_load.c
			# notes it): image set without the sweep.
			vga.load_pic(VGA_SCREEN, arg, false)
			present()
		"C":
			await fade_black(10)
			vga.clear(VGA_SCREEN, 0)
			vga.load_palette(7)
			vga.set_palette(0, 255)
			present()
		"B":
			await fade_black(10)
		"F":
			await fade_white(100)
			await fade_black(30)
			vga.clear(VGA_SCREEN, 0)
			present()
		"A":
			await _play_anim("tyrend.anm", 7)
		"W":
			# level_load.c, case 'W': "]Wy 01[" = alert bars (y), then
			# tens digit = red text, units digit = appearance speed.
			_warning_col = 14 * 16 + 5
			_warning_col_change = 1
			_warning_sound_delay = 0
			_warning_display = directive.length() > 2 and directive[2] == "y"
			var fcm := _atoi(directive.substr(4))
			_delay2_end_ms = _now_ms() + _units_ms(6)
			warning_red = fcm / 10 != 0
			_frame_count_max = fcm % 10
			await _display_text(text)


## level_load.c, case 'U': image `n` rises from the bottom pushing
## the old one (one line per setDelay(1)); a key skips the sweep.
func _wipe_up(n: int) -> void:
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	vga.load_pic(VGA_SCREEN, n, false)
	var pic: PackedByteArray = vga.indices(VGA_SCREEN)
	var old: PackedByteArray = vga.indices(VGA_SCREEN2)
	_input_seen = false
	for z in 200:
		if _input_seen or not is_inside_tree():
			break
		vga.set_indices(VGA_SCREEN, pic.slice((199 - z) * 320) + old.slice(0, (199 - z) * 320))
		present()
		await wait_units(1)
	vga.set_indices(VGA_SCREEN, pic)


## animlib.c:JE_playAnim: black screen, animation palette, one image per
## `speed` units; a key stops it.
func _play_anim(file: String, speed: int) -> void:
	var anim: Dictionary = vga.anm_frames(Game.data_dir.path_join(file))
	if anim.is_empty():
		return
	vga.set_colors_rgb(anim["palette"])
	vga.set_palette(0, 255)
	vga.clear(VGA_SCREEN, 0)
	present()
	_input_seen = false
	# setDelay(speed) before each image, wait_delay after: fixed rate,
	# measured from the first image (without accumulating timer latency).
	var period_ms := speed * DELAY_PERIOD * 1000.0
	var start_ms := Time.get_ticks_msec()
	var i := 0
	for f: PackedByteArray in anim["frames"]:
		if not is_inside_tree():
			return
		vga.set_indices(VGA_SCREEN, f)
		present()
		if _any_button():
			break
		i += 1
		if Capture.active:
			await get_tree().process_frame
			continue
		while Time.get_ticks_msec() < start_ms + i * period_ms and is_inside_tree():
			await get_tree().process_frame


## level_helpers.c:JE_displayText: glowing lines starting at y = 55 (2 if
## red text), "Press a key", wait for a key.
func _display_text(lines: PackedStringArray) -> void:
	# Only keys pressed during this screen count (the C code reads the
	# new SDL events at each JE_anyButton).
	_input_seen = false
	var y := 2 if warning_red else 55
	for line in lines:
		await _out_char_glow(10, y, line)
		y += 10
	var finish_on_skip := _frame_count_max != 0
	if finish_on_skip:
		_frame_count_max = 6
	var press := TyrianText.L("misc_press_key")
	await _out_char_glow(TyrianText.font_center(press, SMALL_FONT), 7 * 16 + 6 if warning_red else 184, press)
	idle = true
	_input_seen = false
	if not Capture.active:
		while is_inside_tree():
			if _warning_display:
				_update_warning()
			await wait_units(1)
			if _any_button() or (_frame_count_max == 0 and finish_on_skip):
				break
	idle = false
	_warning_display = false


## mainint.c:JE_outCharGlow: each character lights up then falls back, a
## wave of 28 characters at a time (TINY_FONT, bank 14, 15 with `]@`, 7 in
## red); a key displays the rest at once.
func _out_char_glow(x: int, y: int, s: String) -> void:
	var bank := 7 if warning_red else (15 if use_last_bank else 14)
	if s.is_empty():
		return
	if _frame_count_max == 0:
		TyrianText.text_shade(self, x, y, s, bank, 0, TyrianText.PART_SHADE)
		present()
		return
	var ids := TyrianText.glyphs(s, SMALL_FONT)
	var n := ids.size()
	var glow_col := []
	var glow_step := []
	var text_x := []
	var loc := x
	for z in n:
		glow_col.append(-8)
		glow_step.append(1)
		text_x.append(loc)
		if ids[z] == -2:
			loc += 6
		elif ids[z] >= 0:
			loc += vga.sprite_size(SMALL_FONT, ids[z]).x + 1
	for l in n + 28:
		if not is_inside_tree():
			return
		var sprite_id := -1
		for z in range(l - 28, l + 1):
			if z >= 0 and z < n:
				sprite_id = ids[z]
				if sprite_id >= 0:
					vga.blit_sprite_hv(VGA_SCREEN, text_x[z], y, SMALL_FONT, sprite_id, bank, glow_col[z])
					glow_col[z] += glow_step[z]
					if glow_col[z] > 9:
						glow_step[z] = -1
		if sprite_id >= 0 and l < n:
			vga.blit_sprite_dark(VGA_SCREEN, text_x[l] + 1, y + 1, SMALL_FONT, sprite_id, true)
		if _any_button():
			_frame_count_max = 0
		await _wait_with_warning(_frame_count_max)
		present()


## Waits `units` setDelay units while animating the alert bars.
func _wait_with_warning(units: int) -> void:
	if Capture.active or units <= 0:
		if _warning_display:
			_update_warning()
		return
	# Cumulative deadline, like wait_units (nortsong.c: setDelay then wait_delay on
	# `target`): a fresh deadline per character waited for the next frame, i.e.
	# two frames per step at 70 fps; the episode 4 end text lasted 159 s
	# instead of 135 s in the C code.
	var now := Time.get_ticks_usec() / 1000.0
	if _delay_next_ms < now - 4.0 * DELAY_PERIOD * 1000.0:
		_delay_next_ms = now
	_delay_next_ms += units * DELAY_PERIOD * 1000.0
	while is_inside_tree():
		if _warning_display:
			_update_warning()
		var left := (_delay_next_ms - Time.get_ticks_usec() / 1000.0) / 1000.0
		if left <= 0.0:
			break
		# At most one wait frame, to animate the alert bars.
		await get_tree().create_timer(minf(left, 1.0 / 70.0), true, false, true).timeout


## fonthand.c:JE_updateWarning: blinking bars at the top and bottom, alert
## sound every 15 updates.
func _update_warning() -> void:
	if _now_ms() < _delay2_end_ms:
		return
	_warning_col += _warning_col_change
	if _warning_col > 14 * 16 + 10 or _warning_col < 14 * 16 + 4:
		_warning_col_change = -_warning_col_change
	vga.fill_rectangle_xy(VGA_SCREEN, 0, 0, 319, 5, _warning_col)
	vga.fill_rectangle_xy(VGA_SCREEN, 0, 194, 319, 199, _warning_col)
	present()
	_delay2_end_ms = _now_ms() + _units_ms(6)
	if _warning_sound_delay > 0:
		_warning_sound_delay -= 1
	else:
		_warning_sound_delay = 14
		Sfx.play(S_WARNING)


# --- End of episode (level_load.c, case 'Q'; mainint.c:JE_nextEpisode) -----------


func _episode_end(r: Dictionary) -> void:
	# Score and hint page: temp = secretHint + (rng % 3) * 3, blocks 1..9.
	var lines := PackedStringArray()
	var muts := _mutators()
	var mut_line := mutator_notice(muts)
	if Game.two_player_game():
		for i in 2:
			lines.append("%s %d" % [TyrianText.L("miscText_%d" % (48 + i)), _cash(i)])
	else:
		lines.append("%s %d" % [TyrianText.L("misc_score2"), Game.total_score()])
	lines.append(mut_line)
	var blocks: Array = r.get("hint_blocks", [])
	var block := clampi(Game.secret_hint, 1, 3) + (randi() % 3) * 3
	if block > blocks.size():
		block = 1
	if not blocks.is_empty():
		lines.append_array(blocks[block - 1])
	_frame_count_max = 4
	await _display_text(lines.slice(0, 14))
	await fade_black(15)
	if not is_inside_tree():
		return
	# JE_nextEpisode.
	Game.level_name = "Completed"
	Game.last_level_name = "Completed"
	Game.ach_episode_done(Game.episode, _mutators())
	if Game.campaign_active and Game.episode == Game.initial_episode and not Game.game_has_repeated:
		await high_score_check(false)
	var next := int(r.get("next_episode", 1))
	var jump_back := bool(r.get("jump_back_to_episode1", false))
	if jump_back:
		Game.game_has_repeated = true
		if Game.episode > 2:
			await play_credits()
		# "randomly give player the SuperCarrot".
		if randi() % 6 == 0:
			Game.give_super_carrot()
	var finished := Game.episode
	Game.episode = next
	Music.play_song(Music.SONG_NEXT_EPISODE)
	vga.clear(VGA_SCREEN, 0)
	vga.load_palette(5)
	var ep_name := TyrianText.L("ep_%d" % next)
	TyrianText.dstring(self, TyrianText.font_center(ep_name, NORMAL_FONT), 130, ep_name, NORMAL_FONT)
	var press := TyrianText.L("misc_press_key")
	TyrianText.dstring(self, TyrianText.font_center(press, NORMAL_FONT), 185, press, NORMAL_FONT)
	present()
	await fade_palette(15)
	await _wait_any_button()
	await fade_black(15)
	# Back to episode 1 in one-player: the next Super Arcade ship's code.
	var super_tyrian := Game.campaign_modes.size() >= 10 and Game.campaign_modes[4] != 0
	if jump_back and not Game.two_player_game():
		var sa_mode: int = Game.campaign_modes[3] if Game.campaign_modes.size() >= 10 else 0
		if super_tyrian:
			sa_mode = 8 if Game.initial_difficulty == DIFFICULTY_ZINGLON else 1
		if sa_mode < SA_ENGAGE:
			await _reveal_super_arcade_ship(SA_NEXT_SHIP[sa_mode])
	Game.campaign_next_episode(finished, super_tyrian and jump_back)


## level_load.c, case 'Q' (jumpBackToEpisode1): "Super Arcade Mode", name and
## ship image, "Type <code> at Title".
func _reveal_super_arcade_ship(next_ship: int) -> void:
	vga.load_pic(VGA_SCREEN, 1, false)
	vga.clear(VGA_SCREEN, 0)
	var title := TyrianText.L("superShip_0")
	if next_ship == SA_DESTRUCT:
		TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 30, title, LARGE_FONT)
		var dn := TyrianText.L("sa_destruct_name")
		TyrianText.dstring(self, TyrianText.font_center(dn, NORMAL_FONT), 100, dn, NORMAL_FONT)
	else:
		TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 30, title, LARGE_FONT)
		var ship_name := TyrianText.L("superShip_%d" % next_ship)
		TyrianText.dstring(self, TyrianText.font_center(ship_name, NORMAL_FONT), 100, ship_name, NORMAL_FONT)
	if next_ship == SA_NORTSHIPZ:
		Game.trent_win = true
	if next_ship <= SA and next_ship != SA_NORTSHIPZ:
		var g := _ship_graphic(SA_SHIP[next_ship - 1])
		if g > 500:
			vga.blit_sprite2(VGA_SCREEN, 148, 70, "main:12", g - 500, 0, 0, true)
		else:
			vga.blit_sprite2(VGA_SCREEN, 148, 70, "main:8", g, 0, 0, true)
	var code := TyrianText.L("sa_type_at_title").replace("%s", TyrianText.L("specialName_%d" % (next_ship - 1)))
	TyrianText.dstring(self, TyrianText.font_center(code, NORMAL_FONT), 160, code, NORMAL_FONT)
	present()
	await fade_palette(50)
	await _wait_any_button()


func _ship_graphic(ship: int) -> int:
	var g: int = Mods.ship_graphic(ship) if Mods.has_method("ship_graphic") else -1
	if g >= 0:
		return g
	var ships: Array = _items()["ships"]
	return int(ships[ship].get("shipgraphic", 0)) if ship < ships.size() else 0


func _items() -> Dictionary:
	return TyrianAssets.read_json("items.json")["episodes_1_3" if Game.episode <= 3 else "episode_%d" % Game.episode]


# --- Credits (mainint.c:JE_playCredits) ------------------------------------------


## Scrolling text of tyrian.cdt, images of estsc.shp fading in on the right,
## ships crossing the screen; a key stops it.
func play_credits() -> void:
	var count: int = vga.load_sprite_table(EXTRA_SHAPES, Game.data_dir.path_join("estsc.shp"))
	var lines: PackedStringArray = vga.credits_lines(Game.data_dir.path_join("tyrian.cdt"))
	if count <= 0 or lines.is_empty():
		return
	var rng := RandomNumberGenerator.new()
	var ships: Array = _items()["ships"]
	var delay2_end := _now_ms() + _units_ms(1000)
	Music.play_song(Music.SONG_CREDITS)
	vga.load_palette(5)
	vga.clear(VGA_SCREEN, 0)
	present()
	await fade_palette(2)
	var current_pic := 0
	var fade := 0
	var fade_change := 1
	var ship := 0
	var ship_x := 0
	var ship_wait := 0
	var ship_speed := 0
	var ship_accel := 0
	var ticks_max := lines.size() * 20 * 3
	var ticks := 0
	_input_seen = false
	# The C's setDelay(1) waits exactly 1 unit; here the deadline accumulates so that the
	# engine's frame rate (14 ms at 70 fps) does not lengthen each tick.
	var next_tick_ms := float(_now_ms())
	while ticks < ticks_max and is_inside_tree():
		next_tick_ms = maxf(next_tick_ms + DELAY_PERIOD * 1000.0, float(_now_ms()) - DELAY_PERIOD * 4000.0)
		vga.clear(VGA_SCREEN, 0)
		var size: Vector2i = vga.sprite_size(EXTRA_SHAPES, current_pic)
		vga.blit_sprite_hv(VGA_SCREEN, 319 - size.x, 100 - size.y / 2, EXTRA_SHAPES, current_pic, 0, fade - 15)
		fade += fade_change
		if fade == 0 and fade_change == -1:
			fade_change = 1
			current_pic += 1
			if current_pic >= count:
				current_pic = 0
		if fade == 15:
			fade_change = 0
		if _now_ms() >= delay2_end:
			fade_change = -1
			delay2_end = _now_ms() + _units_ms(900)
		if ticks % 200 == 0:
			ship = rng.randi() % 11 + 1
			ship_wait = rng.randi() % 80 + 10
			if rng.randi() % 2 == 1:
				ship_x = 1
				ship_speed = 0
				ship_accel = 1
			else:
				ship_x = 900
				ship_speed = 0
				ship_accel = -1
		ship_wait -= 1
		if ship_wait == 0:
			if ship_x == 1 or ship_x == 900:
				ship_speed = 0
			ship_accel = -ship_accel
			ship_wait = rng.randi() % 40 + 15
		ship_speed += ship_accel
		ship_x += ship_speed
		if ship_x < 1:
			ship_x = 1
			ship_wait = 1
		if ship_x > 900:
			ship_x = 900
			ship_wait = 1
		if 450 + ship_speed * ship_speed > 900:
			if (ship_accel < 0 and ship_speed < 0) or (ship_accel > 0 and ship_speed > 0):
				ship_wait = 1
		var sprite := int(ships[ship].get("shipgraphic", 0)) if ship < ships.size() else 0
		if ship_speed < -10:
			sprite -= 4 if ship_speed < -20 else 2
		elif ship_speed > 10:
			sprite += 4 if ship_speed > 20 else 2
		vga.blit_sprite2(VGA_SCREEN, ship_x / 40, 184 - (ticks % 200), "main:8", sprite, 0, 0, true)
		var bottom_line := (ticks / 3) / 20
		var y := 20 - ((ticks / 3) % 20)
		for line in range(bottom_line - 10, bottom_line):
			if line >= 0 and line < lines.size():
				var s := lines[line]
				if s != "." and not s.is_empty():
					var color := s.unicode_at(0) - 65
					var t := s.substr(1)
					var x := 110 - TyrianText.width(t, NORMAL_FONT) / 2
					TyrianText.out_text_adjust(
						self, x + absi((y / 18) % 4 - 2) - 1, y - 1, t, color, -8, NORMAL_FONT, false
					)
					TyrianText.out_text_adjust(self, x, y, t, color, -2, NORMAL_FONT, false)
			y += 20
		vga.fill_rectangle_xy(VGA_SCREEN, 0, 0, 319, 10, 0)
		vga.fill_rectangle_xy(VGA_SCREEN, 0, 190, 319, 199, 0)
		if current_pic == count - 1:
			TyrianText.out_text_adjust(self, 5, 180, TyrianText.L("miscText_54"), 2, -2, NORMAL_FONT, false)
		if bottom_line == CREDITS_LINES_MAX - 8:
			Music.fade_song()
		if ticks == ticks_max - 1:
			ticks -= 1
			Music.play_song(SONG_CREDITS_END)
		present()
		if Capture.active:
			break
		while float(_now_ms()) < next_tick_ms and is_inside_tree():
			await get_tree().process_frame
		if _any_button():
			break
		ticks += 1
	await fade_black(10)


# --- High scores (highscore.c:JE_highScoreCheck) ----------------------------------


## For each player (two players: highest score first), if the score
## makes the top three: name entry, then the score table.
## Score: JE_totalScore in one-player, money in two-player and in timed
## battle (table timeBattleSelection - 1, entry without cancel).
func high_score_check(timed: bool) -> void:
	if HighScores.t2k_high_scores.is_empty() and not HighScores.load_scores():
		HighScores.t2k_high_scores = HighScores.default_high_scores()
	var two_p := Game.two_player_game() and not timed
	var muts := _mutators()
	var mult := _mutator_multiplier(muts)
	for temp_p in 2 if two_p else 1:
		HighScores.sort_high_scores(HighScores.t2k_high_scores)
		var p := temp_p
		var table := 10 + (Game.initial_episode - 1) * 2
		var score: int
		if timed:
			score = _cash(0)
			table = Game.time_battle_selection - 1
		elif two_p:
			if _cash(0) < _cash(1):
				p = 1 if temp_p == 0 else 0
			score = _cash(p)
			table += 1
		else:
			score = Game.total_score()
		score = HighScores.apply_multiplier(score, mult)
		var slot := HighScores.qualifying_slot(table, score)
		if slot < 0:
			continue
		var res: Dictionary = await _enter_name(score, p, two_p, timed, muts, mult)
		HighScores.shift_down(table, slot)
		if not res["cancel"] or timed:
			HighScores.set_entry(table, slot, score, res["name"], Game.difficulty, muts != 0)
		HighScores.save_scores()
		await _show_table(table, slot, timed)


## "Enter Thy name": Congratulations, prompt, score, entry field
## (uppercase, 28 characters, game font), Enter confirms, Esc cancels;
## click on the Yes (56-142, 123-149) or No (151-237) area.
func _enter_name(score: int, p: int, two_p: bool, timed: bool, muts: int, mult: float) -> Dictionary:
	vga.clear(VGA_SCREEN, 0)
	present()
	vga.load_palette(0)
	if not timed:
		Music.play_song(Music.SONG_HIGH_SCORES)
	var flash := 8 * 16 + 10
	var fadein := true
	var name := ""
	var cancel := false
	if timed:
		vga.load_pic(VGA_SCREEN, 13, false)
	vga.bar_shade(VGA_SCREEN, 65, 55, 255, 155)
	name_entry = true
	mouse_cursor = true
	_name_keys.clear()
	var quit := false
	while not quit and is_inside_tree():
		var congrats := TyrianText.L("miscText_51")
		TyrianText.dstring(self, TyrianText.font_center(congrats, LARGE_FONT), 3, congrats, LARGE_FONT)
		var line := TyrianText.L("miscText_%d" % ((58 + p if two_p else 53) - 1))
		TyrianText.dstring(self, TyrianText.font_center(line, NORMAL_FONT), 30, line, NORMAL_FONT)
		vga.blit_sprite(VGA_SCREEN, 50, 50, OPTION_SHAPES, 35)
		if two_p:
			TyrianText.text_shade(
				self,
				60,
				55,
				"%s %s" % [TyrianText.L("miscText_%d" % (48 + p)), TyrianText.L("miscText_53")],
				11,
				4,
				TyrianText.FULL_SHADE
			)
		else:
			TyrianText.text_shade(self, 60, 55, TyrianText.L("miscText_53"), 11, 4, TyrianText.FULL_SHADE)
		TyrianText.text_shade(
			self, 70, 70, "%s %d" % [TyrianText.L("misc_score2"), score], 11, 4, TyrianText.FULL_SHADE
		)
		if muts != 0:
			var notice := (
				"%s x%.2f" % [TyrianText.L("mut_score_notice"), mult]
				if mult > 1.0
				else TyrianText.L("mut_score_notice")
			)
			TyrianText.text_shade(self, 70, 103, notice, 15, 2, TyrianText.FULL_SHADE)
		# Blinking cursor until a key or a click.
		var got := false
		while not got and is_inside_tree():
			flash = 8 * 16 + 2 if flash == 8 * 16 + 10 else 8 * 16 + 10
			TyrianText.out_text(self, 65, 89, name, 8, 3)
			var cx := 65 + TyrianText.width(name, SMALL_FONT)
			vga.bar_shade(VGA_SCREEN, cx + 2, 90, cx + 6, 95)
			vga.fill_rectangle_xy(VGA_SCREEN, cx + 1, 89, cx + 5, 94, flash)
			for i in 14:
				present()
				if fadein:
					# highscore.c:475-482: fade between JE_mouseStart and JE_mouseReplace.
					await fade_palette_over_pointer(15)
					fadein = false
				if Capture.active:
					_name_keys.append(TyrianControls.ACCEPT)
				await wait_units(1)
				if not _name_keys.is_empty() or _click.x >= 0:
					got = true
					break
		if not Music.is_playing():
			Music.play_song(Music.SONG_ZANAC)
		if _click.x >= 0:
			var c := _click
			_click = Vector2(-1, -1)
			if c.y > 123 and c.y < 149:
				if c.x > 56 and c.x < 142:
					quit = true
				elif c.x > 151 and c.x < 237:
					quit = true
					cancel = true
			continue
		while not _name_keys.is_empty() and not quit:
			var k: Variant = _name_keys.pop_front()
			if k is StringName:
				match k:
					TyrianControls.DELETE:
						name = name.left(maxi(name.length() - 1, 0))
					TyrianControls.BACK:
						quit = true
						cancel = true
					TyrianControls.ACCEPT:
						quit = true
			else:
				var up := str(k).to_upper()
				if (up == " " or TyrianText.sprite_id(up.unicode_at(0)) >= 0) and name.length() < HIGH_SCORE_NAME_MAX:
					name += up
		_name_keys.clear()
	name_entry = false
	mouse_cursor = false
	return {"name": name, "cancel": cancel}


## Table after the entry: "High Scores", episode (or battle), the
## other lines, then the new line and a glowing "Press a key".
func _show_table(table: int, slot: int, timed: bool) -> void:
	await fade_black(15)
	# The Enter that confirmed the name must not skip the glow or the wait.
	_input_seen = false
	vga.load_pic(VGA_SCREEN, 2, false)
	var title := TyrianText.L("hs_title")
	TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 10, title, LARGE_FONT)
	var sub := (
		TyrianText.L("timedBattle_%d" % Game.time_battle_selection) if timed else TyrianText.L("ep_%d" % Game.episode)
	)
	TyrianText.dstring(self, TyrianText.font_center(sub, NORMAL_FONT), 35, sub, NORMAL_FONT)
	var rows: Array = HighScores.t2k_high_scores[table]
	var any_marked := false
	for i in 3:
		if i != slot:
			TyrianText.text_shade(
				self, 20, i * 12 + 65, "~#%d:~  %d" % [i + 1, rows[i].score], 15, 0, TyrianText.FULL_SHADE
			)
			TyrianText.text_shade(self, 150, i * 12 + 65, rows[i].name, 15, 2, TyrianText.FULL_SHADE)
		any_marked = any_marked or HighScores._is_marked(rows[i].name)
	if any_marked:
		TyrianText.text_shade(
			self,
			20,
			105,
			"%s %s" % [HighScores.MUTATOR_SCORE_MARK, TyrianText.L("mut_score_legend")],
			15,
			0,
			TyrianText.FULL_SHADE
		)
	present()
	await fade_palette(15)
	_frame_count_max = 6
	await _out_text_glow(20, slot * 12 + 65, "~#%d:~  %d" % [slot + 1, rows[slot].score], 10)
	await _out_text_glow(150, slot * 12 + 65, rows[slot].name, 10)
	var press := TyrianText.L("misc_press_key")
	await _out_text_glow(TyrianText.font_center(press, SMALL_FONT), 180, press, 10)
	present()
	if _frame_count_max != 0:
		await _wait_any_button()
	await fade_black(15)


## fonthand.c:JE_outTextGlow (TINY_FONT, glow up to `brightness`).
func _out_text_glow(x: int, y: int, s: String, brightness: int) -> void:
	var c := 7 if warning_red else 15
	for d in [[-1, 0], [0, -1], [1, 0], [0, 1]]:
		TyrianText.out_text_adjust(self, x + d[0], y + d[1], s, 0, -12, SMALL_FONT, false)
	if _frame_count_max > 0:
		for z in range(1, 13):
			TyrianText.out_text_adjust(self, x, y, s, c, z - 10, SMALL_FONT, false)
			if _any_button():
				_frame_count_max = 0
			present()
			await wait_units(_frame_count_max)
	for z in range(6 if _frame_count_max == 0 else 12, brightness - 1, -1):
		TyrianText.out_text_adjust(self, x, y, s, c, z - 10, SMALL_FONT, false)
		if _any_button():
			_frame_count_max = 0
		present()
		await wait_units(_frame_count_max)


# --- Inputs and utilities -------------------------------------------------------


func _unhandled_input(event: InputEvent) -> void:
	TyrianMouse.note(event)
	if not name_entry:
		return
	if TyrianMouse.is_button_press(event):
		_click = event.position
		return
	var k := event as InputEventKey
	if k != null and k.pressed and not k.echo:
		if k.keycode in [KEY_BACKSPACE, KEY_DELETE] or TyrianControls.menu_action(event) == TyrianControls.DELETE:
			_name_keys.append(TyrianControls.DELETE)
		elif k.keycode == KEY_ESCAPE:
			_name_keys.append(TyrianControls.BACK)
		elif k.keycode in [KEY_ENTER, KEY_KP_ENTER]:
			_name_keys.append(TyrianControls.ACCEPT)
		elif k.unicode >= 32 and k.unicode < 127:
			_name_keys.append(char(k.unicode))
		return
	# joystick.c:push_joysticks_as_keyboard: the gamepad pushes Enter / Esc.
	if event is InputEventJoypadButton and event.pressed:
		var action := TyrianControls.menu_action(event)
		if action in [TyrianControls.ACCEPT, TyrianControls.BACK]:
			_name_keys.append(action)


func _any_button() -> bool:
	var seen := _input_seen
	_input_seen = false
	return seen


## wait_input / "while (!JE_anyButton())": a key, a click or a button.
func _wait_any_button() -> void:
	idle = true
	_input_seen = false
	if not Capture.active:
		while is_inside_tree() and not _any_button():
			await get_tree().process_frame
	idle = false


func _cash(player: int) -> int:
	if player == 0:
		return int(Game.loadout[10]) if Game.loadout.size() > 10 else 0
	return int(Game.campaign_modes_extra[1]) if Game.campaign_modes_extra.size() >= 2 else 0


func _mutators() -> int:
	# C mut_* globals: Setup mutators (full game) or of the launched mode (B-03).
	return (int(Game.modes[6]) if Game.modes.size() >= 7 else 0) | Game.mutators


## level_load.c (case 'Q', B-05): mutators note on the end-of-episode score
## page (that is where the C code displays it: JE_displayText of levelWarningText, not
## in the level summary). Empty without mutators, "<label> x1.50" if the
## multiplier exceeds 1, the label alone otherwise.
static func mutator_notice(muts: int) -> String:
	if muts == 0:
		return ""
	var mult := HighScores.mutator_multiplier(muts)
	var label := TyrianText.L("mut_score_notice")
	return "%s x%.2f" % [label, mult] if mult > 1.0 else label


## mutators.c:mutators_score_multiplier (see HighScores.mutator_multiplier).
func _mutator_multiplier(muts: int) -> float:
	return HighScores.mutator_multiplier(muts)


func _atoi(s: String) -> int:
	var t := s.strip_edges()
	var end := 0
	while end < t.length() and (t[end].is_valid_int() or (end == 0 and t[end] == "-")):
		end += 1
	return int(t.left(end)) if end > 0 else 0


func _now_ms() -> int:
	return Time.get_ticks_msec()


func _units_ms(units: int) -> int:
	return int(units * DELAY_PERIOD * 1000.0)
