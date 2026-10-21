## New game from the title (title_screen.c:newGame, newSuperArcadeGame,
## newSuperTyrianGame) and its choice screens (menus.c:gameplaySelect,
## episodeSelect, difficultySelect, timedBattleSelect), frame by frame on
## TyrianScreen (ADR 0020) with the shared menu engine (ADR 0121).
##
## Each menus.c screen: tyrian.pic no. 2 background and title in large font
## in VGAScreen2 (`restart` branch), then at each turn background copy,
## shadowed rows (draw_font_hv_shadow, value -4, +2 if chosen, -4 if
## unavailable), fade-in (10 steps) and JE_showVGA. Esc (S_SPRING,
## fade_black(15)) goes back one screen like the C code:
## difficulty -> episode (or timed battle) -> title.
##
## End of the choice: the corresponding rewrite game (campaign from
## Game.start_campaign, with the TyrianSim.set_modes modes for arcade,
## two players, timed battle and the ships of the title
## codes; Endless through EndlessSession).
extends TyrianScreen

## menus.c:gameplaySelect, MenuItemIndex.
enum { MODE_FULL, MODE_ARCADE, MODE_ENDLESS, MODE_TIMED, MODE_2P_ARCADE, MODE_2P_ENDLESS, MODE_NETWORK }
## helptext.c: gameplay_name[1..7] <- locale keys (order of MenuItemIndex).
const GAMEPLAY_KEYS := ["gm_1p_full", "gm_1p_arcade", "gm_endless", "gm_1p_timed", "gm_2p_arcade", "gm_2p_endless",
		"gm_network"]
## helptext.c: difficulty_name[1..6]; menus.c:difficultySelect, difficultyLevel of each row.
const DIFFICULTY_KEYS := ["diff_easy", "diff_normal", "diff_hard", "diff_impossible", "diff_suicide", "diff_lord"]
const DIFFICULTY_LEVELS := [1, 2, 3, 4, 6, 8]
## config.h: DIFFICULTY_SUICIDE, DIFFICULTY_ZINGLON ("Lord of the Game").
const DIFFICULTY_SUICIDE := 6
const DIFFICULTY_ZINGLON := 8
## menus.c:difficultySelect: keys of "LORD".
const LORD_KEYS := [KEY_L, KEY_O, KEY_R, KEY_D]
## title_screen.c:newGame: initial_cash[] (episodes 1 to 5).
const INITIAL_CASH := [10000, 15000, 20000, 30000, 20000]
## varz.c: SAShip, SAWeapon[i][0], SASpecialWeapon (Super Arcade, 9 ships).
const SA_SHIP := [3, 1, 5, 10, 2, 11, 12, 15, 17]
const SA_FRONT_WEAPON := [9, 19, 27, 15, 23, 2, 40, 47, 44]
const SA_SPECIAL := [7, 8, 9, 10, 11, 12, 13, 48, 47]
## varz.h: SA_NORTSHIPZ (superArcadeMode, 1-based).
const SA_NORTSHIPZ := 7
## sndmast.h : V_GOOD_LUCK.
const V_GOOD_LUCK := 35
## mainint.c:JE_initPlayerData: starting equipment (order of set_loadout);
## player[1]: same equipment, Vulcan Cannon (15) at the rear.
const START_LOADOUT := [1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]
const PLAYER2_LOADOUT := [1, 1, 1, 15, 1, 4, 2, 0, 0, 0]

## Entry path: title "New Game", Super Arcade code, ENGAGE code.
enum Flow { NEW_GAME, SUPER_ARCADE, SUPER_TYRIAN }
static var flow := Flow.NEW_GAME
## Super Arcade code typed at the title (index of specialName, 0-based).
static var super_arcade := 0

## Idle screen (capture tool, tests).
var idle := false
## Displayed screen: "gameplay", "episode", "difficulty", "timed", "super_arcade", "super_tyrian".
var current_screen := ""
var menus := MenuEngine.new(self)
var _busy := true
## Choice in progress: mode, episode (1-based), battle (timeBattleSelection).
var mode := MODE_FULL
var episode := 1
var time_battle := 0
## menus.c:difficultySelect: menuItemsVisibleCount, lordProgress.
var difficulty_count := 3
var _lord_progress := 0
## title_screen.c:newSuperTyrianGame: initialDifficulty.
var _super_tyrian_difficulty := DIFFICULTY_ZINGLON


