## Intro logos and title screen, frame by frame like the C version
## (title_screen.c:intro_logos and titleScreen, ADR 0020).
##
## Everything is drawn in the 8-bit framebuffer of `TyrianScreen` with the
## same calls as the C code; each `present()` matches a `JE_showVGA()`
## of the C code, in the same order (palette fades included): the image sequence
## is compared one by one with that of the instrumented C build (GOLDEN_FRAMES).
extends TyrianScreen

## title_screen.c:titleScreen.
const Y_MENU_ITEMS := 88
## helptext.c: menuText[0..8] <- locale keys (same order as MenuItemIndex).
const MENU_KEYS := [
	"menu_new_game", "menu_load_game", "menu_high_scores", "menu_instructions", "menu_setup",
	"menu_achievements", "menu_replays", "menu_demo", "menu_quit",
]
## highscore.c:JE_highScoreScreen (own scene, outside game.gd).
const SCENE_HIGH_SCORES := "res://scenes/high_scores.tscn"
const SCENE_INSTRUCTIONS := "res://scenes/instructions.tscn"
const SCENE_LOAD := "res://scenes/load_screen.tscn"
const SCENE_ACHIEVEMENTS := "res://scenes/achievements_screen.tscn"
const SCENE_REPLAYS := "res://scenes/replays_screen.tscn"
enum { NEW_GAME, LOAD_GAME, HIGH_SCORES, INSTRUCTIONS, SETUP, ACHIEVEMENTS, REPLAYS, DEMO, QUIT }

## title_screen.c:moveTyrianLogoUp: true on the first pass (intro_logos)
## and after a demo (tyrian2.c:JE_main), the logo then moves up.
static var move_tyrian_logo_up := true
## opentyr.c:main: intro_logos only once, at launch.
static var intro_shown := false
## opentyrian_version: published version (application/config/version of the
## project); nothing is displayed if it is empty, never "dev".
static var version_text: String = ProjectSettings.get_setting("application/config/version", "")
## Margin of the bottom texts, identical on left and right (title_screen.c).
const FOOTER_MARGIN := 4
## title_screen.c:titleScreen: "Play demo after idle for 30 seconds"
## (SDL_GetTicks() - idleStartTick > 30000). Modifiable by tests.
static var demo_idle_ms := 30000
## helptext.c: specialName[0..10] (title codes, varz.h:SA_* = index + 1).
const SPECIAL_NAME_COUNT := 11
## varz.h: SA_DESTRUCT, SA_ENGAGE.
const SA_DESTRUCT := 10
const SA_ENGAGE := 11
## sndmast.h: V_DANGER (JE_whoa of the ENGAGE code).
const V_DANGER := 37
## Destruct mini-game (ported by another batch), opened by the DESTRUCT code if it exists.
const SCENE_DESTRUCT := "res://scenes/destruct.tscn"

## selectedIndex of titleScreen: survives the passage through a subscreen
## (the C local variable remains valid during `restart`).
static var selected := 0
## True when the screen is waiting for a key ("do ... while (!newkey)" loop).
var idle := false
## Menu entries (shared engine, TITLE style, ADR 0121).
var _items: Array = []
var menus := MenuEngine.new(self)
var _busy := false
## title_screen.c: specialNameProgress[SA_ENGAGE] (local to the title).
var _code_progress := PackedInt32Array()
## Time (ms) since the last JE_showVGA of the loop (idleStartTick).
var _idle_ms := 0.0


## Screens called from the title loop: returning from them does not cut the sounds.
const TITLE_SUB_SCREENS := [
	"res://scenes/load_screen.tscn", "res://scenes/high_scores.tscn",
	"res://scenes/instructions.tscn", "res://scenes/achievements_screen.tscn",
	"res://scenes/replays_screen.tscn", "res://scenes/jukebox.tscn",
	"res://scenes/mods.tscn", "res://scenes/modes_menu.tscn",
	"res://scenes/controls_options.tscn", "res://scenes/display_options.tscn",
]

