## "Side modes" screen (block 5): choice of the mode, mutators and
## anomalies, then level launch with TyrianSim.set_modes (ADR 0050).
##
## The rules of each mode are in tyrian-sim (src/modes.rs and the functions
## it cites); this screen only sets the parameters that the C code
## sets in its menus (title_screen.c:newGame / newSuperArcadeGame,
## menus.c:timedBattleSelect, mutators_menu.c, anomalies_ui.c).
##
## Navigation: shared menu engine (MenuEngine, ADR 0121) — PICKER row
## for the mode, then one TOGGLE row per mutator and per anomaly; the drawing
## (node-based screen with TyrianText, with no 8-bit equivalent in the C code) is provided by
## the `render` option.
extends Node2D

## Name, [two_player, galaga, one_player_action, super_arcade, super_tyrian, timed],
## episode, level (]L[), difficulty, equipment (order of set_loadout).
## Starting values taken from the C golden masters (rewrite/golden/mode-*.gm):
const MODES := [
	["gd_modes_two_players", [1, 0, 0, 0, 0, 0], 1, 9, 2, [1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]],
	["gd_modes_arcade", [0, 0, 1, 0, 0, 0], 1, 9, 2, [8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]],
	["gd_modes_super_arcade", [0, 0, 1, 1, 0, 0], 1, 15, 3, [3, 9, 1, 0, 1, 4, 2, 0, 0, 7, 0]],
	["gd_modes_galaga", [0, 1, 1, 0, 1, 0], 1, 18, 2, [13, 39, 3, 0, 1, 4, 2, 0, 0, 0, 0]],
	["gd_modes_timed_1", [0, 0, 1, 0, 0, 1], 1, 5, 2, [8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]],
	["gd_modes_timed_2", [0, 0, 1, 0, 0, 2], 5, 3, 2, [8, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]],
	# Destruct mini-game (destruct.c): DESTRUCT code at the C title; here
	# too, like the other hidden modes. No level: its own scene.
	["sa_destruct_name", [], 0, 0, 0, []],
]
## Index of Destruct in MODES (mutators and anomalies have no effect).
const MODE_DESTRUCT := 6
## tyrian-sim bits: modes::MUT_* (order of replay.c).
const MUTATORS := ["mut_fast_enemies", "mut_armored", "mut_bullet_hell", "mut_glass_cannon",
	"mut_no_powerups", "mut_one_life", "mut_mirror", "mut_risk_reward"]
## tyrian-sim bits: modes::ANOM_* (order of AnomalyEffect).
const ANOMALIES := ["anom_glass_hull_name", "anom_bullet_storm_name", "anom_twin_cannons_name",
	"anom_hardened_name", "anom_slow_hands_name", "anom_eyes_closed_name", "anom_combo_lock_name",
	"anom_frost_hands_name", "anom_heavy_iron_name"]

var mode_index := 0
var menus := MenuEngine.new(self)
## Backdrop and 8-bit framebuffer of the screen (TyrianScreen.menu_backdrop).
var screen: TyrianScreen


func _ready() -> void:
	# Rewrite's cursor (screen absent from the 8-bit C code).
	add_child(MouseCursor.new())
	screen = TyrianScreen.menu_backdrop(self)
	menus.open("gd_modes_title", _items(), {
		"render": _redraw,
		"adjust_sfx": -1,
		# Mouse (screen specific to the rewrite): row under the pointer.
		"hit": _hit_row,
		"on_back": func() -> void: Game.goto_scene(Game.SCENE_TITLE),
		"shortcuts": {
			TyrianControls.ACCEPT: start,
			# Space: like Right (next mode, or box checked / unchecked).
			TyrianControls.SELECT: func() -> bool:
				menus.adjust_horizontal(menus.current().items[menus.selection()], 1, -1)
				return true,
		},
	})


