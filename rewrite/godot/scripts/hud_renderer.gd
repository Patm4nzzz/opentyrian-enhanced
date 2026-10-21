## In-game dashboard drawn pixel-perfect (ADR 0020).
##
## The C draws the HUD into `VGAScreenSeg` (320×200 surface whose play area
## is overwritten every frame by `game_screen`) with incremental
## calls: the bar is only redrawn when a value changes.
## This class replays those calls in a level-specific `TyrianVga`, in
## the same order, from the state published by the simulation
## (`TyrianSim.hud_state`):
##   - tyrian2.c:JE_main (level start): JE_loadPic(3, or 6 in two-player
##     mode), JE_drawOptions, JE_outText(levelName),
##     JE_drawTextWindow(miscText[20]), JE_drawShield/JE_drawArmor,
##     JE_drawPortConfigButtons;
##   - tyrian2.c (loop, "draw HUD"): clearing of the message area
##     (textErase), "Weapon Display", "Power Bar";
##   - drawing done in the middle of the simulation, in their order
##     (`TyrianSim.take_hud_ops`): shield and armor bars (recharge,
##     damage, respawn, bonus, special weapon), cells of player 2's charge
##     gauge, JE_drawOptions; player_movement.c (sidekick
##     ammo);
##   - mainint.c:JE_inGameDisplays: money, special weapon, lives, player
##     labels, super bombs, drawn over the play area (transparent
##     layer elsewhere).
## The message of an event 16 (level_events.c:JE_drawTextWindow) arrives through
## a `take_hud_ops` record (type 3).
class_name HudRenderer
extends Sprite2D

const SEG := 0
const OVER_A := 1
const OVER_B := 2
const OPTION_SHAPES := 5
## Play area (game_screen) overwriting VGAScreenSeg.
const PLAY_W := 264
## enemy_render.c: game_screen (x 24..287) copied to x 0..263 of VGAScreenSeg;
## JE_inGameDisplays draws into game_screen, hence shifted by 24 px on screen.
const PLAY_AREA_X := 24
const PLAY_H := 184
## varz.h:hud_sidekick_y (one player, two players).
const SIDEKICK_Y := [[64, 82], [108, 126]]
## tyrian2.c:JE_main: JE_loadPic(twoPlayerMode ? 6 : 3).
const PIC_ONE_PLAYER := 3
const PIC_TWO_PLAYERS := 6

## Indices of TyrianSim.hud_state().
enum {
	SHIELD,
	SHIELD_MAX,
	ARMOR,
	POWER,
	WPOWER0,
	WPOWER1,
	WEAPON_MODE,
	SPECIAL_GR,
	CASH,
	SK0_ICON,
	SK0_AMMO,
	SK0_AMMO_MAX,
	SK1_ICON,
	SK1_AMMO,
	SK1_AMMO_MAX,
	SUPERBOMBS,
	TWO_PLAYER,
	ONE_PLAYER_ACTION,
	GALAGA,
	SMOOTHIE6,
	SHIELD2,
	ARMOR2,
	CASH2,
	SUPERBOMBS2,
	LIVES1,
	LIVES2,
	SIDEKICK_LEVEL2,
	TOP_TEXT_BOTTOM
}
## Size of a TyrianSim.take_hud_ops() record.
const OP_SIZE := 8

