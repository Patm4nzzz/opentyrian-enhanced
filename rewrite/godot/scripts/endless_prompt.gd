## Endless choice screens rendered in 8-bit on TyrianScreen (ADR 0020):
##
## - "resume": endless.c:endless_resume_prompt, offered by menus.c
##   (MENU_ITEM_1/2_PLAYER_ENDLESS) before endless_start when a game is
##   saved. ENTER resumes (endless_resume), ESCAPE starts a new game.
## - "pick": anomalies_ui.c:anomaly_ui_pick_modal, drawing one card out of
##   three (anomaly_pick_three) between two waves; choice is mandatory.
##
## Fonts: the C's TINY_FONT = SMALL_FONT, SMALL_FONT_SHAPES = NORMAL_FONT
## (same mapping as endless_end.gd). S_ITEM sound of the choice not played.
##
## The mode is passed through EndlessSession.prompt_mode before goto_scene.
extends TyrianScreen

## anomalies_ui.c: CARD_W, CARD_H, CARD_GAP, CARDS_Y0, NAME_BAND_Y/H.
const CARD_W := 96
const CARD_H := 128
const CARD_GAP := 6
const CARDS_Y0 := 38
const NAME_BAND_Y := 13
const NAME_BAND_H := 22
## anomalies_ui.c: PANEL_FILL, PANEL_ACCENT, PANEL_BORDER.
const PANEL_FILL := 228
const PANEL_ACCENT := 227
const PANEL_BORDER := 0
## Frame of the selected card: thickness and color (white of palette 0).
const SELECTION_FRAME := 2
const SELECTION_COLOR := 15
## anomalies_ui.c: rank_color, rank_colorbank, rank_brightness (bronze, silver, gold).
const RANK_COLOR := [0x1A, 0x0C, 0x7C]
const RANK_BANK := [1, 0, 7]
const RANK_BRIGHT := [4, 5, 6]
const RANK_KEY := ["anom_rank_bronze", "anom_rank_silver", "anom_rank_gold"]

## Last menu action received (&"": none; &"other": key without an action).
var _key: StringName = &""
## True when the screen is waiting for a key (capture tool).
var idle := false
## Result (test tool): true = resume / index of the chosen card.
var result: Variant = null
## Draw in progress: number of cards offered and card under the pointer.
var _pick_count := 0
var _pick_cursor := 0
## A hover changed the selected card: wakes the wait to redraw.
var _wake := false


func _ready() -> void:
	_run.call_deferred()


## Shared InputMap actions (ADR 0120); A / D of the card choice = the
## player 2 directions (tyrian_p2_left / right, ADR 0051).
func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouse:
		handle_mouse(event)
		return
	if not event.is_pressed() or event.is_echo():
		return
	var action := TyrianControls.menu_action(event)
	if action == &"" and event.is_action_pressed(TyrianControls.action_name(2, "left")):
		action = TyrianControls.LEFT
	elif action == &"" and event.is_action_pressed(TyrianControls.action_name(2, "right")):
		action = TyrianControls.RIGHT
	if action == &"" and event is InputEventKey:
		action = &"other"
	if action != &"":
		_key = action


## Mouse (rewrite addition: the C of endless.c / anomalies_ui.c only reads
## keys). On the draw, the pointer chooses: hover highlights
## the card under the pointer, a left click confirms THAT card
## — the choice stays mandatory, no click cancels it (the C has no
## cancel). On the resume screen, left click = resume, right click =
## new game. Wheel = left / right, like the other menus of the
## rewrite.
func handle_mouse(event: InputEvent) -> void:
	TyrianMouse.note(event)
	if TyrianMouse.is_wheel(event):
		return
	if _pick_count > 0 and event is InputEventMouseMotion:
		# Hover only on real movement (mouseInactive false), like
		# the C menus.
		if not TyrianMouse.inactive and event.relative != Vector2.ZERO:
			var i := _card_at(event.position)
			if i >= 0 and i != _pick_cursor:
				_pick_cursor = i
				_wake = true
		return
	if not TyrianMouse.is_button_press(event):
		return
	if _pick_count > 0:
		# Left click: confirms the card under the pointer; elsewhere, nothing.
		if event.button_index == MOUSE_BUTTON_LEFT:
			var i := _card_at(event.position)
			if i >= 0:
				_pick_cursor = i
				_key = TyrianControls.ACCEPT
		return
	_key = TyrianControls.BACK if event.button_index == MOUSE_BUTTON_RIGHT else TyrianControls.ACCEPT


## Rank of the draw card under point `p` (320×200 coordinates), -1
## otherwise. Same geometry as the `pick` drawing.
func _card_at(p: Vector2) -> int:
	var n := _pick_count
	if n <= 0 or p.y < CARDS_Y0 or p.y >= CARDS_Y0 + CARD_H:
		return -1
	var x0 := (320 - (n * CARD_W + (n - 1) * CARD_GAP)) / 2
	for i in n:
		var cx := x0 + i * (CARD_W + CARD_GAP)
		if p.x >= cx and p.x < cx + CARD_W:
			return i
	return -1


