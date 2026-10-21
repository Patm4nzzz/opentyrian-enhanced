## Display settings, outside the simulation (ADR 0100, ADR 0124): a single
## file for persistence (userdata/display.cfg), application and screen.
##
## Persistence: one saved setting, graphics rendering
## (`[display] art_mode`): Original (original pixels, default), Sharp pixel art
## (xBRZ freescale filter at display time, shaders/xbrz_freescale.gdshaderinc,
## no dedicated GPU or file) or HD (AI): images upscaled ×4 by the
## 4x-PixelPerfectV4 model (`upscaled/`, ADR 0100) — scenery as whole
## level maps, no tile grid — and game viewport rendered at 4× its
## logical size. HD (AI) is only offered if `upscaled/` is present;
## `--hd-sprites[=0|1]` forces it (captures).
## The C game has no equivalent for the HD mode. The other graphics
## settings of Setup (graphics_menu.c) live here, in the same `[display]`
## file — a single source of truth: the CRT filter (`crt_mode`, six
## modes of video.h/CrtMode), the scaler (`scaler`, nine algorithms of
## video_scale.c:scalers[]) and the scaling mode (`scaling_mode`,
## six modes of video.c:scaling_mode_names[]). Bullet time (F4) is NO
## LONGER there: it is a runtime state of the level (bullet_time.c), never saved;
## an old `[display] bullet_time` key is ignored and removed from the file.
##
## `[display] crt` (boolean from the old version) is migrated on load:
## true → CRT_PLUS (closest mode: curvature, scanlines and
## vignetting). The key is no longer written.
##
## Screen: scenes/display_options.tscn (this script), opened by F10 from
## any screen (effects.gd); the options menus of the other batches
## can redirect to it with `Game.goto_scene(DisplaySettings.SCENE_PATH)`.
## Enter / Space: toggle; Escape: back to the title screen, or to the
## suspended game if F10 was pressed during a level.
class_name DisplaySettings
extends Node2D

const SCENE_PATH := "res://scenes/display_options.tscn"
static var PATH: String = UserData.path("display.cfg")
## Folder of the upscaled PNGs: `res://assets/upscaled` in the project (the
## script drops a `.gdignore` there: Godot does not import them, they are read
## with Image.load_from_file); exported game: `upscaled/` next to the executable
## (macOS: in the bundle resources or the player data folder, GamePaths).
const UPSCALED_RES := "res://assets/upscaled"
static var UPSCALED: String = _upscaled_dir()
## Factor of the 4x-PixelPerfectV4 model.
const HD_SCALE := 4

## Graphics rendering (`[display] art_mode`).
enum ArtMode { ORIGINAL, SHARP, HD }
## Screen modes, in toggle order (HD only if its images
## are present: screen_art_modes).
const SCREEN_ART_MODES := [ArtMode.ORIGINAL, ArtMode.SHARP, ArtMode.HD]
const ART_MODE_KEYS := ["gd_art_original", "gd_art_sharp", "gd_art_hd"]

## CRT filter (`[display] crt_mode`): six modes of video.h:CrtMode /
## graphics_menu.c:crt_set. The label comes from the C locale keys.
enum CrtMode { OFF, SCANLINES, PLUS, PREMIUM, APERTURE, SOFT }
const CRT_MODE_KEYS := [
	"off", "crt_scanlines_label", "crt_plus_label", "crt_premium_label", "crt_aperture_label", "crt_soft_label"
]

## Scaler (`[display] scaler`): the nine algorithms of video_scale.c:scalers[],
## in their order (None, 2x, Scale2x, hq2x, 3x, Scale3x, hq3x, 4x, hq4x) and
## with their factor (video.c:init_scaler sizes the window).
const SCALER_KEYS := [
	"gfx_scaler_none",
	"gfx_scaler_2x",
	"gfx_scaler_scale2x",
	"gfx_scaler_hq2x",
	"gfx_scaler_3x",
	"gfx_scaler_scale3x",
	"gfx_scaler_hq3x",
	"gfx_scaler_4x",
	"gfx_scaler_hq4x"
]
const SCALER_FACTORS := [1, 2, 2, 2, 3, 3, 3, 4, 4]

## Scaling Mode (`[display] scaling_mode`): six modes of
## video.c:scaling_mode_names[ScalingMode].
const SCALING_MODE_KEYS := [
	"gfx_scale_center", "gfx_scale_integer", "gfx_scale_8_5", "gfx_scale_4_3", "gfx_scale_16_9", "gfx_scale_fill"
]

