## End of an Endless game (endless.c:endless_on_level_end, branch
## "lives_exhausted"): name entry if the score makes the leaderboard
## (prompt_endless_name), summary (endless_show_run_summary), leaderboard
## (endless_leaderboard_show), 8-bit rendering on TyrianScreen (ADR 0020).
##
## Base score = s_score, final score = endless_apply_score_mult(deck × mutators). Summary: "anomaly
## bonus" row and list of carried cards; leaderboard: "2P" and "*" marks and
## legend ported. Entry date: the C writes the local date; here too, YYYY-MM-DD format.
## Retry "R" (ENTER starts a new game, ESCAPE goes to the title).
extends TyrianScreen

const ENDLESS_NAME_MAX := 16
const DEV_CONSOLE := preload("res://scripts/dev_console.gd")

var _typed := ""
## Last menu action received (&"": none; &"other": key without an action).
var _key: StringName = &""
## True when the screen is waiting for a key (capture tool).
var idle := false


func _ready() -> void:
	# endless.c (name entry, summary, leaderboard) does not call
	# JE_mouseStart: no pointer; clicks are still accepted.
	mouse_cursor = false
	_run.call_deferred()


func _unhandled_key_input(event: InputEvent) -> void:
	var k := event as InputEventKey
	if k == null or not k.pressed or k.echo:
		return
	var action := TyrianControls.menu_action(event)
	_key = action if action != &"" else &"other"
	# Name entry: the typed character, whatever the action.
	if k.unicode >= 32 and k.unicode < 127:
		_typed += char(k.unicode).to_upper()


## joystick.c:push_joysticks_as_keyboard, called by prompt_endless_name,
## endless_show_run_summary and the leaderboard: the gamepad pushes RETURN
## (confirm) and ESCAPE (cancel); its directions, arrows, have
## no effect on these screens.
func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventJoypadButton) or not event.pressed:
		return
	var action := TyrianControls.menu_action(event)
	if action in [TyrianControls.ACCEPT, TyrianControls.BACK]:
		_key = action


## Mouse (rewrite addition): left click = confirm, right click =
## cancel, like Enter / Escape.
func handle_mouse(event: InputEvent) -> void:
	if not TyrianMouse.is_button_press(event):
		return
	_key = TyrianControls.BACK if event.button_index == MOUSE_BUTTON_RIGHT else TyrianControls.ACCEPT


func _wait_key() -> StringName:
	_key = &""
	idle = true
	while _key == &"" and not Capture.active:
		await get_tree().process_frame
	idle = false
	return _key


func _run() -> void:
	var board := EndlessSession.load_board()
	var s := EndlessSession.final_score
	# endless.c: "if (!constantPlay && endless_qualifies(final_score_q))".
	# endless.c: "if (!constantPlay && endless_qualifies(final_score_q))" (ENH-16).
	if not DEV_CONSOLE.god_mode and EndlessSession.qualifies(s, board):
		var name := await _prompt_name(s)
		board = (
			EndlessSession
			. insert_score(
				board,
				{
					"name": name,
					"score": s,
					"wave": EndlessSession.final_wave,
					"players": 2 if EndlessSession.final_two_player else 1,
					"mutators": 1 if EndlessSession.mutators != 0 else 0,
					"date": Time.get_date_string_from_system(),
				}
			)
		)
		# endless.c:endless_submit_score: achievements_unlock(ACH_ARCADE_ENTRY).
		if ClassDB.class_exists("TyrianVga"):
			Game.ach_unlocked_outside_level(
				ClassDB.class_call_static("TyrianVga", "ach_unlock_by_name", Game.ach_dat_path(), "ARCADE_ENTRY")
			)
	await _summary(s)
	await _leaderboard(board)