func _wait_key() -> StringName:
	_key = &""
	_wake = false
	idle = true
	while _key == &"" and not _wake and not Capture.active:
		await get_tree().process_frame
	idle = false
	return _key


func _run() -> void:
	if EndlessSession.prompt_mode == "":
		_prepare_from_cmdline()
	if EndlessSession.prompt_mode == "pick":
		var card: int = await pick(EndlessSession.prompt_picks)
		if not Capture.active:
			EndlessSession.after_pick(card)
	else:
		var resume: bool = await resume_prompt(EndlessSession.resume_info())
		if not Capture.active:
			EndlessSession.after_resume_prompt(resume)


## Direct capture (--capture=endless_prompt:... --endless-prompt=pick|resume):
## new game (seed Game.rng_seed), wave 1 draw or save.
func _prepare_from_cmdline() -> void:
	var mode := "resume"
	for a in OS.get_cmdline_user_args():
		if a.begins_with("--endless-prompt="):
			mode = a.trim_prefix("--endless-prompt=")
	EndlessSession.start(false, OS.get_cmdline_user_args().has("--endless2"))
	EndlessSession.prompt_mode = mode
	if mode == "pick":
		EndlessSession.prompt_picks = EndlessSession.run.pick_three(1)
	else:
		EndlessSession.save_resume()


## endless.c:endless_resume_prompt: black screen (JE_clr256), title
## FONT_SHAPES centered y = 60, "Wave %d - Score %lu" (+ "(2 players)")
## TINY_FONT centered y = 100 (15, 4), help y = 130 (14, 4), FULL_SHADE.
func resume_prompt(info: Dictionary) -> bool:
	vga.clear(VGA_SCREEN, 0)
	var title := TyrianText.L("endless_resume_title")
	TyrianText.dstring(self, TyrianText.font_center(title, LARGE_FONT), 60, title, LARGE_FONT)
	var line := TyrianText.L("endless_resume_fmt").replace("%d", str(int(info.get("levels_completed", 0)) + 1)).replace(
		"%lu", str(int(info.get("score", 0)))
	)
	if bool(info.get("two_player", false)):
		line += "  " + TyrianText.L("endless_resume_2p")
	TyrianText.text_shade(self, TyrianText.font_center(line, SMALL_FONT), 100, line, 15, 4, TyrianText.FULL_SHADE)
	_draw_deck_initials(info.get("deck", []), 112)
	var hint := TyrianText.L("endless_resume_hint")
	TyrianText.text_shade(self, TyrianText.font_center(hint, SMALL_FONT), 130, hint, 14, 4, TyrianText.FULL_SHADE)
	present()
	await fade_palette(10, 0, 255)
	idle = Capture.active
	var resume := false
	while not Capture.active:
		var k := await _wait_key()
		if k == TyrianControls.ACCEPT or k == TyrianControls.SELECT:
			resume = true
			break
		if k == TyrianControls.BACK:
			break
	result = resume
	if not Capture.active:
		await fade_black(10)
	return resume


## Label then initials of the deck cards (rank color), centered on line y.
func _draw_deck_initials(cards: Variant, y: int) -> void:
	var letters := EndlessSession.deck_initials(cards)
	if letters.is_empty():
		return
	var label := TyrianText.L("endless_deck_label")
	var total := TyrianText.width(label, SMALL_FONT)
	for l: Array in letters:
		total += 6 + TyrianText.width(l[0], SMALL_FONT)
	var x := (320 - total) / 2
	TyrianText.text_shade(self, x, y, label, 15, 4, TyrianText.FULL_SHADE)
	x += TyrianText.width(label, SMALL_FONT)
	for l: Array in letters:
		x += 6
		TyrianText.text_shade(self, x, y, l[0], RANK_BANK[l[1]], RANK_BRIGHT[l[1]], TyrianText.FULL_SHADE)
		x += TyrianText.width(l[0], SMALL_FONT)