## Row 0: mode; then one toggle per bit of Game.mutators and Game.anomalies.
func _items() -> Array:
	var items := [MenuEngine.item("gd_modes_mode", "", MenuEngine.Kind.PICKER, {
		"get": func() -> int: return mode_index,
		"set": func(i: int) -> void: mode_index = i,
		"count": MODES.size(),
	})]
	for i in MUTATORS.size():
		items.append(MenuEngine.item(MUTATORS[i], "", MenuEngine.Kind.TOGGLE, {
			"get": func() -> bool: return Game.mutators & (1 << i) != 0,
			# B-18: the toggle is the C's saved global -> written to setup.cfg.
			"set": func(_v: bool) -> void: preload("res://scripts/setup.gd")._set_mutators(Game.mutators ^ (1 << i)),
		}))
	for i in ANOMALIES.size():
		items.append(MenuEngine.item(ANOMALIES[i], "", MenuEngine.Kind.TOGGLE, {
			"get": func() -> bool: return Game.anomalies & (1 << i) != 0,
			"set": func(_v: bool) -> void: Game.anomalies ^= 1 << i,
		}))
	return items


## Selected row (0 = mode).
func row() -> int:
	return menus.selection()


## Drawing in the 8-bit framebuffer on image 2 and its palette, with the C
## colors: title like menu_tree.c:run_menu (draw_font_hv_shadow 15, -4, shadow 2);
## rows in small font like mod_manager.c:1549 (JE_textShade 15, 4 on the
## chosen row, 2 otherwise, FULL_SHADE); help JE_textShade(15, 4, PART_SHADE).
## Screen specific to the rewrite (two columns).
func _redraw() -> void:
	if screen == null:
		return
	screen.begin_menu_frame()
	var r := row()
	TyrianText.draw_font_hv_shadow(screen, 160, 4, TyrianText.L("gd_modes_title"), TyrianScreen.NORMAL_FONT,
			TyrianScreen.CENTERED, 15, -4, false, 2)
	_row_text(TyrianText.L("gd_modes_mode") % TyrianText.L(MODES[mode_index][0]), 20, 24, r == 0)
	for i in MUTATORS.size():
		var on: bool = Game.mutators & (1 << i) != 0
		_row_text("[%s] %s" % ["x" if on else " ", TyrianText.L(MUTATORS[i])], 20, 38 + i * 9, r == 1 + i)
	for i in ANOMALIES.size():
		var on: bool = Game.anomalies & (1 << i) != 0
		_row_text(
			"[%s] %s" % ["x" if on else " ", TyrianText.L(ANOMALIES[i])], 170, 38 + i * 9, r == 1 + MUTATORS.size() + i
		)
	TyrianText.text_shade(screen, 20, 176, TyrianText.L("gd_modes_keys"), 15, 4, TyrianText.PART_SHADE)
	TyrianText.text_shade(screen, 20, 186, TyrianText.L("gd_modes_keys2"), 15, 4, TyrianText.PART_SHADE)
	screen.present()


func _row_text(s: String, x: int, y: int, selected: bool) -> void:
	TyrianText.text_shade(screen, x, y, s, 15, 4 if selected else 2, TyrianText.FULL_SHADE)


## Sets the chosen mode in Game then launches the level.
func start() -> void:
	if mode_index == MODE_DESTRUCT:
		Sfx.play(Sfx.SELECT)
		Game.goto_scene(Game.SCENE_DESTRUCT)
		return
	var m: Array = MODES[mode_index]
	var flags: Array = m[1]
	Game.modes = PackedInt32Array(flags + [Game.mutators, Game.anomalies, 0, 0])
	Game.difficulty = m[4]
	Game.loadout = PackedInt32Array(m[5])
	Game.start_level_at(m[2], m[3], 0)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouse:
		menus.handle_mouse(event)
		return
	var action := TyrianControls.menu_action(event)
	if action != &"":
		handle_action(action)


## Row under point `p`: the mode row (y 24..37), then one toggle per
## mutator (left column) and per anomaly (right column).
func _hit_row(p: Vector2i) -> int:
	if p.y >= 24 and p.y < 37 and p.x >= 20 and p.x < 300:
		return 0
	if p.y < 38:
		return -1
	var i := (p.y - 38) / 9
	if i < 0:
		return -1
	if p.x >= 20 and p.x < 165 and i < MUTATORS.size():
		return 1 + i
	if p.x >= 170 and p.x < 305 and i < ANOMALIES.size():
		return 1 + MUTATORS.size() + i
	return -1


## Menu action (shared InputMap, ADR 0120), handled by the shared engine.
func handle_action(action: StringName) -> void:
	menus.handle_action(action)