func _ready() -> void:
	_run.call_deferred()


func _run() -> void:
	match flow:
		Flow.SUPER_ARCADE:
			mode = MODE_ARCADE
			await _open_episode()
		Flow.SUPER_TYRIAN:
			await _super_tyrian_intro()
		_:
			await _open_gameplay()


## Background and title of a screen (`restart` branch): JE_loadPic(VGAScreen2, 2)
## then draw_font_hv_shadow(VGAScreen2, 160, 20, title, large_font).
func _restart(header: String) -> void:
	vga.load_pic(VGA_SCREEN2, 2, false)
	TyrianText.draw_font_hv_shadow(self, 160, 20, header, LARGE_FONT, CENTERED, 15, -3, false, 2, VGA_SCREEN2)


## A menus.c row: centered (x < 0) or at x, 2 px shadow.
func _row(label: String, x: int, disabled: Callable) -> Dictionary:
	var width := TyrianText.width(label, NORMAL_FONT)
	return MenuEngine.item(label, "", MenuEngine.Kind.CUSTOM, {
		# Active area = text width (menus.c: xMenuItem .. + wMenuItem).
		"hit_x": x if x >= 0 else 160 - width / 2, "hit_w": width,
		"draw": func(_it: Dictionary, _x: int, y: int, selected: bool) -> void:
			var px := x if x >= 0 else 160 - TyrianText.width(label, NORMAL_FONT) / 2
			var value := -4 + (2 if selected else 0) + (-4 if disabled.call() else 0)
			TyrianText.draw_font_hv_shadow(self, px, y, label, NORMAL_FONT, LEFT, 15, value, false, 2),
		# Left / Right: the C's "default: break", the image is shown again.
		"adjust": func(_it: Dictionary, _dir: int) -> void: pass,
		"activate": func(_it: Dictionary) -> void: pass,
	})


## Opens a screen: background, rows, fade-in (fade_palette 10) then JE_showVGA.
func _open(id: String, header_key: String, items: Array, y: int, dy: int, on_activate: Callable, on_back: Callable,
		sel := 0) -> void:
	current_screen = id
	_busy = true
	idle = false
	_restart(TyrianText.L(header_key))
	menus.stack.clear()
	await menus.open("", items, {
		"style": MenuEngine.Style.ROWS, "row_x": 0, "row_y": y, "row_step": dy, "sel": sel,
		"move_sfx": Sfx.CURSOR, "accept": [TyrianControls.SELECT, TyrianControls.ACCEPT],
		"redraw_on_other": true, "on_activate": on_activate,
		"hit": _row_under_pointer.bind(y, dy),
		"on_back": func() -> void:
			Sfx.play(Sfx.SPRING)
			_leave(on_back),
		"pre_present": func() -> void: await fade_palette(10, 0, 255),
	})
	_busy = false


## menus.c: a row is only targeted on its text (x .. x + width,
## height hMenuItem = 13), not on the whole screen width.
func _row_under_pointer(p: Vector2i, y0: int, dy: int) -> int:
	if menus.stack.is_empty():
		return -1
	var items: Array = menus.current().items
	for i in items.size():
		var it: Dictionary = items[i]
		var row_y := y0 + dy * i
		if p.x >= int(it.hit_x) and p.x < int(it.hit_x) + int(it.hit_w) and p.y >= row_y and p.y < row_y + 13:
			return i
	return -1


## Esc or right click: "fade_black(15); return false;".
func _leave(then: Callable) -> void:
	_busy = true
	idle = false
	await fade_black(15)
	await then.call()


func _to_title() -> void:
	Game.goto_scene(Game.SCENE_TITLE)


# --- menus.c:gameplaySelect -----------------------------------------------------

func _open_gameplay() -> void:
	var items := []
	for i in GAMEPLAY_KEYS.size():
		items.append(_row(TyrianText.L(GAMEPLAY_KEYS[i]), -1, func() -> bool: return i == MODE_NETWORK))
	await _open("gameplay", "gm_title", items, 50, 20, _gameplay_chosen, _to_title)