## endless.c:prompt_endless_name: image 2, title FONT_SHAPES y = 20, wave and
## score SMALL_FONT_SHAPES y = 50, prompt TINY_FONT y = 80, field JE_barShade
## (60, 95)-(260, 115), text JE_outText (70, 100, 8, 3), blinking cursor
## (8*16+10 / 8*16+2), help y = 140. Default "PLAYER" (ESCAPE or empty name).
func _prompt_name(final_score: int) -> String:
	var name := ""
	var flash := 8 * 16 + 10
	var fadein := true
	while true:
		vga.load_pic(VGA_SCREEN, 2, false)
		var title := TyrianText.L("endless_new_high_score")
		TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 20, title, LARGE_FONT)
		var buf := TyrianText.L("endless_wave_score_fmt").replace("%d", str(EndlessSession.final_wave)).replace(
			"%lu", str(final_score)
		)
		TyrianText.text_shade(self, TyrianText.font_center(buf, NORMAL_FONT), 50, buf, 15, 4, TyrianText.FULL_SHADE)
		var prompt := TyrianText.L("endless_enter_name")
		TyrianText.text_shade(
			self, TyrianText.font_center(prompt, SMALL_FONT), 80, prompt, 15, 4, TyrianText.FULL_SHADE
		)
		vga.bar_shade(VGA_SCREEN, 60, 95, 260, 115)
		flash = 8 * 16 + 2 if flash == 8 * 16 + 10 else 8 * 16 + 10
		TyrianText.out_text(self, 70, 100, name, 8, 3)
		var cx := 70 + TyrianText.width(name, SMALL_FONT)
		vga.fill_rectangle_xy(VGA_SCREEN, cx + 1, 100, cx + 5, 108, flash)
		var hint := TyrianText.L("endless_enter_esc_hint")
		TyrianText.text_shade(self, TyrianText.font_center(hint, SMALL_FONT), 140, hint, 14, 2, TyrianText.FULL_SHADE)
		present()
		if fadein:
			await fade_palette(15, 0, 255)
			fadein = false
		if Capture.active:
			break
		# 12 × setDelay(1), interrupted by a key.
		_key = &""
		_typed = ""
		for i in 12:
			await wait_units(1)
			if _key != &"":
				break
		for c in _typed:
			if name.length() < ENDLESS_NAME_MAX - 1 and (c == " " or TyrianText.sprite_id(c.unicode_at(0)) >= 0):
				name += c
		if _key == TyrianControls.DELETE and name.length() > 0:
			name = name.substr(0, name.length() - 1)
		elif _key == TyrianControls.ACCEPT:
			# endless.c:prompt_endless_name: "if (name_len > 0) quit = true" (ENH-07):
			# Enter with an empty name is ignored.
			if name != "":
				break
		elif _key == TyrianControls.BACK:
			name = ""
			break
	await fade_black(10)
	return name if name != "" else "PLAYER"


## endless.c:endless_show_run_summary: image 1, two JE_barShade, title
## FONT_SHAPES y = 12, rows from y = 44 (label x = 60, value
## right-aligned at x = 260, spacing 12), help centered y = 188.
func _summary(final_score: int) -> void:
	# Image 2 (like the leaderboard): image 1 let the shop's frames and icons show through.
	vga.load_pic(VGA_SCREEN, 2, false)
	vga.load_palette(0)
	vga.bar_shade(VGA_SCREEN, 0, 0, 319, 199)
	vga.bar_shade(VGA_SCREEN, 0, 0, 319, 199)
	var title := TyrianText.L("endless_summary_title")
	TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 12, title, LARGE_FONT)
	var y := 44
	var lines := [
		["endless_summary_waves", str(EndlessSession.final_wave - 1)],
		["endless_summary_base", str(EndlessSession.final_base)]
	]
	# endless.c:endless_show_run_summary: "anomaly bonus" row if the percentage is > 0 (ENH-06).
	if EndlessSession.final_bonus_pct > 0:
		lines.append(["endless_summary_bonus", "x%s" % str(1.0 + EndlessSession.final_bonus_pct / 100.0)])
	lines.append(["endless_summary_final", str(final_score)])
	for line in lines:
		TyrianText.text_shade(self, 60, y, TyrianText.L(line[0]), 15, 4, TyrianText.FULL_SHADE)
		TyrianText.text_shade(
			self, 260 - TyrianText.width(line[1], SMALL_FONT), y, line[1], 14, 4, TyrianText.FULL_SHADE
		)
		y += 12
	# List of active cards, one name per row at x = 68, spacing 10.
	y += 6
	if not EndlessSession.final_deck.is_empty():
		TyrianText.text_shade(self, 60, y, TyrianText.L("endless_summary_anomalies"), 15, 4, TyrianText.FULL_SHADE)
		y += 12
		for card in EndlessSession.final_deck:
			var id := str(TyrianEndless.card_id(card))
			TyrianText.text_shade(self, 68, y, TyrianText.L("anom_%s_name" % id), 14, 4, TyrianText.FULL_SHADE)
			y += 10
	var hint := TyrianText.L("endless_summary_hint")
	TyrianText.text_shade(self, (320 - TyrianText.width(hint, SMALL_FONT)) / 2, 188, hint, 14, 6, TyrianText.FULL_SHADE)
	present()
	await fade_palette(10, 0, 255)
	while true:
		var k := await _wait_key()
		if Capture.active or k == TyrianControls.ACCEPT or k == TyrianControls.BACK:
			break
	await fade_black(10)