## Other screens from which the title returns through `restart = true` (Setup,
## New Game cancelled, Super Arcade / Super Tyrian ship screen cancelled).
const TITLE_RESTART_SCREENS := ["res://scenes/setup.tscn", "res://scenes/new_game.tscn"]


func _ready() -> void:
	# title_screen.c:462 stop_all_samples(): a long sample started at the end of
	# the level must not continue on the menu.
	# The C code only cuts on entry to titleScreen(), not on return from a title
	# subscreen (the Esc sound finishes).
	if not Effects.previous_scene_path in TITLE_SUB_SCREENS:
		Sfx.stop_all()
	# title_screen.c:482: selectedIndex is local to titleScreen. A return
	# from a screen opened from the title (`restart = true`) keeps the row;
	# a new entry (after a game, a demo, a replay, Destruct:
	# opentyr.c:406) restarts on "Start New Game".
	if not Effects.previous_scene_path in TITLE_SUB_SCREENS + TITLE_RESTART_SCREENS:
		selected = NEW_GAME
	_code_progress.resize(SPECIAL_NAME_COUNT)
	# title_screen.c: JE_mouseStartFilter(0xF0) (tinted title pointer).
	mouse_filter_hue = 0xF0
	for k in MENU_KEYS:
		_items.append(MenuEngine.item(TyrianText.L(k), "", MenuEngine.Kind.ACTION))
	_run.call_deferred()


## varz.c:517-545 (JE_tyrianHalt): end screen after the Super Tyrian victory; DOS text
## from 1995 kept as is, a key quits the game.
const TRENT_TEXT := [
	"Sleep well, Trent, you deserve the rest.",
	"You now have permission to borrow my ship",
	"on your next mission.",
	"",
	"Also, you might want to try out the",
	"YESXMAS parameter in Dos.",
	"  Type: File0001 YESXMAS",
]


func _trent_ending() -> void:
	# varz.c:JE_tyrianHalt: text and wait without JE_mouseStart.
	push_pointer_hidden()
	Game.trent_win = false
	vga.clear(VGA_SCREEN, 0)
	for i in TRENT_TEXT.size():
		TyrianText.out_text(self, 10, 40 + 10 * i, TRENT_TEXT[i], 15, 4)
	# gdlint: ignore=max-line-length
	TyrianText.out_text(self, 10, 130, "Press a Key to Quit", 15, 2)  # locale-exempt: varz.c hardcodes the Trent screen, in English
	vga.load_palette(0)
	present()
	await fade_palette(10)
	# wait_noinput then wait_input: a key already held does not close the screen.
	_input_seen = false
	idle = true
	while is_inside_tree() and not _input_seen:
		await get_tree().process_frame
	idle = false
	await fade_black(15)
	get_tree().quit()


func _run() -> void:
	_busy = true
	# opentyr.c: in December, the Christmas prompt comes before everything (logos included).
	if Christmas.needs_prompt():
		Effects.show_scene("res://scenes/xmas_prompt.tscn")
		return
	if Game.trent_win:
		await _trent_ending()
		return
	if move_tyrian_logo_up and not intro_shown and not Capture.has_option("no-intro"):
		intro_shown = true
		await intro_logos()
	await _restart()
	_busy = false
	_redraw_selection()


## title_screen.c:intro_logos (1098-1125): logos presented without JE_mouseStart,
## no pointer.
func intro_logos() -> void:
	push_pointer_hidden()
	vga.clear(VGA_SCREEN, 0)
	await fade_white(25)
	vga.load_pic(VGA_SCREEN, 10, false)
	present()
	await fade_palette(25, 0, 255)
	await wait_units_or_input(200)
	await fade_black(10)
	vga.load_pic(VGA_SCREEN, 12, false)
	present()
	await fade_palette(10, 0, 255)
	await wait_units_or_input(200)
	await fade_black(10)
	pop_pointer_hidden()