var vga: RefCounted
var _texture: ImageTexture
## HD (AI) mode: upscaled dashboard frame (HdInterface); the play
## area, taken from the witness surfaces, is excluded from it.
var _hd: HdInterface
var _prev := PackedInt32Array()
var _old_weapon_bar := [0, 0]
var _last_power := 0
var _text_erase := 0
## message_bar.c: queue of the text area messages (4 slots,
## 100 frames each), [text, priority, remaining frames]; -1 = none active.
const MESSAGE_DURATION_FRAMES := 100
const MSG_PRIORITY_COMBO := 1
const MSG_PRIORITY_SYSTEM := 2
var _messages: Array = [["", 0, 0], ["", 0, 0], ["", 0, 0], ["", 0, 0]]
var _active_message := -1
## "Achievement: ..." banner (achievements.c:achievements_draw_notification),
## empty if none; set by level.gd after each tick.
var notification := ""
## Banner displayed at this frame, wherever it is drawn (here, or in
## the game image in Original mode): the slow-motion indicator is placed below it.
var banner_shown := false
## True during the pause menu or help: the "Achievement" banner is not
## drawn (it would be frozen into the image under the menu).
var banner_suppressed := false
## message_bar.c:message_bar_pin: text placed in front of the queue (quick
## load question), empty = none.
var pinned_message := ""
## F3 / F4 / F6 overlays (level_overlays.gd) and the simulation that provides the figures.
var overlays: LevelOverlays
var overlay_sim: Node
## enemy_render.c: "PLAY" at the top right of the play area during a
## replay (JE_textShade(VGAScreenSeg, 225, 10, ..., 15, 4)); empty otherwise.
var replay_label := ""
const REPLAY_LABEL_X := 225
const REPLAY_LABEL_Y := 10
## "REC" badge (F10), top right corner of the play area, in place of
## the replay label (never both together); the left corner already carries
## "Player 1".
const REC_MARKER_X := 238
const REC_MARKER_TEXT := "REC"  # locale-exempt: universal acronym
## "REC" while recording (enemy_render.c: x = 233, color 4/4)
## instead of "PLAY" (x = 225, color 15/4): position and color of the label.
var replay_label_x := REPLAY_LABEL_X
var replay_label_color := 15
## mainint.c:JE_pauseGame: "PAUSED" (miscText[22]) in FONT_SHAPES at (120, 90)
## of VGAScreenSeg, by JE_dString (tint 15 of the level palette); empty outside pause.
var pause_text := ""
const PAUSE_TEXT_X := 120
const PAUSE_TEXT_Y := 90
## Maximum height of a glyph of the large font (main_00_font: 20 px),
## shadow not included.
const PAUSE_TEXT_H := 20
const FONT_SHAPES := 0
## achievements.c: ACH_NOTIFY_PLAY_W, ACH_NOTIFY_Y (VGAScreenSeg coordinates).
## Deliberate divergence from the original (fixed x 60): banner centered in
## the play area, whatever the name and language.
const ACH_NOTIFY_PLAY_W := 264
const ACH_NOTIFY_Y := 3


func _init() -> void:
	centered = false
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	if ClassDB.class_exists("TyrianVga"):
		vga = ClassDB.instantiate("TyrianVga")
		ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
		_hd = HdInterface.new(self, vga)
		_hd.skip = Rect2i(0, 0, PLAY_W, PLAY_H)


## Level start (tyrian2.c:JE_main, before the loop). Returns false if
## the state published by the simulation is incomplete.
func start(sim: Node, level_name: String) -> bool:
	if vga == null or sim == null:
		return false
	var h: PackedInt32Array = sim.hud_state()
	if h.size() <= TOP_TEXT_BOTTOM:
		return false
	sim.take_hud_ops()  # drawing from before the loop: redone below
	var two := h[TWO_PLAYER] != 0
	# JE_loadPic(VGAScreen, 3 or 6) then fade_palette(colors).
	vga.load_pic(SEG, PIC_TWO_PLAYERS if two else PIC_ONE_PLAYER, true)
	_draw_options(h)
	vga.out_text(SEG, 268, 76 if two else 118, level_name, 12, 4)
	# power = 0 at the start (tyrian2.c): lastPower = 0, weapon bars at 0.
	_last_power = 0
	_old_weapon_bar = [0, 0]
	_draw_text_window(TyrianText.L("miscText_20"))
	_draw_shield(h)
	_draw_armor(h)
	_draw_port_config_buttons(h)
	_prev = h
	_set_wave_intro(sim)
	refresh(sim)
	return true


## After each simulation tick: incremental updates of
## VGAScreenSeg in the order of the tyrian2.c loop (the color of the energy
## gauge depends on history: one call per tick, not per frame).
func tick(sim: Node) -> void:
	if vga == null or sim == null or _prev.is_empty():
		return
	var h: PackedInt32Array = sim.hud_state()
	if h.size() <= TOP_TEXT_BOTTOM:
		return
	_apply_ops(sim.take_hud_ops(), h)
	_tick(h)
	_prev = h
	# combos.c:combo_on_kill: "NAME xM" at the tier (message_bar_queue),
	# then tyrian2.c: message_bar_update at the bottom of the loop.
	var combo: PackedInt32Array = sim.combo_state() if sim.has_method("combo_state") else PackedInt32Array()
	if combo.size() >= 3 and combo[2] >= 0:
		var tier: Array = COMBO_TIERS[combo[2]]
		_message_queue("%s x%d" % [TyrianText.L(tier[0]), tier[1]], MSG_PRIORITY_COMBO)
	_message_update()
	_set_wave_intro(sim)


## Endless wave entry banner (2 s of real time since the first
## frame of the level, set by level.gd): darkening shadow on the game
## image, composed by TyrianSim.playfield_background (draw_wave_intro). Set on
## every tick, before the image composition.
var wave_intro_elapsed_ms := 0