static var _loaded := false
static var _art_mode := ArtMode.ORIGINAL
## Saved CRT filter (`[display] crt_mode`), re-read at startup by Effects.
static var _crt_mode := CrtMode.OFF
const DEFAULT_SCALER := 5
static var _scaler := DEFAULT_SCALER  # video_scale.c: Scale3x on first launch
static var _scaling_mode := 1  # video.c:scaling_mode = SCALE_INTEGER
## Gamma correction (`[display] gamma`, 0..3: save_json.c:video.gamma),
## changed in a level by the Gamma key (mainint.c:JE_gammaCheck).
const GAMMA_LEVELS := 4
static var _gamma := 0

# --- Persistence -----------------------------------------------------------


## Preference for the folder shipped with the exported package (next to the
## executable; macOS: bundle resources, then the player data folder: GamePaths).
static func _upscaled_dir() -> String:
	if not OS.has_feature("editor"):
		for candidate in GamePaths.upscaled_candidates():
			if DirAccess.dir_exists_absolute(candidate):
				return candidate
	return ProjectSettings.globalize_path(UPSCALED_RES)


static func _load() -> void:
	if _loaded:
		return
	_loaded = true
	var cfg := ConfigFile.new()
	_art_mode = ArtMode.ORIGINAL
	_crt_mode = CrtMode.OFF
	_scaler = DEFAULT_SCALER
	_scaling_mode = 1
	_gamma = 0
	if cfg.load(PATH) == OK:
		_gamma = clampi(int(cfg.get_value("display", "gamma", 0)), 0, GAMMA_LEVELS - 1)
		var v := int(cfg.get_value("display", "art_mode", ArtMode.ORIGINAL))
		if v >= 0 and v < ArtMode.size():
			_art_mode = v as ArtMode
		if cfg.has_section_key("display", "crt_mode"):
			_crt_mode = clampi(int(cfg.get_value("display", "crt_mode")), 0, CrtMode.size() - 1) as CrtMode
		elif cfg.has_section_key("display", "crt"):
			# Old boolean: true = the closest of the six modes.
			_crt_mode = CrtMode.PLUS if bool(cfg.get_value("display", "crt")) else CrtMode.OFF
		_scaler = clampi(int(cfg.get_value("display", "scaler", DEFAULT_SCALER)), 0, SCALER_FACTORS.size() - 1)
		_scaling_mode = clampi(int(cfg.get_value("display", "scaling_mode", 1)), 0, SCALING_MODE_KEYS.size() - 1)
		# Old key (bullet time state saved by mistake: tinted
		# the whole game at launch): ignored and removed from the file.
		if cfg.has_section_key("display", "bullet_time"):
			cfg.erase_section_key("display", "bullet_time")
			cfg.save(PATH)
	# Command line (captures, tests): takes precedence over the file.
	for arg in OS.get_cmdline_user_args():
		if arg == "--hd-sprites" or arg == "--hd-sprites=1":
			_art_mode = ArtMode.HD
		elif arg == "--hd-sprites=0" and _art_mode == ArtMode.HD:
			_art_mode = ArtMode.ORIGINAL
	# Light package (without upscaled/): the requested HD mode does not apply;
	# the screen shows Original (effective_art_mode) and the log says so.
	if _art_mode == ArtMode.HD and not hd_available():
		var beside := " or ".join(GamePaths.upscaled_candidates())
		push_warning(
			(
				"Display: HD (AI) requested, %s is missing (HD pack not installed): falling back to Original rendering"
				% beside
			)
		)


## The upscaled PNGs are present (regenerable folder, not versioned).
static func hd_available() -> bool:
	return DirAccess.dir_exists_absolute(UPSCALED.path_join("sprites"))


## HD (AI) mode actually applied: requested AND PNGs present.
static func hd_sprites() -> bool:
	_load()
	return _art_mode == ArtMode.HD and hd_available()


## Requested mode (saved or forced by the command line).
static func art_mode() -> ArtMode:
	_load()
	return _art_mode


## Mode actually rendered, the one the menus display: HD (AI) requested without
## its images (light package, HD pack absent) renders as Original. The saved
## choice is kept: it comes back as soon as the pack is installed.
static func effective_art_mode() -> ArtMode:
	_load()
	return ArtMode.ORIGINAL if _art_mode == ArtMode.HD and not hd_available() else _art_mode


