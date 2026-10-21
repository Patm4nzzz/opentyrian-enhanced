## Jukebox (jukebox.c:jukebox), opened by Setup → Jukebox as in the C
## (setup_menu.c:action_jukebox), rendered frame by frame on TyrianScreen
## (ADR 0020): starlib.c starfield (TyrianStarlib, Rust) under the
## vga_palette palette, texts in small font.
##
## The songs are those of the Music autoload (offline-rendered OGG or MIDI,
## depending on Setup > Sound). The C's keys: Left/Up and Right/Down/Enter
## change song, S stops, R restarts, X random, F fade, N fade
## at the end of the turn, G song played in a level, / effects ("," "." to
## choose, ";" to play), Space hides the text, Escape / Q / click
## quit; the starlib.c keys adjust the stars. The gamepad goes
## through the menu actions (push_joysticks_as_keyboard).
extends TyrianScreen

## jukebox.c:jb_order: Part 1 before Part 2, the rest in order.
const JB_ORDER := [
	1,
	0,
	2,
	3,
	4,
	5,
	6,
	7,
	8,
	9,
	10,
	11,
	12,
	13,
	14,
	15,
	16,
	17,
	18,
	19,
	20,
	21,
	22,
	23,
	24,
	25,
	26,
	27,
	28,
	29,
	30,
	31,
	32,
	33,
	34,
	35,
	36,
	37,
	38,
	39,
	40
]
const SHUFFLE_HISTORY_SIZE := 8
## sndmast.c:soundTitle (SOUND_COUNT = 31 effects + 9 voices): names of the
## original files, displayed as is like in the C.
const SOUND_TITLES := [
	"SCALEDN2",
	"F2",
	"TEMP10",
	"EXPLSM",
	"PASS3",
	"TEMP2",
	"BYPASS1",
	"EXP1RT",
	"EXPLLOW",
	"TEMP13",
	"EXPRETAP",
	"MT2BOOM",
	"TEMP3",
	"LAZB",
	"LAZGUN2",
	"SPRING",
	"WARNING",
	"ITEM",
	"HIT2",
	"MACHNGUN",
	"HYPERD2",
	"EXPLHUG",
	"CLINK1",
	"CLICK",
	"SCALEDN1",
	"TEMP11",
	"TEMP16",
	"SMALL1",
	"POWERUP",
	"MARS3",
	"NEEDLE2",
	"VOICE1",
	"VOICE2",
	"VOICE3",
	"VOICE4",
	"VOICE5",
	"VOICE6",
	"VOICE7",
	"VOICE8",
	"VOICE9"
]
const PALETTE_FADE_STEPS := 15
## "Jukebox" row of Setup, reselected on return (setup_menu.c:
## run_menu keeps its selection during the action): Graphics, Sound, Gameplay,
## Key Bindings, Joystick, Mods, Jukebox (setup_menu.c:setup_items).
const SETUP_ROW := 6

## starlib.c: statics, initialized on the first pass only
## (JE_starlib_init); the sky continues from one visit to the next.
static var _starlib: RefCounted
## jukebox.c: shuffle_history, static too.
static var _shuffle_history: Array[int] = []

## At rest for the capture tool: fade-in finished, key handled.
var idle: bool:
	get:
		return _running and _palette_fade_steps == 0 and _key == KEY_NONE and not _quitting
var _running := false
var _quitting := false
var _trigger_quit := false
var _hide_text := false
var _fade_looped_songs := true
var _fading_song := false
var _stopped := false
var _shuffle := false
var _fx := false
var _fx_num := 0
var _jb_pos := 0
var _palette_fade_steps := PALETTE_FADE_STEPS
var _fade_volume := 0
## Key of the frame (newkey / lastkey_scan): handled once per frame,
## after drawing, like the C loop.
var _key := KEY_NONE
var _shift := false
## starlib.c, P key: pause until the next key.
var _star_paused := false


func _ready() -> void:
	# jukebox.c does not call JE_mouseStart: no cursor, only a click
	# (any button) triggers the exit.
	mouse_cursor = false
	if _starlib == null and ClassDB.class_exists("TyrianStarlib"):
		_starlib = ClassDB.instantiate("TyrianStarlib")
		# The C's mt_rand: arbitrary state on entry; fixed seed in capture.
		_starlib.reseed(0 if Capture.active else randi())
	_jb_pos = maxi(JB_ORDER.find(Music.song_playing), 0)
	_fade_volume = Music.user_volume
	# init_step_fade_palette(diff, vga_palette, 0, 255): fade towards vga_palette.
	vga.load_vga_palette()
	vga.fade_palette_init(0, 255)
	_run.call_deferred()


func _run() -> void:
	_running = true
	while is_inside_tree():
		_frame()
		if _quitting and _palette_fade_steps == 0:
			break
		await wait_units(1)
	if not is_inside_tree():
		return
	_running = false
	_fading_song = false
	Music.set_volume(Music.user_volume)
	preload("res://scripts/setup.gd").entry_row = SETUP_ROW
	Game.goto_scene(Game.SCENE_SETUP)