func _set_wave_intro(_sim: Node) -> void:
	var lines := wave_intro_lines(wave_intro_elapsed_ms)
	ClassDB.class_call_static("TyrianSim", "set_playfield_wave_intro", lines[0], lines[1])


## Lines of the wave entry banner `elapsed_ms` real ms after the level
## start (empty: none). endless.c:1300 compares with SDL_GetTicks: neither slow
## motion nor pause lengthens it. Nothing under the Eyes Closed anomaly (the C
## exits before the banner), "..." when the level name is empty.
static func wave_intro_lines(elapsed_ms: int) -> PackedStringArray:
	var on: bool = EndlessSession.active() and not EndlessSession.eyes_closed() and elapsed_ms < EndlessSession.INTRO_MS
	if not on:
		return PackedStringArray(["", ""])
	var name := EndlessSession.level_name()
	return PackedStringArray(
		[
			EndlessSession.hud_text(),
			TyrianText.L("endless_entering_fmt").replace("%s", name if not name.is_empty() else "...")
		]
	)


## combos.c:combo_tiers (text key, multiplier), from highest to lowest.
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


## message_bar.c:message_bar_queue (4 slots, ttl 0 = free).
func _message_queue(text: String, priority: int) -> void:
	if _active_message >= 0 and _messages[_active_message][1] < priority:
		# Preemption: the active one keeps its remainder in its slot.
		var slot := _message_free_slot()
		if slot < 0:
			var lowest := _message_lowest_slot()
			slot = lowest if lowest >= 0 and _messages[lowest][1] <= priority else _active_message
		_messages[slot] = [text, priority, MESSAGE_DURATION_FRAMES]
		_active_message = slot
		return
	var slot := _message_free_slot()
	if slot < 0:
		# Queue full: evicts the non-active slot of priority <= the lowest.
		var lowest_pri := priority
		for i in 4:
			if i != _active_message and _messages[i][2] > 0 and _messages[i][1] <= lowest_pri:
				lowest_pri = _messages[i][1]
				slot = i
		if slot < 0:
			return
	_messages[slot] = [text, priority, MESSAGE_DURATION_FRAMES]
	if _active_message < 0:
		_active_message = slot


## message_bar_queue(text, MSG_PRIORITY_SYSTEM): player confirmations
## (recording key), taking priority over combo announcements.
func queue_system_message(text: String) -> void:
	# Deliberate divergence from the C: a new system message (player
	# action) replaces the one displayed instead of waiting for its 100 frames;
	# without this F5 kept "Recording starts at next level" for several seconds.
	for i in 4:
		if _messages[i][1] == MSG_PRIORITY_SYSTEM:
			_messages[i] = ["", 0, 0]
			if i == _active_message:
				_active_message = -1
	_message_queue(text, MSG_PRIORITY_SYSTEM)


## Y of ">> SLOW MOTION <<" (enemy_render.c:213): y 22 at minimum,
## under the top text; an achievement banner at `banner_y` (-1: none)
## outside the low position 170 pushes it to banner_y + 9 (achievements.c:1587).
static func slow_motion_y(top_text_bottom: int, banner_y: int) -> int:
	var bottom := top_text_bottom
	if banner_y >= 0 and banner_y != LevelOverlays.NOTIFY_Y_WITH_OVERLAY:
		bottom = banner_y + 9
	return maxi(22, bottom + 2)


func _message_free_slot() -> int:
	for i in 4:
		if _messages[i][2] == 0:
			return i
	return -1


## message_bar.c:find_lowest_priority_slot (never the active slot).
func _message_lowest_slot() -> int:
	var result := -1
	var lowest := 2
	for i in 4:
		if i != _active_message and _messages[i][2] > 0 and _messages[i][1] <= lowest:
			lowest = _messages[i][1]
			result = i
	return result


## message_bar.c:message_bar_update: displays the active message (JE_drawTextWindow)
## or, once expired, the highest-priority one in the queue (then the longest remaining).
func _message_update() -> void:
	var shown := ""
	if _active_message >= 0:
		var m: Array = _messages[_active_message]
		if m[2] > 0:
			shown = m[0]
			m[2] -= 1
		else:
			_messages[_active_message] = ["", 0, 0]
			var best := -1
			for i in 4:
				var q: Array = _messages[i]
				if (
					q[2] > 0
					and (
						best < 0
						or q[1] > _messages[best][1]
						or (q[1] == _messages[best][1] and q[2] > _messages[best][2])
					)
				):
					best = i
			_active_message = best
			if best >= 0:
				shown = _messages[best][0]
				_messages[best][2] -= 1
	if not pinned_message.is_empty():
		shown = pinned_message
	if not shown.is_empty():
		_draw_text_window(shown)