## `if (restart)` branch of titleScreen. Logo rise and fades presented
## without JE_mouseStart (title_screen.c:511-558): the pointer only appears at
## the menu (572-574).
func _restart() -> void:
	push_pointer_hidden()
	# title_screen.c:titleScreen: play_song(SONG_TITLE) at the head of the restart branch.
	Music.play_song(Music.SONG_TITLE)
	vga.load_pic(VGA_SCREEN, 4, false)
	if not version_text.is_empty():
		TyrianText.draw_font_hv_shadow(self, FOOTER_MARGIN, 192, version_text, SMALL_FONT, LEFT, 15, 0, false, 1)
	if move_tyrian_logo_up:
		vga.copy(VGA_SCREEN2, VGA_SCREEN)
		vga.blit_sprite(VGA_SCREEN, 11, 62, PLANET_SHAPES, 146)
		vga.blit_sprite(VGA_SCREEN, 155, 41, PLANET_SHAPES, 151)
		await fade_palette(10, 0, 255 - 16)
		var y2k := 45
		for y_logo in range(60, 3, -2):
			vga.copy(VGA_SCREEN, VGA_SCREEN2)
			vga.blit_sprite(VGA_SCREEN, 11, y_logo, PLANET_SHAPES, 146)
			vga.blit_sprite(VGA_SCREEN, 155, y2k, PLANET_SHAPES, 151)
			present()
			await wait_units(2)
			y2k += 1
		move_tyrian_logo_up = false
	else:
		vga.blit_sprite(VGA_SCREEN, 11, 4, PLANET_SHAPES, 146)
		vga.blit_sprite(VGA_SCREEN, 155, 73, PLANET_SHAPES, 151)
		await fade_palette(10, 0, 255 - 16)
	menus.render_title_items(_items)
	vga.copy(VGA_SCREEN2, VGA_SCREEN)
	await fade_palette(20, 255 - 16 + 1, 255)
	pop_pointer_hidden()


## Top of the main loop: highlight of the chosen entry, JE_showVGA.
func _redraw_selection() -> void:
	if menus.stack.is_empty():
		menus.open("", _items, {
			"style": MenuEngine.Style.TITLE, "draw": false, "move_sfx": Sfx.CURSOR,
			"accept": [TyrianControls.SELECT, TyrianControls.ACCEPT], "redraw_on_horizontal": false,
			"on_activate": func(sel: int) -> void:
				selected = sel
				idle = false
				activate(),
			# Esc asks for confirmation before quitting (title_screen.c).
			"on_back": func() -> void:
				Sfx.play(Sfx.SPRING)
				_quit_or_restart(),
			# Shortcuts called without await by MenuEngine (".call() == true"):
			# a coroutine passed as is raises "async function without
			# await"; wrapped like on_back, they run in the background.
			"shortcuts": {
				# M (rewrite's side modes) only acts if it does not continue
				# a code (ENEMY); E, first letter of ENEMY and ENGAGE,
				# is no longer a shortcut: Endless is in New Game, like
				# in the C code (menus.c:gameplaySelect).
				TyrianControls.MODES: func() -> void:
					if not _continues_code("M"):
						_open_modes(),
				TyrianControls.ENDLESS_2P: func() -> void: _start_endless(true),
			},
		})
	menus.current().sel = selected
	menus.draw()
	_idle_ms = 0.0


## title_screen.c:titleScreen, wait loop: after 30 s with no key,
## click or mouse movement, fade_black(15) and demo.
func _process(delta: float) -> void:
	if not idle or _busy or modal_active or Capture.active:
		_idle_ms = 0.0
		return
	_idle_ms += delta * 1000.0
	if _idle_ms > demo_idle_ms:
		_idle_ms = 0.0
		_start_demo()


## title_screen.c: "fade_black(15); play_demo = true; return true;".
func _start_demo() -> void:
	idle = false
	_busy = true
	await fade_black(15)
	Game.start_demo()