func _gameplay_chosen(sel: int) -> void:
	match sel:
		MODE_FULL, MODE_ARCADE, MODE_TIMED, MODE_2P_ARCADE:
			Sfx.play(Sfx.SELECT)
			_busy = true
			idle = false
			await fade_black(10)
			mode = sel
			if mode == MODE_TIMED:
				await _open_timed()
			else:
				await _open_episode()
		MODE_ENDLESS, MODE_2P_ENDLESS:
			# endless_resume_prompt() && endless_resume(), otherwise endless_start(two_p);
			# a refusal goes back to the title (gameplaySelect returns false).
			Sfx.play(Sfx.SELECT)
			_busy = true
			idle = false
			await fade_black(10)
			if not EndlessSession.enter(sel == MODE_2P_ENDLESS):
				_to_title()
		MODE_NETWORK:
			# Grayed-out entry: the C code refuses it (S_SPRING) with or without WITH_NETWORK.
			Sfx.play(Sfx.SPRING)
			menus.draw()


# --- menus.c:episodeSelect -------------------------------------------------------

## episodes.c:JE_scanForEpisodes: tyrianN.lvl present in data/.
static func episode_available(ep: int) -> bool:
	return FileAccess.file_exists(Game.data_dir.path_join("tyrian%d.lvl" % ep))


func _open_episode() -> void:
	await _open("episode", "ep_title", _episode_rows(), 50, 30, _episode_chosen, _to_title)


func _episode_chosen(sel: int) -> void:
	if not episode_available(sel + 1):
		Sfx.play(Sfx.SPRING)
		menus.draw()
		return
	Sfx.play(Sfx.SELECT)
	_busy = true
	idle = false
	await fade_black(10)
	episode = sel + 1  # JE_initEpisode(selectedIndex + 1)
	if flow == Flow.SUPER_TYRIAN:
		_start_super_tyrian()
	else:
		await _open_difficulty()


# --- menus.c:timedBattleSelect ---------------------------------------------------

func _open_timed() -> void:
	var items := []
	for i in 3:
		items.append(_row(TyrianText.L("timedBattle_%d" % (i + 1)), -1, func() -> bool: return false))
	await _open("timed", "timedBattle_0", items, 54, 24, _timed_chosen, _to_title)


func _timed_chosen(sel: int) -> void:
	Sfx.play(Sfx.SELECT)
	_busy = true
	idle = false
	await fade_black(10)
	# "JE_initEpisode(selectedIndex == 0 ? 1 : 5); timeBattleSelection = selectedIndex + 1;"
	episode = 1 if sel == 0 else 5
	time_battle = sel + 1
	await _open_difficulty()


# --- menus.c:difficultySelect ----------------------------------------------------

func _difficulty_items() -> Array:
	var items := []
	for i in difficulty_count:
		items.append(_row(TyrianText.L(DIFFICULTY_KEYS[i]), -1, func() -> bool: return false))
	return items


func _open_difficulty() -> void:
	difficulty_count = 3
	_lord_progress = 0
	# Esc: episodeThenDifficultySelect / timedBattleThenDifficultySelect
	# resume the previous choice.
	var back := _open_timed if mode == MODE_TIMED else _open_episode
	await _open("difficulty", "diff_title", _difficulty_items(), 54, 24, _difficulty_chosen, back, 1)


func _difficulty_chosen(sel: int) -> void:
	Sfx.play(Sfx.SELECT)
	_busy = true
	idle = false
	var level: int = DIFFICULTY_LEVELS[sel]
	await fade_black(10)
	if flow == Flow.SUPER_ARCADE:
		await _start_super_arcade(level)
	else:
		_start_game(level)