## Replay recording armed (F10 before the level) or in progress.
func _recording_marker_shown(sim: Node) -> bool:
	if ReplayRecording.armed:
		return true
	return sim != null and sim.has_method("is_recording") and sim.is_recording()


## One frame: JE_inGameDisplays then layer composition.
func refresh(sim: Node) -> void:
	if vga == null or sim == null or _prev.is_empty():
		return
	var endless := PackedInt64Array()
	if EndlessSession.active():
		var st: PackedInt64Array = sim.endless_state() if sim.has_method("endless_state") else PackedInt64Array()
		var score := EndlessSession.score() + (st[2] if st.size() >= 3 else 0)
		endless = PackedInt64Array(
			[score, st[0] if st.size() >= 1 else EndlessSession.lives(), st[1] if st.size() >= 2 else 0]
		)
	_draw_in_game_displays(_prev, endless)
	# JE_inGameDisplays draws into game_screen, which JE_starShowVGA then copies
	# through the lamp (code 2): money and icons are darkened too.
	var fx: PackedInt32Array = sim.hd_screen_effects() if sim.has_method("hd_screen_effects") else PackedInt32Array()
	if fx.size() > 4 and fx[2] == 2:
		vga.lamp_overlay(PLAY_AREA_X, fx[3], fx[4])
	# enemy_render.c: DPS, statistics, achievements, then the banner.
	if (
		overlays != null
		and (overlays.is_open() or overlays.dps_active)
		and overlay_sim != null
		and overlay_sim.has_method("ach_stats")
	):
		overlays.draw(vga, overlay_sim.ach_stats(), overlay_sim.ach_unlocked_flags())
	# enemy_render.c: the banner is drawn into VGAScreenSeg after the
	# game_screen copy, so without the 24 px offset of the play area.
	# Deliberate divergence: under the countdown, the boss bar or
	# "Secret Level" instead of covering them (shadow included).
	var notify_y := maxi(ACH_NOTIFY_Y, _prev[TOP_TEXT_BOTTOM] + 2)
	# achievements.c: y 170 when the achievements list or the statistics are open.
	if overlays != null and overlays.is_open():
		notify_y = LevelOverlays.NOTIFY_Y_WITH_OVERLAY
	if not notification.is_empty() and not banner_suppressed:
		var notify_x := (
			(ACH_NOTIFY_PLAY_W - int(ClassDB.class_call_static("TyrianVga", "text_width", notification, 2))) / 2
		)
		for s in [OVER_A, OVER_B]:
			vga.text_shade(s, notify_x + PLAY_AREA_X, notify_y, notification, 15, 4, TyrianText.FULL_SHADE)
	# enemy_render.c:213: slow-motion indicator, at x 130, under the
	# top text (y 22 at minimum); achievements.c:1587: a banner outside y 170
	# pushes this bottom to notify_y + 9, slow motion goes below.
	if Effects.bullet_time_active():
		var slow_y := slow_motion_y(_prev[TOP_TEXT_BOTTOM], notify_y if banner_shown else -1)
		for s in [OVER_A, OVER_B]:
			vga.text_shade(s, 130 + PLAY_AREA_X, slow_y, TyrianText.L("msg_slow_motion"), 15, 4, TyrianText.FULL_SHADE)
	if _recording_marker_shown(sim):
		for s in [OVER_A, OVER_B]:
			vga.text_shade(s, REC_MARKER_X + PLAY_AREA_X, REPLAY_LABEL_Y, REC_MARKER_TEXT, 4, 4, TyrianText.FULL_SHADE)
	if not replay_label.is_empty():
		for s in [OVER_A, OVER_B]:
			vga.text_shade(
				s,
				replay_label_x + PLAY_AREA_X,
				REPLAY_LABEL_Y,
				replay_label,
				replay_label_color,
				4,
				TyrianText.FULL_SHADE
			)
	if not endless.is_empty():
		_draw_endless_hud(sim)
	# Drawn last, over the frozen image (JE_pauseGame).
	if not pause_text.is_empty():
		_draw_pause_text(sim)
	var img: Image = vga.hud_overlay(0, 0, PLAY_W, PLAY_H, PLAY_AREA_X)
	if _texture == null:
		_texture = ImageTexture.create_from_image(img)
		texture = _texture
	else:
		_texture.update(img)
	if _hd != null:
		_hd.update(SEG, img)