## One turn of the jukebox.c loop.
func _frame() -> void:
	var song_count := mini(Music.song_count(), JB_ORDER.size())
	# jukebox.c: "if (!stopped && !audio_disabled)" — without song
	# rendering, neither fade nor automatic chaining.
	if not _stopped and Music.enabled and Music.has_audio() and song_count > 0:
		var looped := Music.song_looped()
		if looped and _fade_looped_songs:
			_fading_song = true
		if _fading_song:
			if _fade_volume > 5:
				_fade_volume -= 2
			else:
				_fade_volume = Music.user_volume
				_fading_song = false
			Music.set_volume(_fade_volume)
		if not Music.is_playing() or (looped and _fade_looped_songs and not _fading_song):
			_next_song(1)
	vga.clear(VGA_SCREEN, 0)
	if _starlib != null and not _star_paused:
		_starlib.step(vga, VGA_SCREEN, _star_key())
	if not _hide_text:
		_draw_text(song_count)
	if _palette_fade_steps > 0:
		vga.fade_step(_palette_fade_steps, 0, 255)
		_palette_fade_steps -= 1
	present()
	_apply_key(song_count)
	if _trigger_quit and not _quitting:
		_palette_fade_steps = PALETTE_FADE_STEPS
		vga.fade_solid_init(Color.BLACK, 0, 255)
		_quitting = true


func _draw_text(song_count: int) -> void:
	const X := 160
	TyrianText.draw_font_hv(self, X, 5, TyrianText.L("jb_header"), SMALL_FONT, CENTERED, 1, 4)
	if _fx:
		TyrianText.draw_font_hv(
			self, X, 25, TyrianText.L("jb_sfx") % [_fx_num + 1, SOUND_TITLES.size()], SMALL_FONT, CENTERED, 1, 2
		)
		TyrianText.draw_font_hv(self, X, 40, SOUND_TITLES[_fx_num], SMALL_FONT, CENTERED, 1, 4)
	else:
		TyrianText.draw_font_hv(
			self, X, 25, TyrianText.L("jb_track") % [_jb_pos + 1, song_count], SMALL_FONT, CENTERED, 1, 2
		)
		TyrianText.draw_font_hv(self, X, 40, Music.title(JB_ORDER[_jb_pos]), SMALL_FONT, CENTERED, 1, 4)
	var status := (
		TyrianText.L("jb_stopped")
		if _stopped
		else TyrianText.L("jb_fading") if _fading_song else TyrianText.L("jb_playing")
	)
	TyrianText.draw_font_hv(self, X, 60, status, SMALL_FONT, CENTERED, 1, 2 if _stopped else 4)
	TyrianText.draw_font_hv(
		self,
		X,
		75,
		TyrianText.L("jb_shuffle") % TyrianText.L("on" if _shuffle else "off"),
		SMALL_FONT,
		CENTERED,
		1,
		4 if _shuffle else 2
	)
	if Music.ingame_song_override >= 0:
		TyrianText.draw_font_hv(
			self, X, 90, TyrianText.L("jb_ingame") % Music.title(Music.ingame_song_override), SMALL_FONT, CENTERED, 1, 2
		)
		TyrianText.draw_font_hv(self, X, 100, TyrianText.L("jb_ingame_hint"), SMALL_FONT, CENTERED, 1, 2)
	TyrianText.draw_font_hv(self, X, 155, TyrianText.L("jb_ctrl1"), SMALL_FONT, CENTERED, 1, 2)
	TyrianText.draw_font_hv(self, X, 165, TyrianText.L("jb_ctrl2"), SMALL_FONT, CENTERED, 1, 2)
	TyrianText.draw_font_hv(self, X, 175, TyrianText.L("jb_ctrl3"), SMALL_FONT, CENTERED, 1, 2)


## Next song (dir = 1) or previous (-1), or drawn at random.
func _next_song(dir: int) -> void:
	var song_count := mini(Music.song_count(), JB_ORDER.size())
	if song_count == 0:
		return
	if _shuffle:
		Music.play_song(_shuffle_next_song(song_count))
		_jb_pos = maxi(JB_ORDER.find(Music.song_playing), 0)
	else:
		_jb_pos = posmod(_jb_pos + dir, song_count)
		Music.play_song(JB_ORDER[_jb_pos])


## jukebox.c:shuffle_next_song: at most 20 draws to avoid the last eight
## songs.
func _shuffle_next_song(song_count: int) -> int:
	var song := 0
	var attempts := 0
	while true:
		song = randi() % song_count
		attempts += 1
		if attempts >= 20 or _shuffle_history.is_empty() or not _shuffle_history.has(song):
			break
	_shuffle_history.append(song)
	if _shuffle_history.size() > SHUFFLE_HISTORY_SIZE:
		_shuffle_history.pop_front()
	return song