## Any input resets the demo wait (newkey, newmouse or
## mouse movement: the C loop restarts and re-reads idleStartTick); a
## typed letter advances the title codes (new_text, last_text).
func handle_event(event: InputEvent) -> void:
	if (event is InputEventKey or event is InputEventJoypadButton) and event.is_pressed() \
			or TyrianMouse.is_button_press(event) \
			or (event is InputEventMouseMotion and event.relative != Vector2.ZERO):
		_idle_ms = 0.0
	if event is InputEventKey and event.pressed and not event.echo and not _busy and idle and not modal_active:
		var k := event as InputEventKey
		var c: int = k.unicode if k.unicode != 0 else int(k.keycode)
		if (c >= KEY_A and c <= KEY_Z) or (c >= 97 and c <= 122):
			if _type_code_letter(char(c).to_upper()):
				return
	super(event)


## Handling of an action (title_screen.c, `switch (lastkey_scan)`).
func handle_action(action: StringName) -> void:
	if modal_input(action):
		return
	if _busy or not idle:
		return
	menus.handle_action(action)
	selected = menus.selection()


## title_screen.c:titleScreen (new_text): a letter advances each
## of the eleven codes (specialName); after a failure, the letter is retested
## as the first letter of the code. Complete code: DESTRUCT opens the mini-game,
## ENGAGE the Super Tyrian game, the others a Super Arcade game.
## Returns true if a code fired (the screen is left).
func _type_code_letter(c: String) -> bool:
	for i in SPECIAL_NAME_COUNT - 1:  # i < SA_ENGAGE
		var code := TyrianText.L("specialName_%d" % i).to_upper()
		var progress := _code_progress[i]
		if progress >= code.length() or c != code[progress]:
			_code_progress[i] = 1 if code.length() > 0 and c == code[0] else 0
			continue
		_code_progress[i] = progress + 1
		if _code_progress[i] == code.length():
			_code_progress.fill(0)
			_code_complete(i)
			return true
	# ENGAGE (i = SA_ENGAGE - 1): same C loop, its bound being SA_ENGAGE.
	var engage := TyrianText.L("specialName_%d" % (SA_ENGAGE - 1)).to_upper()
	var k := _code_progress[SA_ENGAGE - 1]
	if k >= engage.length() or c != engage[k]:
		_code_progress[SA_ENGAGE - 1] = 1 if engage.length() > 0 and c == engage[0] else 0
	else:
		_code_progress[SA_ENGAGE - 1] = k + 1
		if _code_progress[SA_ENGAGE - 1] == engage.length():
			_code_progress.fill(0)
			_code_complete(SA_ENGAGE - 1)
			return true
	return false


func _code_complete(i: int) -> void:
	idle = false
	_busy = true
	if i + 1 == SA_DESTRUCT:
		# "fade_black(10); loadDestruct = true;": the Destruct mini-game.
		await fade_black(10)
		if ResourceLoader.exists(SCENE_DESTRUCT):
			Game.goto_scene(SCENE_DESTRUCT)
			return
		# Mini-game absent from this version: the title resumes.
		await _restart()
		_busy = false
		_redraw_selection()
		return
	var ng := load("res://scripts/new_game.gd")
	if i + 1 == SA_ENGAGE:
		# "JE_playSampleNum(V_DANGER); JE_whoa(); set_colors(black); newSuperTyrianGame".
		Sfx.play(V_DANGER)
		await whoa()
		vga.set_colors(Color.BLACK, 0, 255)
		ng.flow = ng.Flow.SUPER_TYRIAN
	else:
		# "fade_black(10); newSuperArcadeGame(i)".
		await fade_black(10)
		ng.flow = ng.Flow.SUPER_ARCADE
		ng.super_arcade = i
	Game.goto_scene(Game.SCENE_NEW_GAME)


## level_helpers.c:JE_whoa: "melting" fade of the screen, 300 turns of
## setDelay(1), interrupted by a key; JE_showVGA without JE_mouseStart.
func whoa() -> void:
	push_pointer_hidden()
	vga.whoa_begin(VGA_SCREEN)
	_input_seen = false
	for t in 300:
		vga.whoa_step(VGA_SCREEN)
		present()
		await wait_units(1)
		if _input_seen:
			break
	pop_pointer_hidden()