## mainint.c:3243, JE_dString(VGAScreenSeg, 120, 90, miscText[22], FONT_SHAPES):
## the label's shadow (fonthand.c:208, blit_sprite_dark with black = false)
## DARKENS the frozen image underneath (low nibble of the index halved,
## sprite.c:507), it is not black. The HUD layer does not see the play
## area: with the witnesses cleared to 0 and to 1, the shadow gave 0 on both sides,
## hence an opaque black. Here the label is drawn on two witnesses filled with
## 0x0F and 0x0D in its rectangle: the text comes out identical on both sides,
## the shadow gives 0x07 / 0x06, the rest is untouched. Only the shadow pixels
## receive the darkened index of what is underneath (HUD drawing
## already placed, otherwise the C play area image, playfield_indices); the rest
## of the rectangle takes back its content, transparent included (HD mode unchanged).
func _draw_pause_text(sim: Node) -> void:
	var x0 := PAUSE_TEXT_X + PLAY_AREA_X
	var x1 := mini(
		x0 + int(ClassDB.class_call_static("TyrianVga", "text_width", pause_text, FONT_SHAPES)) + 2,
		PLAY_AREA_X + PLAY_W
	)
	var y1 := PAUSE_TEXT_Y + PAUSE_TEXT_H + 2
	var pf := PackedByteArray()
	if sim != null and sim.has_method("playfield_indices"):
		pf = sim.playfield_indices(Game.data_dir)
	var a0: PackedByteArray = vga.indices(OVER_A)
	var b0: PackedByteArray = vga.indices(OVER_B)
	vga.fill_rectangle_xy(OVER_A, x0, PAUSE_TEXT_Y, x1 - 1, y1 - 1, 0x0F)
	vga.fill_rectangle_xy(OVER_B, x0, PAUSE_TEXT_Y, x1 - 1, y1 - 1, 0x0D)
	vga.dstring(OVER_A, x0, PAUSE_TEXT_Y, pause_text, FONT_SHAPES)
	vga.dstring(OVER_B, x0, PAUSE_TEXT_Y, pause_text, FONT_SHAPES)
	var a: PackedByteArray = vga.indices(OVER_A)
	var b: PackedByteArray = vga.indices(OVER_B)
	for y in range(PAUSE_TEXT_Y, y1):
		for x in range(x0, x1):
			var i := y * 320 + x
			if a[i] == b[i]:
				continue  # glyph
			if a[i] == 0x07 and b[i] == 0x06:
				var under: int = a0[i] if a0[i] == b0[i] else (pf[i] if pf.size() == 320 * 200 else -1)
				if under < 0:
					a[i] = a0[i]
					b[i] = b0[i]
				else:
					var d := (under & 0xF0) | ((under & 0x0F) >> 1)
					a[i] = d
					b[i] = d
			else:
				a[i] = a0[i]
				b[i] = b0[i]
	vga.set_indices(OVER_A, a)
	vga.set_indices(OVER_B, b)


## Drawing done during the simulation, in their order (TyrianSim.take_hud_ops).
func _apply_ops(ops: PackedInt32Array, h: PackedInt32Array) -> void:
	for k in range(0, ops.size() - OP_SIZE + 1, OP_SIZE):
		match ops[k]:
			0:
				# Bars with shield and armor of both players at that moment.
				var d := h.duplicate()
				d[SHIELD] = ops[k + 4]
				d[ARMOR] = ops[k + 5]
				d[SHIELD2] = ops[k + 6]
				d[ARMOR2] = ops[k + 7]
				if ops[k + 1] != 0:
					_wipe_shield_armor_bars(d)
				if ops[k + 2] != 0:
					_draw_shield(d)
				if ops[k + 3] != 0:
					_draw_armor(d)
			1:
				# player_movement.c: cell of player 2's charge gauge.
				var level := ops[k + 1]
				vga.fill_rectangle_xy(SEG, 269, 107 + (level - 1) * 3, 275, 108 + (level - 1) * 3, ops[k + 2])
			2:
				_draw_options(h)
			3:
				# level_events.c:414: JE_drawTextWindow(outputs[dat-1]).
				_draw_text_window(TyrianText.L("outputs_%d" % ops[k + 1]))