## anomalies_ui.c:anomaly_ui_pick_modal: image 1, palette 0, two full-screen
## JE_barShade, title FONT_SHAPES y = 14, centered cards
## (draw_card), help TINY_FONT (SMALL_FONT here) below the cards (14, 6). Left/right,
## ENTER chooses; no cancel.
func pick(cards: PackedInt32Array) -> int:
	if cards.is_empty():
		return -1
	_pick_count = cards.size()
	_pick_cursor = 0
	var fadein := true
	while true:
		vga.load_pic(VGA_SCREEN, 1, false)
		vga.load_palette(0)
		vga.bar_shade(VGA_SCREEN, 0, 0, 319, 199)
		vga.bar_shade(VGA_SCREEN, 0, 0, 319, 199)
		var title := TyrianText.L("anom_pick_title")
		TyrianText.dstring(self, (320 - TyrianText.width(title, LARGE_FONT)) / 2, 14, title, LARGE_FONT)
		var n := cards.size()
		var x0 := (320 - (n * CARD_W + (n - 1) * CARD_GAP)) / 2
		for i in n:
			_draw_card(cards[i], x0 + i * (CARD_W + CARD_GAP), CARDS_Y0, i == _pick_cursor)
		var hint := TyrianText.L("anom_pick_hint")
		TyrianText.text_shade(
			self,
			(320 - TyrianText.width(hint, SMALL_FONT)) / 2,
			CARDS_Y0 + CARD_H + 10,
			hint,
			14,
			6,
			TyrianText.FULL_SHADE
		)
		present()
		if fadein:
			await fade_palette(10, 0, 255)
			fadein = false
		if Capture.active:
			idle = true
			break
		var k := await _wait_key()
		if k == TyrianControls.LEFT and _pick_cursor > 0:
			_pick_cursor -= 1
		elif k == TyrianControls.RIGHT and _pick_cursor < n - 1:
			_pick_cursor += 1
		elif k == TyrianControls.ACCEPT or k == TyrianControls.SELECT:
			break
	result = cards[_pick_cursor]
	_pick_count = 0
	if not Capture.active:
		await fade_black(10)
	return cards[_pick_cursor]


## anomalies_ui.c:draw_card (simplified: name on one SMALL_FONT line, without
## the two-line split of name_layout; description in JE_helpBox).
func _draw_card(card: int, x: int, y: int, selected: bool) -> void:
	var id := str(TyrianEndless.card_id(card))
	var rank := int(TyrianEndless.card_rank(card))
	vga.fill_rectangle_xy(VGA_SCREEN, x, y, x + CARD_W - 1, y + CARD_H - 1, PANEL_FILL)
	vga.fill_rectangle_xy(VGA_SCREEN, x, y, x + CARD_W - 1, y, PANEL_BORDER)
	vga.fill_rectangle_xy(VGA_SCREEN, x, y + CARD_H - 1, x + CARD_W - 1, y + CARD_H - 1, PANEL_BORDER)
	vga.fill_rectangle_xy(VGA_SCREEN, x, y, x, y + CARD_H - 1, PANEL_BORDER)
	vga.fill_rectangle_xy(VGA_SCREEN, x + CARD_W - 1, y, x + CARD_W - 1, y + CARD_H - 1, PANEL_BORDER)
	vga.fill_rectangle_xy(VGA_SCREEN, x + 2, y + CARD_H - 3, x + CARD_W - 3, y + CARD_H - 3, PANEL_ACCENT)
	vga.fill_rectangle_xy(VGA_SCREEN, x + 3, y + 3, x + CARD_W - 4, y + 9, RANK_COLOR[rank])
	vga.fill_rectangle_xy(VGA_SCREEN, x + 3, y + 10, x + CARD_W - 4, y + 10, PANEL_ACCENT)
	var name := TyrianText.L("anom_%s_name" % id)
	# Same font for all names: choosing by width mixed two sizes in a single draw.
	var font := SMALL_FONT
	TyrianText.text_shade(
		self,
		x + maxi((CARD_W - TyrianText.width(name, font)) / 2, 2),
		y + NAME_BAND_Y + 6,
		name,
		15,
		6 if selected else 4,
		TyrianText.FULL_SHADE
	)
	var cur_y := y + NAME_BAND_Y + NAME_BAND_H + 1
	var rank_lbl := TyrianText.L(RANK_KEY[rank])
	TyrianText.text_shade(
		self,
		x + (CARD_W - TyrianText.width(rank_lbl, SMALL_FONT)) / 2,
		cur_y,
		rank_lbl,
		RANK_BANK[rank],
		RANK_BRIGHT[rank],
		TyrianText.FULL_SHADE
	)
	cur_y += 11
	TyrianText.help_box(
		self,
		x + 4,
		cur_y,
		TyrianText.L("anom_%s_desc" % id),
		14,
		15 if selected else 14,
		6 if selected else 4,
		TyrianText.DARKEN
	)
	if selected:
		# Light 2 px frame in the gap between cards, to see the selected card (keyboard and mouse).
		var left := x - SELECTION_FRAME
		var right := x + CARD_W + SELECTION_FRAME - 1
		var top := y - SELECTION_FRAME
		var bottom := y + CARD_H + SELECTION_FRAME - 1
		vga.fill_rectangle_xy(VGA_SCREEN, left, top, right, top + SELECTION_FRAME - 1, SELECTION_COLOR)
		vga.fill_rectangle_xy(VGA_SCREEN, left, bottom - SELECTION_FRAME + 1, right, bottom, SELECTION_COLOR)
		vga.fill_rectangle_xy(VGA_SCREEN, left, top, left + SELECTION_FRAME - 1, bottom, SELECTION_COLOR)
		vga.fill_rectangle_xy(VGA_SCREEN, right - SELECTION_FRAME + 1, top, right, bottom, SELECTION_COLOR)