## menus.c:542-582: "LORD" (four keys held or typed in order)
## when Suicide is visible; Shift+] (Suicide) when Impossible is;
## Shift+G (Impossible) from the three starting rows. Tested at each
## event, before navigation.
func _difficulty_unlocks(event: InputEventKey) -> void:
	var raw: int = event.physical_keycode if event.physical_keycode != KEY_NONE else event.keycode
	var base: int = raw & KEY_CODE_MASK
	var before := difficulty_count
	if difficulty_count == 5:
		var held := true
		for k in LORD_KEYS:
			held = held and (Input.is_physical_key_pressed(k) or base == k)
		if held:
			difficulty_count = 6
		elif _lord_progress < LORD_KEYS.size() and base == LORD_KEYS[_lord_progress]:
			_lord_progress += 1
			if _lord_progress == LORD_KEYS.size():
				difficulty_count = 6
		else:
			_lord_progress = 0
	var shift := event.shift_pressed or (raw & KEY_MASK_SHIFT) != 0 or Input.is_physical_key_pressed(KEY_SHIFT)
	if shift:
		if difficulty_count == 4 and (base == KEY_BRACKETRIGHT or Input.is_physical_key_pressed(KEY_BRACKETRIGHT)):
			difficulty_count = 5
		if difficulty_count == 3 and (base == KEY_G or Input.is_physical_key_pressed(KEY_G)):
			difficulty_count = 4
	if difficulty_count != before:
		menus.current().items = _difficulty_items()


# --- Game start (title_screen.c:newGame) ----------------------------------------

## End of newGame: initialDifficulty = difficultyLevel then, depending on the mode,
## ship and money; the campaign (JE_main / JE_loadMap) begins.
func _start_game(level: int) -> void:
	var loadout := PackedInt32Array(START_LOADOUT)
	match mode:
		MODE_ARCADE, MODE_TIMED:
			# onePlayerAction: money at 0, Stalker (8).
			loadout[0] = 8
			Game.start_campaign(episode, level, {
				"loadout": loadout,
				"modes": PackedInt32Array([0, 0, 1, 0, 0, time_battle, 0, 0, 0, 0]),
				"time_battle": time_battle,
			})
		MODE_2P_ARCADE:
			# twoPlayerMode: money at 0, Silver Ship (11); difficultyLevel++.
			loadout[0] = 11
			Game.start_campaign(episode, level, {
				"loadout": loadout,
				"difficulty": level + 1,
				"modes": PackedInt32Array([1, 0, 0, 0, 0, 0, 0, 0, 1, 2]),
				"modes_extra": [0, 0, level, PackedInt32Array(PLAYER2_LOADOUT)],
			})
		_:
			# Full game: starting money of the episode.
			Game.start_campaign(episode, level)


## title_screen.c:newSuperArcadeGame, after the episode and difficulty:
## ship screen ("Super Arcade Mode", name, enlarged ship), key,
## then one-player game with the code's ship, weapon and special weapon.
func _start_super_arcade(level: int) -> void:
	var i := super_arcade
	current_screen = "super_arcade"
	vga.load_pic(VGA_SCREEN, 1, false)
	vga.clear(VGA_SCREEN, 0)
	var title := TyrianText.L("superShip_0")
	vga.dstring(VGA_SCREEN, TyrianText.font_center(title, LARGE_FONT), 30, title, LARGE_FONT)
	var ship_name := TyrianText.L("superShip_%d" % (i + 1))
	vga.dstring(VGA_SCREEN, TyrianText.font_center(ship_name, NORMAL_FONT), 100, ship_name, NORMAL_FONT)
	var g := _ship_graphic(SA_SHIP[i])
	if g > 500:
		vga.blit_sprite2(VGA_SCREEN, 148, 70, "main:12", g - 500, 0, 0, true)
	elif g != 1:
		vga.blit_sprite2(VGA_SCREEN, 148, 70, "main:8", g, 0, 0, true)
	present()
	await fade_palette(50, 0, 255)
	await wait_input()
	var loadout := PackedInt32Array(START_LOADOUT)
	loadout[0] = SA_SHIP[i]
	loadout[1] = SA_FRONT_WEAPON[i]
	loadout[9] = SA_SPECIAL[i]
	if i + 1 == SA_NORTSHIPZ:
		loadout[7] = 24  # Companion Ship Quicksilver
		loadout[8] = 24
	await fade_black(10)
	# "initialDifficulty = ++difficultyLevel".
	Game.start_campaign(episode, level + 1, {
		"loadout": loadout,
		"modes": PackedInt32Array([0, 0, 1, i + 1, 0, 0, 0, 0, 0, 0]),
	})