func _tick(h: PackedInt32Array) -> void:
	var two := h[TWO_PLAYER] != 0
	if h[WEAPON_MODE] != _prev[WEAPON_MODE]:
		_draw_port_config_buttons(h)
	# Sidekick ammo (player_movement.c: fill_rectangle_xy + gauge).
	for i in 2:
		var ammo := h[SK0_AMMO + 3 * i]
		if ammo != _prev[SK0_AMMO + 3 * i]:
			var y: int = SIDEKICK_Y[1 if two else 0][i] + 13
			vga.fill_rectangle_xy(SEG, 284, y, 312, y + 2, 0)
			_segmented_gauge(284, y, 112, 2, 2, maxi(1, h[SK0_AMMO_MAX + 3 * i] / 10), ammo)
	# Message area (tyrian2.c: textErase).
	if _text_erase > 0:
		_text_erase -= 1
		if _text_erase == 0:
			vga.blit_sprite(SEG, 16, 189, OPTION_SHAPES, 36)
	# "Weapon Display".
	for i in 2:
		var item_power := h[WPOWER0 + i]
		if _old_weapon_bar[i] != item_power:
			_old_weapon_bar[i] = item_power
			var x := 286 if two else 289
			var y := (6 if two else 17) if i == 0 else (100 if two else 38)
			vga.fill_rectangle_xy(SEG, x, y, x + 1 + 10 * 2, y + 2, 0)
			for j in range(1, item_power + 1):
				vga.rectangle(SEG, x, y, x + 1, y + 2, 115 + j)
				x += 2
	# "Power Bar".
	if h[TWO_PLAYER] == 0 and h[ONE_PLAYER_ACTION] == 0:
		var temp := h[POWER] / 10
		if temp != _last_power:
			if temp > _last_power:
				vga.fill_rectangle_xy(SEG, 269, 113 - 11 - temp, 276, 114 - 11 - _last_power, 113 + temp / 7)
			else:
				vga.fill_rectangle_xy(SEG, 269, 113 - 11 - _last_power, 276, 114 - 11 - temp, 0)
			_last_power = temp


## mainint.c:JE_inGameHelp: "textErase = 1" (message area cleared at the
## next tick).
func erase_text_window() -> void:
	_text_erase = 1


## Before freezing the screen under the pause menu or help: no more banner,
## no more message in the bottom bar (they would stay drawn under the menu
## without ever expiring, the simulation being stopped). The pinned quick-load
## question stays.
func prepare_frozen_screen(sim: Node) -> void:
	banner_suppressed = true
	# Original mode: the banner is composed into the game image by the Rust
	# core (set_playfield_banner), which the frozen image copies; it is removed before.
	# It comes back at the next _ach_update_banner, after resuming.
	if ClassDB.class_has_method("TyrianSim", "set_playfield_banner"):
		ClassDB.class_call_static("TyrianSim", "set_playfield_banner", "")
	for i in 4:
		_messages[i] = ["", 0, 0]
	_active_message = -1
	if vga != null:
		vga.blit_sprite(SEG, 16, 189, OPTION_SHAPES, 36)
	_text_erase = 0
	refresh(sim)


## Game resume: the current banner comes back on the next tick.
func release_frozen_screen() -> void:
	banner_suppressed = false


## mainint.c:JE_drawTextWindow.
func _draw_text_window(text: String) -> void:
	if _text_erase > 0:
		vga.blit_sprite(SEG, 16, 189, OPTION_SHAPES, 36)
	_text_erase = 100
	vga.out_text(SEG, 20, 190, text, 0, 4)


## varz.c:JE_drawOptions then JE_drawOptionLevel.
func _draw_options(h: PackedInt32Array) -> void:
	var two := h[TWO_PLAYER] != 0
	for i in 2:
		var y: int = SIDEKICK_Y[1 if two else 0][i]
		vga.fill_rectangle_xy(SEG, 284, y, 284 + 28, y + 15, 0)
		var icon := h[SK0_ICON + 3 * i]
		if icon > 0:
			vga.blit_sprite(SEG, 284, y, OPTION_SHAPES, icon - 1)
		_segmented_gauge(284, y + 13, 112, 2, 2, maxi(1, h[SK0_AMMO_MAX + 3 * i] / 10), h[SK0_AMMO + 3 * i])
	# varz.c:JE_drawOptionLevel: player 2's sidekick level (101..103).
	if two:
		for level in range(1, 4):
			var lit := h[SIDEKICK_LEVEL2] - 100 == level
			vga.fill_rectangle_xy(
				SEG, 268, 127 + (level - 1) * 6, 269, 127 + 3 + (level - 1) * 6, 193 + (11 if lit else 0)
			)


## vga256d.c:draw_segmented_gauge.
func _segmented_gauge(x: int, y: int, color: int, sw: int, sh: int, seg_value: int, value: int) -> void:
	var segments := value / seg_value
	var partial := value % seg_value
	for i in segments:
		vga.fill_rectangle_xy(SEG, x, y, x + sw - 1, y + sh - 1, color + 12)
		x += sw + 1
	if partial > 0:
		vga.fill_rectangle_xy(SEG, x, y, x + sw - 1, y + sh - 1, color + (12 * partial / seg_value))