## endless.c:endless_leaderboard_show: image 2, title FONT_SHAPES y = 10,
## columns 20/50/130/170/230, headers y = 40, rows from y = 54
## (spacing 11), "no score" at (100, 100), help y = 183 and 190.
## ENTER or R: new game (endless_try_retry); ESCAPE: title.
func _leaderboard(board: Array) -> void:
	vga.load_pic(VGA_SCREEN, 2, false)
	var title := TyrianText.L("endless_leaderboard_title")
	TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 10, title, LARGE_FONT)
	var cols := [20, 50, 130, 170, 230]
	var y := 40
	for i in 5:
		TyrianText.text_shade(
			self,
			cols[i],
			y,
			TyrianText.L(
				["endless_col_rank", "endless_col_name", "endless_col_wave", "endless_col_score", "endless_col_date"][i]
			),
			15,
			4,
			TyrianText.FULL_SHADE
		)
	y += 14
	var any_mutators := false
	for i in board.size():
		var e: Dictionary = board[i]
		var nm := str(e.get("name", ""))
		var shown := EndlessSession.shorten_name(nm, 13)
		# endless.c:endless_leaderboard_show: "*" (mutators) and "2P" mark (ENH-05).
		var mut := int(e.get("mutators", 0)) != 0
		if mut:
			any_mutators = true
			shown = EndlessSession.shorten_name(nm, 11) + " *"
		if int(e.get("players", 1)) == 2:
			var mark := TyrianText.L("endless_two_player_mark")
			shown = (EndlessSession.shorten_name(nm, 7) + " *") if mut else EndlessSession.shorten_name(nm, 9)
			TyrianText.text_shade(
				self, cols[2] - 4 - TyrianText.width(mark, SMALL_FONT), y, mark, 12, 4, TyrianText.FULL_SHADE
			)
		var cells := [
			"%2d." % (i + 1), shown, str(int(e.get("wave", 0))), str(int(e.get("score", 0))), str(e.get("date", ""))
		]
		for c in 5:
			TyrianText.text_shade(self, cols[c], y, cells[c], 14, 4, TyrianText.FULL_SHADE)
		y += 11
	if board.is_empty():
		TyrianText.text_shade(self, 100, 100, TyrianText.L("endless_no_scores"), 12, 4, TyrianText.FULL_SHADE)
	if any_mutators:
		TyrianText.text_shade(
			self, cols[1], y + 3, "* = %s" % TyrianText.L("mut_score_legend"), 12, 4, TyrianText.FULL_SHADE
		)
	# Help centered, 10 px apart.
	var retry_hint := TyrianText.L("endless_retry_hint")
	var return_hint := TyrianText.L("endless_return_hint")
	TyrianText.text_shade(
		self, TyrianText.font_center(retry_hint, SMALL_FONT), 180, retry_hint, 14, 4, TyrianText.FULL_SHADE
	)
	TyrianText.text_shade(
		self, TyrianText.font_center(return_hint, SMALL_FONT), 190, return_hint, 15, 4, TyrianText.FULL_SHADE
	)
	present()
	await fade_palette(10, 0, 255)
	if Capture.active:
		return
	while true:
		var k := await _wait_key()
		if k == TyrianControls.ACCEPT or k == TyrianControls.RETRY:
			# tyrian2.c:201: fade_song(); fade_black(10) on leaving the end screens.
			Music.fade_song()
			await fade_black(10)
			# endless_try_retry: endless_start(s_last_two_player).
			EndlessSession.start(true, EndlessSession.final_two_player)
			return
		if k == TyrianControls.BACK:
			Music.fade_song()
			await fade_black(10)
			Game.goto_scene(Game.SCENE_TITLE)
			return