static func set_art_mode(mode: ArtMode) -> void:
	_load()
	_art_mode = mode
	_save("art_mode", int(mode))


## Saved CRT filter (graphics_menu.c:crt_get), six modes.
static func crt_mode() -> int:
	_load()
	return _crt_mode


static func set_crt_mode(mode: int) -> void:
	_load()
	_crt_mode = clampi(mode, 0, CrtMode.size() - 1)
	_save("crt_mode", int(_crt_mode))


## Saved gamma (0..3) and switch to the next one (mainint.c:JE_gammaCheck:
## `gammaCorrection = (gammaCorrection + 1) % 4`).
static func gamma() -> int:
	_load()
	return _gamma


static func cycle_gamma() -> int:
	_load()
	_gamma = (_gamma + 1) % GAMMA_LEVELS
	_save("gamma", _gamma)
	return _gamma


## A CRT is active (compatibility of `Effects.crt`).
static func crt() -> bool:
	return crt_mode() != CrtMode.OFF


## Saved scaler (graphics_menu.c:scaler_get), index in video_scale.c:scalers[].
static func scaler() -> int:
	_load()
	return _scaler


static func set_scaler(i: int) -> void:
	_load()
	_scaler = clampi(i, 0, SCALER_FACTORS.size() - 1)
	_save("scaler", _scaler)


## Screen factor of the scaler (video.c:init_scaler: window = factor × 320×200).
static func scaler_factor() -> int:
	return SCALER_FACTORS[scaler()]


## Saved Scaling Mode (graphics_menu.c:sm_get).
static func scaling_mode() -> int:
	_load()
	return _scaling_mode


static func set_scaling_mode(i: int) -> void:
	_load()
	_scaling_mode = clampi(i, 0, SCALING_MODE_KEYS.size() - 1)
	_save("scaling_mode", _scaling_mode)


## A key of `[display]`, the others preserved.
static func _save(key: String, value: Variant) -> void:
	var cfg := ConfigFile.new()
	cfg.load(PATH)
	cfg.set_value("display", key, value)
	cfg.save(PATH)


# --- Application -----------------------------------------------------------


## Offered modes: HD (AI) removed when `upscaled/` is missing.
static func screen_art_modes() -> Array:
	return SCREEN_ART_MODES if hd_available() else SCREEN_ART_MODES.filter(func(m: int) -> bool: return m != ArtMode.HD)


## Next mode among the screen's ones, saved then applied: xBRZ
## filter, atlas caches, viewport size and scenery of the current level
## (Effects.apply_display_settings).
static func cycle_art_mode(dir := 1) -> void:
	var modes := screen_art_modes()
	var i := modes.find(effective_art_mode())
	set_art_mode(modes[(i + dir + modes.size()) % modes.size()])
	Effects.apply_display_settings()


# --- Screen (scenes/display_options.tscn) -----------------------------------

var selected := 0
## Backdrop and 8-bit framebuffer of the screen (TyrianScreen.menu_backdrop).
var screen: TyrianScreen


func _ready() -> void:
	# Rewrite cursor (screen absent from the 8-bit C).
	add_child(MouseCursor.new())
	screen = TyrianScreen.menu_backdrop(self)
	if Effects.level_paused():
		screen.modulate.a = 0.8  # suspended game visible underneath
	_redraw()


func _items() -> Array:
	return [
		[TyrianText.L("gd_display_art") % TyrianText.L(ART_MODE_KEYS[effective_art_mode()]), _cycle_art],
		[TyrianText.L("gd_display_crt") % TyrianText.L(CRT_MODE_KEYS[crt_mode()]), _cycle_crt],
		[TyrianText.L("gd_display_scaler") % TyrianText.L(SCALER_KEYS[scaler()]), _cycle_scaler],
		[TyrianText.L("gd_display_scale_mode") % TyrianText.L(SCALING_MODE_KEYS[scaling_mode()]), _cycle_scaling_mode],
		[TyrianText.L("gd_back"), _leave],
	]


## Rows receive the direction (+1 Right / Enter, -1 Left) and cycle
## their value in both directions, like Setup > Graphics.
func _cycle_art(dir: int) -> void:
	cycle_art_mode(dir)