## Double bars: two players outside Galaga (varz.c).
func _split(h: PackedInt32Array) -> bool:
	return h[TWO_PLAYER] != 0 and h[GALAGA] == 0


## varz.c:JE_drawShield; two players: roundf(shield * 0.8f) at y = 60 + 134 i
## (v * 8 is even, never an exact half: (8 v + 5) / 10).
func _draw_shield(h: PackedInt32Array) -> void:
	if _split(h):
		_dbar3(270, 60, (h[SHIELD] * 8 + 5) / 10, 144)
		_dbar3(270, 194, (h[SHIELD2] * 8 + 5) / 10, 144)
		return
	_dbar3(270, 194, h[SHIELD], 144)
	if h[SHIELD] != h[SHIELD_MAX]:
		var y := 193 - h[SHIELD_MAX] * 2
		if y >= 0:
			vga.rectangle(SEG, 270, y, 278, y, 68)


## varz.c:JE_drawArmor (armor capped at 28).
func _draw_armor(h: PackedInt32Array) -> void:
	if _split(h):
		_dbar3(307, 60, (mini(h[ARMOR], 28) * 8 + 5) / 10, 224)
		_dbar3(307, 194, (mini(h[ARMOR2], 28) * 8 + 5) / 10, 224)
		return
	_dbar3(307, 194, mini(h[ARMOR], 28), 224)


## varz.c:JE_wipeShieldArmorBars.
func _wipe_shield_armor_bars(h: PackedInt32Array) -> void:
	if _split(h):
		for x in [270, 307]:
			vga.fill_rectangle_xy(SEG, x, 60 - 44, x + 8, 60, 0)
			vga.fill_rectangle_xy(SEG, x, 194 - 44, x + 8, 194, 0)
		return
	vga.fill_rectangle_xy(SEG, 270, 137, 278, 194 - h[SHIELD] * 2, 0)
	vga.fill_rectangle_xy(SEG, 307, 137, 315, 194 - mini(h[ARMOR], 28) * 2, 0)


## nortvars.c:JE_dBar3.
func _dbar3(x: int, y: int, num: int, col: int) -> void:
	var z_wait := 2
	col += 2
	for z in range(0, num + 1):
		if y < 1:
			break
		vga.rectangle(SEG, x, y - 1, x + 8, y, col)
		if z_wait > 0:
			z_wait -= 1
		else:
			col += 1
			z_wait = 1
		y -= 2


## mainint.c:JE_drawPortConfigButtons.
func _draw_port_config_buttons(h: PackedInt32Array) -> void:
	if h[TWO_PLAYER] != 0:
		return
	var lit := h[WEAPON_MODE] == 1
	vga.blit_sprite(SEG, 285, 44, OPTION_SHAPES, 18 if lit else 19)
	vga.blit_sprite(SEG, 302, 44, OPTION_SHAPES, 19 if lit else 18)