## shipgraphic of a ship (items.json, mods included).
func _ship_graphic(ship: int) -> int:
	var g: int = Mods.ship_graphic(ship) if Mods.has_method("ship_graphic") else -1
	if g >= 0:
		return g
	var ships: Array = TyrianAssets.read_json("items.json")["episodes_1_3"]["ships"]
	return int(ships[ship].get("shipgraphic", 0)) if ship < ships.size() else 0


## title_screen.c:newSuperTyrianGame (ENGAGE code), after JE_whoa and
## set_colors(black) done by the title: texts, "SUPER Tyrian", music
## 16, "Good luck", two key waits, fade_black(1), then the choice
## of the episode (Esc: title music and back to the title).
func _super_tyrian_intro() -> void:
	current_screen = "super_tyrian"
	_super_tyrian_difficulty = DIFFICULTY_SUICIDE if Input.is_physical_key_pressed(KEY_SCROLLLOCK) else DIFFICULTY_ZINGLON
	var zinglon := _super_tyrian_difficulty == DIFFICULTY_ZINGLON
	vga.clear(VGA_SCREEN, 0)
	TyrianText.out_text(self, 10, 10, TyrianText.L("stText_0"), 15, 4)
	TyrianText.out_text(self, 10, 20, TyrianText.L("stText_1" if zinglon else "stText_2"), 15, 4)
	TyrianText.out_text(self, 10, 30, TyrianText.L("stText_3"), 15, 4)
	if zinglon:
		TyrianText.out_text(self, 10, 40, TyrianText.L("stText_4"), 15, 4)
	TyrianText.out_text(self, 10, 60, TyrianText.L("stText_5"), 15, 4)
	var buf := "%s %s" % [TyrianText.L("miscTextB_4"), TyrianText.L("planet_0")]
	vga.dstring(VGA_SCREEN, TyrianText.font_center(buf, LARGE_FONT), 110, buf, LARGE_FONT)
	Music.play_song(Music.SONG_SUPER_TYRIAN)
	Sfx.play(V_GOOD_LUCK)
	present()
	await fade_palette(10, 0, 255)
	await wait_input()
	await fade_black(1)
	mode = MODE_ARCADE
	await _open("episode", "ep_title", _episode_rows(), 50, 30, _episode_chosen, _super_tyrian_cancel)


## newSuperTyrianGame, episode refused: "play_song(SONG_TITLE); return false;".
func _super_tyrian_cancel() -> void:
	Music.play_song(Music.SONG_TITLE)
	_to_title()


func _episode_rows() -> Array:
	var items := []
	for i in 5:
		items.append(_row(TyrianText.L("ep_%d" % (i + 1)), 20, func() -> bool: return not episode_available(i + 1)))
	return items


## newSuperTyrianGame, episode chosen: Stalker 21.126 (13), Atomic RailGun
## (39), money at 0, Zinglon difficulty (Suicide if Scroll Lock).
func _start_super_tyrian() -> void:
	var loadout := PackedInt32Array(START_LOADOUT)
	loadout[0] = 13
	loadout[1] = 39
	Game.start_campaign(episode, _super_tyrian_difficulty, {
		"loadout": loadout,
		"modes": PackedInt32Array([0, 0, 1, 0, 1, 0, 0, 0, 0, 0]),
	})


## wait_input(true, true, true): waits for a key, a click or a button.
## Super Arcade and Super Tyrian ship screens (title_screen.c:1012-1015,
## 1065-1069): presented without JE_mouseStart, no pointer.
func wait_input() -> void:
	push_pointer_hidden()
	_input_seen = false
	idle = true
	while not _input_seen and is_inside_tree():
		await get_tree().process_frame
	idle = false
	pop_pointer_hidden()


# --- Input -------------------------------------------------------------------

func handle_event(event: InputEvent) -> void:
	if (
		current_screen == "difficulty"
		and not _busy
		and idle
		and event is InputEventKey
		and event.pressed
		and not event.echo
	):
		_difficulty_unlocks(event)
	super(event)


func handle_action(action: StringName) -> void:
	if modal_input(action):
		return
	if _busy or not idle or menus.stack.is_empty():
		return
	menus.handle_action(action)