func _cycle_crt(dir: int) -> void:
	Effects.set_crt_mode((crt_mode() + dir + CrtMode.size()) % CrtMode.size())


## Next scaler, saved then applied (screen factor, shader).
func _cycle_scaler(dir: int) -> void:
	set_scaler((scaler() + dir + SCALER_FACTORS.size()) % SCALER_FACTORS.size())
	Effects.apply_display_settings()


## Next Scaling Mode, saved then applied to the window.
func _cycle_scaling_mode(dir: int) -> void:
	set_scaling_mode((scaling_mode() + dir + SCALING_MODE_KEYS.size()) % SCALING_MODE_KEYS.size())
	Effects.apply_display_settings()


## Back: to the suspended game (F10 during a level), otherwise to the title.
func _leave(_dir := 1) -> void:
	if not Effects.close_options():
		Game.goto_scene(Game.SCENE_TITLE)


## Drawing into the 8-bit framebuffer on image 2 and its palette, C
## colors: title like menu_tree.c:run_menu (draw_font_hv_shadow 15, -4, shadow 2);
## rows in small font like mod_manager.c:1549 (JE_textShade 15, 4 on the
## selected row, 2 otherwise, FULL_SHADE; grayed option: tint 7, 2); help
## JE_textShade(15, 4, PART_SHADE). Screen specific to the rewrite.
func _redraw() -> void:
	if screen == null:
		return
	screen.begin_menu_frame()
	TyrianText.draw_font_hv_shadow(
		screen,
		160,
		20,
		TyrianText.L("gd_display_title"),
		TyrianScreen.NORMAL_FONT,
		TyrianScreen.CENTERED,
		15,
		-4,
		false,
		2
	)
	var items := _items()
	for i in items.size():
		var hue := 15 if (items[i][1] as Callable).is_valid() else 7
		TyrianText.text_shade(
			screen, 30, 60 + i * 14, items[i][0], hue, 4 if i == selected else 2, TyrianText.FULL_SHADE
		)
	TyrianText.text_shade(screen, 30, 180, TyrianText.L("gd_display_keys"), 15, 4, TyrianText.PART_SHADE)
	screen.present()


## Delegates a keyboard action to the _unhandled_input handler (called by
## handle_mouse): avoids duplicating the navigation logic.
func handle_action(action: StringName) -> void:
	var ev := InputEventAction.new()
	ev.action = action
	ev.pressed = true
	_unhandled_input(ev)


## Mouse (screen specific to the rewrite, no C equivalent): hover = selected
## row, left click = Enter, right click = back, wheel = one row.
func handle_mouse(event: InputEvent) -> void:
	if TyrianMouse.is_wheel(event):
		return
	var p := Vector2i(event.position.floor())
	var i := (p.y - 60) / 14
	var inside: bool = p.x >= 30 and p.x < 300 and p.y >= 60 and i >= 0 and i < _items().size()
	if event is InputEventMouseMotion:
		if inside and not TyrianMouse.inactive and event.relative != Vector2.ZERO and i != selected:
			selected = i
			Sfx.play(Sfx.CURSOR)
			_redraw()
		return
	if not TyrianMouse.is_button_press(event):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		handle_action(TyrianControls.BACK)
		return
	if inside:
		selected = i
		handle_action(TyrianControls.ACCEPT)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouse:
		handle_mouse(event)
		return
	var action := TyrianControls.menu_action(event)
	if action == &"":
		return
	var n := _items().size()
	match action:
		TyrianControls.UP:
			selected = (selected + n - 1) % n
			Sfx.play(Sfx.CURSOR)
		TyrianControls.DOWN:
			selected = (selected + 1) % n
			Sfx.play(Sfx.CURSOR)
		TyrianControls.LEFT, TyrianControls.RIGHT:
			# No value to change on "Back" or on a grayed option.
			var entry: Callable = _items()[selected][1]
			if not entry.is_valid() or selected == n - 1:
				return
			Sfx.play(Sfx.CURSOR)
			entry.call(-1 if action == TyrianControls.LEFT else 1)
		TyrianControls.ACCEPT, TyrianControls.SELECT:
			var entry: Callable = _items()[selected][1]
			if not entry.is_valid():  # grayed option
				return
			Sfx.play(Sfx.SELECT)
			entry.call(1)
			if not is_inside_tree():
				return
		TyrianControls.BACK:
			_leave()
			return
		_:
			return
	_redraw()