## endless.c:endless_draw_hud, in VGAScreenSeg after the copy of the play
## area (screen coordinates: +24 in the witness surfaces): "WAVE n",
## combo multiplier, wave entry banner (2 s of game time).
func _draw_endless_hud(sim: Node) -> void:
	# endless.c:endless_draw_hud: "if (g_anomaly_eyes_closed) return" (ENH-15).
	if EndlessSession.eyes_closed():
		return
	var wave := EndlessSession.hud_text()
	var wave_x := PLAY_W - 6 - int(ClassDB.class_call_static("TyrianVga", "text_width", wave, 2))
	var wave_y := 30 if EndlessSession.two_player else 3
	var mult: int = sim.combo_multiplier()
	for s in [OVER_A, OVER_B]:
		vga.text_shade(s, wave_x + PLAY_AREA_X, wave_y, wave, 15, 4, TyrianText.FULL_SHADE)
		if mult > 1:
			var m := "x%d" % mult
			vga.text_shade(
				s,
				320 / 2 - int(ClassDB.class_call_static("TyrianVga", "text_width", m, 2)) / 2 + PLAY_AREA_X,
				6,
				m,
				9,
				6,
				TyrianText.FULL_SHADE
			)
	# anomalies_ui.c:anomaly_ui_draw_deck_badge: one initial per card, rank color,
	# right-aligned like "WAVE n", y = 22 (39 in two-player mode) (ENH-15).
	var deck: PackedInt32Array = EndlessSession.run.deck_cards() if EndlessSession.run != null else PackedInt32Array()
	if not deck.is_empty():
		var letters: Array = EndlessSession.deck_initials(deck)
		var total := 0
		for l: Array in letters:
			total += int(ClassDB.class_call_static("TyrianVga", "text_width", l[0], 2)) + 2
		total -= 2
		# Label in front of the initials: without it, "S" read as a red 5.
		var label := TyrianText.L("endless_deck_label")
		var label_w := int(ClassDB.class_call_static("TyrianVga", "text_width", label, 2))
		var label_x := maxi(PLAY_W - 6 - total, 2) - 4 - label_w
		var label_y := 39 if EndlessSession.two_player else 22
		for s in [OVER_A, OVER_B]:
			vga.text_shade(s, label_x + PLAY_AREA_X, label_y, label, 15, 4, TyrianText.FULL_SHADE)
		var bx := maxi(PLAY_W - 6 - total, 2)
		var by := 39 if EndlessSession.two_player else 22
		# Color / brightness by rank (rank_colorbank / rank_brightness): bronze, silver, gold.
		var banks := [1, 0, 7]
		var lights := [4, 5, 6]
		for l: Array in letters:
			for s in [OVER_A, OVER_B]:
				vga.text_shade(s, bx + PLAY_AREA_X, by, l[0], banks[l[1]], lights[l[1]], TyrianText.FULL_SHADE)
			bx += int(ClassDB.class_call_static("TyrianVga", "text_width", l[0], 2)) + 2


## mainint.c:JE_inGameDisplays on the two witness surfaces. In Endless,
## the counter shows endless_score() (only once) and lives are
## endless_lives_of(p) (`endless`: [score, P1 lives, P2 lives], empty otherwise).
func _draw_in_game_displays(h: PackedInt32Array, endless: PackedInt64Array) -> void:
	vga.clear(OVER_A, 0)
	vga.clear(OVER_B, 1)
	var two := h[TWO_PLAYER] != 0
	for s in [OVER_A, OVER_B]:
		for i in 2 if _split(h) else 1:
			if not endless.is_empty() and i > 0:
				continue
			var cash := str(endless[0] if not endless.is_empty() else h[CASH2 if i == 1 else CASH])
			# Deliberate divergence from the original (fixed x 230): player 2's
			# money is right-aligned on the right edge of "Player 2".
			var cash_x := 30 if i == 0 else 285 - int(ClassDB.class_call_static("TyrianVga", "text_width", cash, 2))
			if h[SMOOTHIE6] != 0:
				vga.text_shade(s, cash_x, 175, cash, 8, 8, TyrianText.FULL_SHADE)
			else:
				vga.text_shade(s, cash_x, 175, cash, 2, 4, TyrianText.FULL_SHADE)
		if h[SPECIAL_GR] > 0:
			vga.blit_sprite2(s, 25, 1, "main:9", h[SPECIAL_GR], 0, 0, true)
		# "Lives Left": arcade (one player) or two players.
		if h[ONE_PLAYER_ACTION] != 0 or two:
			for p in 1 if h[ONE_PLAYER_ACTION] != 0 else 2:
				var y := 35 if p == 0 and h[SPECIAL_GR] > 0 else 15
				if true:
					var lives := int(endless[1 + p]) if not endless.is_empty() else h[LIVES2 if p == 1 else LIVES1]
					var extra := maxi(lives - 1, 0)
					var icon_x := 30 if p == 0 else 270
					if extra >= 5:
						vga.blit_sprite2(s, icon_x, y, "main:8", 285, 0, 0, false)
						vga.text_shade(s, 45 if p == 0 else 250, y + 3, str(extra), 15, 1, TyrianText.FULL_SHADE)
					else:
						for j in extra:
							vga.blit_sprite2(s, icon_x, y, "main:8", 285, 0, 0, false)
							icon_x += 12 if p == 0 else -12
				var label := TyrianText.L("miscText_48" if p == 0 else "miscText_49")
				var label_x := (
					28 if p == 0 else 285 - int(ClassDB.class_call_static("TyrianVga", "text_width", label, 2))
				)
				vga.text_shade(s, label_x, y - 7, label, 2, 6, TyrianText.FULL_SHADE)
		# "Super Bombs!!": both players.
		for i in 2:
			var x := 30 if i == 0 else 270
			for j in h[SUPERBOMBS2 if i == 1 else SUPERBOMBS]:
				vga.blit_sprite2(s, x, 160, "main:8", 304, 0, 0, false)
				x += 12 if i == 0 else -12