## switch (lastkey_scan) of jukebox.c.
func _apply_key(song_count: int) -> void:
	var key := _key
	_key = KEY_NONE
	if key == KEY_NONE or song_count == 0:
		return
	match key:
		KEY_ESCAPE, KEY_Q:
			_trigger_quit = true
		KEY_SPACE:
			_hide_text = not _hide_text
		KEY_F:
			_fading_song = not _fading_song
		KEY_N:
			_fade_looped_songs = not _fade_looped_songs
		KEY_X:
			_shuffle = not _shuffle
			if not _shuffle:
				_shuffle_history.clear()
		KEY_G:
			# Toggle: G on the already selected song cancels it.
			if not _fx and Music.song_playing >= 0:
				Music.ingame_song_override = (
					-1 if Music.ingame_song_override == Music.song_playing else Music.song_playing
				)
		KEY_SLASH:
			_fx = not _fx
		KEY_COMMA:
			if _fx:
				_fx_num = posmod(_fx_num - 1, SOUND_TITLES.size())
		KEY_PERIOD:
			if _fx:
				_fx_num = (_fx_num + 1) % SOUND_TITLES.size()
		KEY_SEMICOLON:
			if _fx:
				Sfx.play(_fx_num + 1)
		KEY_LEFT, KEY_UP:
			_next_song(-1)
			_stopped = false
		KEY_ENTER, KEY_RIGHT, KEY_DOWN:
			_next_song(1)
			_stopped = false
		KEY_S:
			Music.stop_song()
			_stopped = true
		KEY_R:
			Music.restart_song()
			_stopped = false


## starlib.c: character of the frame's key (scancodes, not the
## keyboard layout), empty if none.
func _star_key() -> String:
	const PLAIN := {
		KEY_C: "c",
		KEY_P: "p",
		KEY_S: "s",
		KEY_X: "x",
		KEY_1: "1",
		KEY_2: "2",
		KEY_3: "3",
		KEY_4: "4",
		KEY_5: "5",
		KEY_6: "6",
		KEY_7: "7",
		KEY_8: "8",
		KEY_9: "9",
		KEY_0: "0",
		KEY_MINUS: "-",
		KEY_BRACKETLEFT: "[",
		KEY_BRACKETRIGHT: "]",
		KEY_QUOTELEFT: "`",
		KEY_KP_SUBTRACT: "-",
		KEY_KP_ADD: "+",
		KEY_KP_1: "1",
		KEY_KP_2: "2",
		KEY_KP_3: "3",
		KEY_KP_4: "4",
		KEY_KP_5: "5",
		KEY_KP_6: "6",
		KEY_KP_7: "7",
		KEY_KP_8: "8",
		KEY_KP_9: "9",
		KEY_KP_0: "0"
	}
	const SHIFTED := {
		KEY_C: "C",
		KEY_P: "P",
		KEY_S: "S",
		KEY_X: "X",
		KEY_1: "!",
		KEY_2: "@",
		KEY_3: "#",
		KEY_4: "$",
		KEY_EQUAL: "+",
		KEY_BRACKETLEFT: "{",
		KEY_BRACKETRIGHT: "}"
	}
	var c: String = (SHIFTED if _shift else PLAIN).get(_key, "")
	if c == "p" or c == "P":
		_star_paused = true
		return ""
	return c


## jukebox.c: "quit on mouse click". Mouse events do not reach
## handle_event (TyrianScreen._unhandled_input sends them here).
func handle_mouse(event: InputEvent) -> void:
	if _quitting:
		return
	if TyrianMouse.is_button_press(event):
		_trigger_quit = true


## Keys: keyboard by scancode (like lastkey_scan), click = quit,
## gamepad through the menu actions.
func handle_event(event: InputEvent) -> void:
	if _quitting:
		return
	if event is InputEventMouseButton:
		if TyrianMouse.is_button_press(event):
			_trigger_quit = true
		return
	if event is InputEventKey:
		if event.pressed and not event.echo:
			if _star_paused:
				_star_paused = false
				return
			_key = event.physical_keycode if event.physical_keycode != KEY_NONE else event.keycode
			_shift = event.shift_pressed
		return
	super.handle_event(event)


## Gamepad (push_joysticks_as_keyboard): same keys as the keyboard.
func handle_action(action: StringName) -> void:
	const KEYS := {
		TyrianControls.UP: KEY_UP,
		TyrianControls.DOWN: KEY_DOWN,
		TyrianControls.LEFT: KEY_LEFT,
		TyrianControls.RIGHT: KEY_RIGHT,
		TyrianControls.ACCEPT: KEY_ENTER,
		TyrianControls.BACK: KEY_ESCAPE
	}
	if KEYS.has(action) and not _quitting:
		_key = KEYS[action]
		_shift = false