## True if letter `c` is the next one of a started code (it has already
## been counted by _type_code_letter, which runs before the shortcuts).
func _continues_code(c: String) -> bool:
	for i in SPECIAL_NAME_COUNT:
		var code := TyrianText.L("specialName_%d" % i).to_upper()
		var k := _code_progress[i]
		if k >= 1 and k <= code.length() and code[k - 1] == c and k >= 2:
			return true
	return false
## Mouse (title_screen.c:titleScreen): hovering a centered entry (text
## width) = selection (S_CURSOR), left click = activation, right click =
## Quit box (S_SPRING); shared engine, TITLE style.
func handle_mouse(event: InputEvent) -> void:
	# title_screen.c: mouseMoved or newmouse restarts the loop (idleStartTick).
	if (event is InputEventMouseMotion and event.relative != Vector2.ZERO) or TyrianMouse.is_button_press(event):
		_idle_ms = 0.0
	super(event)
	if not menus.stack.is_empty() and not _busy:
		selected = menus.selection()


## Access to the side modes (ADR 0050): no single equivalent C menu entry,
## the C code spreads them across "New Game", codes and submenus.
func _open_modes() -> void:
	idle = false
	Sfx.play(Sfx.SELECT)
	Game.goto_scene(Game.SCENE_MODES)


## Access to Endless mode (ADR 0071): the C code offers it in the New
## Game submenu (menus.c, MENU_ITEM_1/2_PLAYER_ENDLESS), absent from the rewrite;
## E = one player, 2 = two players.
func _start_endless(two_players: bool) -> void:
	Sfx.play(Sfx.SELECT)
	idle = false
	_busy = true
	await fade_black(10)
	# menus.c: "endless_resume_prompt() && endless_resume()" before
	# endless_start(two_p): choice screen if a game is saved.
	if not EndlessSession.enter(two_players):
		_busy = false


func activate() -> void:
	Sfx.play(Sfx.SELECT)
	_busy = true
	match selected:
		NEW_GAME:
			# "fade_black(15); if (newGame()) return true;": choice of mode,
			# episode and difficulty (menus.c), new_game screen.
			Game.modes = PackedInt32Array()
			await fade_black(15)
			Game.open_new_game()
		DEMO:
			# "fade_black(15); play_demo = true; return true;".
			await fade_black(15)
			Game.start_demo()
		SETUP:
			await fade_black(15)
			Game.goto_scene(Game.SCENE_SETUP)
		HIGH_SCORES:
			await fade_black(15)
			Game.goto_scene(SCENE_HIGH_SCORES)
		REPLAYS:
			await fade_black(15)
			Game.goto_scene(SCENE_REPLAYS)  # replay_browser
		ACHIEVEMENTS:
			await fade_black(15)
			Game.goto_scene(SCENE_ACHIEVEMENTS)
		LOAD_GAME:
			await fade_black(15)
			Game.goto_scene(SCENE_LOAD)  # JE_loadScreen
		INSTRUCTIONS:
			await fade_black(15)
			Game.goto_scene(SCENE_INSTRUCTIONS)  # JE_helpSystem(1)
		QUIT:
			_busy = false
			await _quit_or_restart()
		_:
			# Screens held by the campaign loop (loading, scores...) or
			# not yet ported: back to the title like `restart = true`.
			if Game.has_method("open_title_entry") and Game.open_title_entry(selected):
				return
			await _restart()
			_busy = false
			_redraw_selection()


## JE_quitRequestMsg then, on "No", `restart = true` (title redrawn,
## logo already at the top).
func _quit_or_restart() -> void:
	if await quit_request_msg(TyrianText.L("misc_quit_title"), TyrianText.L("misc_quit_desktop")):
		idle = false
		_busy = true
		await fade_black(15)
		Game.quit_cleanly()
		return
	idle = false
	_busy = true
	await _restart()
	_busy = false
	_redraw_selection()
