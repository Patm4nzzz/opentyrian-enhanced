## Post-processing and scene host (autoload `Effects`).
##
## The game scenes are rendered in a 320×200 SubViewport (exact
## pixels, like VGAScreen); this viewport is displayed fullscreen by a
## TextureRect whose material applies, at the window resolution:
##   CRT filter, six modes (Off, Scanlines, Plus, Premium, Aperture, Soft:
##        graphics_menu.c:crt, shader.c), set in Setup > Graphics;
##   bullet time in LEVELS only, like the C (bullet_time.c): 4 s of
##        slow motion (the level skips 3 ticks out of 4, see consume_tick) without any tint
##        (the C only has ">> SLOW MOTION <<"), then 15 s of cooldown.
##        Never saved; triggered by left Shift (level_hotkeys.gd);
##   gamma (mainint.c:JE_gammaCorrect), F11 key in a level;
##   outside a level: F2 CRT filter, F3 graphics rendering, F9 controls, F10
##        display (the C binds nothing there);
##   in HD mode (AI) the viewport is rendered at 4× (1280×800) with the same
##        320×200 logical coordinates (ADR 0100).
## The level shortcuts (F2 to F12, Shift) are in level_hotkeys.gd: no display
## key is left here. Nothing here is read by the simulation.
extends Node

const SetupScreen := preload("res://scripts/setup.gd")
const GAME_SIZE := Vector2i(320, 200)
## Post-processing: light variant by default, xBRZ variant when the "Sharp pixel art" mode
## or an hq2x / hq3x / hq4x scaler requires it (compiled on the first frame, see post_body).
const POST_SHADER := preload("res://shaders/post.gdshader")
const POST_SHADER_XBRZ := preload("res://shaders/post_xbrz.gdshader")
## Scaler indices that go through xBRZ (video_scale.c:scalers[]: hq2x, hq3x, hq4x).
const XBRZ_SCALERS := [3, 6, 8]
## bullet_time.h : BULLET_TIME_DURATION_MS, BULLET_TIME_COOLDOWN_MS, BULLET_TIME_SPEED.
const BULLET_TIME_DURATION_MS := 4000
const BULLET_TIME_COOLDOWN_MS := 15000
const BULLET_TIME_SCALE := 0.25

signal effects_changed

## Current CRT filter (graphics_menu.c:crt_get: six modes, 0 = off).
var crt_mode: int = DisplaySettings.CrtMode.OFF:
	set(v):
		crt_mode = clampi(v, 0, DisplaySettings.CrtMode.size() - 1)
		crt = crt_mode != DisplaySettings.CrtMode.OFF
		_apply()
## Boolean compatibility (tools/screenshots, F2): true if a CRT mode is
## active; turning it on without a chosen mode picks the closest one (CRT Plus).
var crt := false:
	set(v):
		crt = v
		if v and crt_mode == DisplaySettings.CrtMode.OFF:
			crt_mode = DisplaySettings.CrtMode.PLUS
		elif not v and crt_mode != DisplaySettings.CrtMode.OFF:
			crt_mode = DisplaySettings.CrtMode.OFF
		else:
			_apply()
var xbrz := false:
	set(v):
		xbrz = v
		_apply()
## Scaler (video_scale.c:scalers[]) and Scaling Mode (video.c:scaling_mode):
## indices saved in display.cfg, applied to the window.
var scaler := 0:
	set(v):
		var changed := v != scaler
		scaler = v
		_apply()
		# Like video.c:init_scaler: the window is only resized when the
		# scaler changes (F3 and the other settings do not touch it).
		_apply_scale(changed)
var scaling_mode := 1:
	set(v):
		scaling_mode = v
		_apply_scale(false)
## bullet_time.c:bullet_time_active - runtime state only, never read
## from nor written to a file; only true inside a level.
var bullet_time := false:
	set(v):
		bullet_time = v
		_apply()
## bullet_time.c:bullet_time_end / bullet_time_cooldown_end (ms, clock
## `bullet_time_clock`) and bullet_time_uses.
var _bullet_time_end := 0
var _bullet_time_cooldown_end := 0
var bullet_time_uses := 0
var _bullet_time_last_update := 0
## Clock in ms (replaceable by tests); bullet_time_update samples it
## every frame.
var bullet_time_clock: Callable = func() -> int: return Time.get_ticks_msec()

var game_viewport: SubViewport
## Repeat of directions held on the gamepad in menus
## (joystick.c, 300 ms) and its clock (ms; replaceable by tests).
var menu_repeat := MenuRepeat.new()
var repeat_clock: Callable = func() -> int: return Time.get_ticks_msec()
## Repeated presses sent to the menus (measurement, tests).
var repeat_pushed := 0
var _layer: CanvasLayer
var _screen: TextureRect
## Position of the image without shake, and integer screen-shake offset
## (screenshake.c, video.c:449-455: added to the destination rectangle).
var _screen_base := Vector2.ZERO
var shake_px := Vector2i.ZERO:
	set(v):
		if v == shake_px:
			return
		shake_px = v
		_apply_shake_position()
var _material: ShaderMaterial


## video.c:449-455: the shake is added to the output rectangle, so in WINDOW
## pixels. `_screen` lives in the logical base that content_scale multiplies by K: the
## offset is divided by K so that the image moves by `shake_px` window pixels.
func _apply_shake_position() -> void:
	if _screen == null:
		return
	_screen.position = _screen_base + Vector2(shake_px) / _content_scale_factor()


func _content_scale_factor() -> Vector2:
	var k := get_window().get_final_transform().get_scale()
	return Vector2(maxf(k.x, 0.0001), maxf(k.y, 0.0001))


## Offset of the image on screen, in window pixels, caused by the shake.
func shake_window_offset() -> Vector2:
	if _screen == null:
		return Vector2.ZERO
	return (_screen.position - _screen_base) * _content_scale_factor()


## hitstop.c:kill_slowmo_update: speed 0.3 -> 1 after a defeated boss
## (set by level.gd every tick; -1 outside slow motion).
var kill_slowmo := -1.0:
	set(v):
		if v != kill_slowmo:
			kill_slowmo = v
			_apply_time_scale()
## Setup > Gameplay: bullet time allowed (bullet_time.c:bullet_time_enabled).
var bullet_time_allowed := true:
	set(v):
		bullet_time_allowed = v
		if not v:
			bullet_time = false
		_apply()


func _ready() -> void:
	game_viewport = SubViewport.new()
	game_viewport.name = "GameViewport"
	game_viewport.size = GAME_SIZE
	game_viewport.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
	game_viewport.snap_2d_transforms_to_pixel = true
	game_viewport.snap_2d_vertices_to_pixel = true
	game_viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	game_viewport.audio_listener_enable_2d = true
	add_child(game_viewport)
	_apply_viewport_scale()
	xbrz = DisplaySettings.art_mode() == DisplaySettings.ArtMode.SHARP
	# Saved settings (userdata/display.cfg); Setup can then
	# forbid bullet time (bullet_time_allowed, setup.gd).
	crt_mode = DisplaySettings.crt_mode()
	crt = crt_mode != DisplaySettings.CrtMode.OFF
	scaler = DisplaySettings.scaler()
	scaling_mode = DisplaySettings.scaling_mode()
	# Resize by the player: layout only, the window keeps
	# the chosen size (otherwise it would immediately go back to K × 320×200).
	get_window().size_changed.connect(_apply_scale.bind(false))

	_layer = CanvasLayer.new()
	_layer.layer = 100
	add_child(_layer)
	_screen = TextureRect.new()
	_screen.texture = game_viewport.get_texture()
	# Fixed 320×200 size whatever the texture (1280×800 in HD mode):
	# set BEFORE size, otherwise the minimum size (= texture) wins.
	_screen.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_screen.size = Vector2(GAME_SIZE)
	_screen.stretch_mode = TextureRect.STRETCH_SCALE
	_screen.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_material = ShaderMaterial.new()
	_material.shader = POST_SHADER
	_material.set_shader_parameter("source_tex", game_viewport.get_texture())
	_screen.material = _material
	_layer.add_child(_screen)
	_apply()
	# Scaler + Scaling Mode: applied after `_screen` is created (the
	# `_ready` setters stayed silent while it did not exist).
	_apply_scale()
	# The main scene is instantiated by Godot under the root:
	# move it into the game viewport.
	_adopt_current_scene.call_deferred()


func _adopt_current_scene() -> void:
	var scene := get_tree().current_scene
	if scene == null or scene.get_parent() == game_viewport:
		return
	scene.get_parent().remove_child(scene)
	game_viewport.add_child(scene)


## Scene left by the last show_scene (empty on first display).
var previous_scene_path := ""


## Instantiated scene, or null if it fails to load. The exported (release)
## binary crashes (SIGSEGV) on `(null as PackedScene).instantiate()` instead
## of raising a script error: a .pck replaced under a running game was enough.
static func instantiate_scene(path: String) -> Node:
	var packed: PackedScene = null
	if ResourceLoader.exists(path):
		packed = load(path) as PackedScene
	if packed == null:
		printerr("Unreadable scene: %s" % path)
		return null
	return packed.instantiate()


## Replaces the displayed scene (equivalent of change_scene_to_file). Unreadable
## scene: the current screen stays displayed and null is returned.
func show_scene(path: String) -> Node:
	var scene := instantiate_scene(path)
	if scene == null:
		return null
	previous_scene_path = ""
	for child in game_viewport.get_children():
		previous_scene_path = child.scene_file_path
		child.queue_free()
		game_viewport.remove_child(child)
	overlay_screen = null
	_paused_level = null
	bullet_time_reset()
	Music.set_paused(false)
	game_viewport.add_child(scene)
	return scene


## Scene that receives input: the overlay screen if there is one.
func current_game_scene() -> Node:
	if overlay_screen != null and is_instance_valid(overlay_screen):
		return overlay_screen
	for child in game_viewport.get_children():
		if not child.is_queued_for_deletion():
			return child
	return null


# --- Overlay during a level (F9 / F10) -------------------------------

## Options screen opened on top of a paused level; null otherwise.
var overlay_screen: Node
const OVERLAY_Z := 100
## Level suspended under `overlay_screen` (simulation stopped).
var _paused_level: Node


## True when a level is suspended under an options screen.
func level_paused() -> bool:
	return _paused_level != null


## F9 / F10: the screen opens on top of the current screen, which is suspended
## (process_mode DISABLED: no more _process or tick), and Esc returns to it.
## From the shop, replacing the screen used to go back to the title and lose the
## game in progress. Only the title is replaced (Esc returns to it anyway).
func open_options(path: String) -> Node:
	var cur := current_game_scene()
	if cur != null and cur.scene_file_path == path:
		return cur  # already open: no second identical screen on top
	var over_scene := (
		overlay_screen != null and is_instance_valid(overlay_screen)
		or (cur != null and cur.scene_file_path != Game.SCENE_TITLE)
	)
	if not over_scene:
		return show_scene(path)
	var screen := instantiate_scene(path)
	if screen == null:
		return null
	if overlay_screen != null and is_instance_valid(overlay_screen):
		# Switching from F9 to F10 (or the reverse) on the same suspended level.
		overlay_screen.queue_free()
		game_viewport.remove_child(overlay_screen)
		overlay_screen = null
	else:
		_paused_level = cur
		_paused_level.process_mode = Node.PROCESS_MODE_DISABLED
		if _is_level(cur):
			Music.set_paused(true)
	overlay_screen = screen
	# Above all the level layers (z_index 0..Z.HUD in level.gd).
	if overlay_screen is CanvasItem:
		overlay_screen.z_index = OVERLAY_Z
	game_viewport.add_child(overlay_screen)
	return overlay_screen


## Closes the overlay screen and resumes the level where it was.
## Returns false if there is no overlay (the screen then goes back to the title).
func close_options() -> bool:
	if overlay_screen == null:
		return false
	overlay_screen.queue_free()
	game_viewport.remove_child(overlay_screen)
	overlay_screen = null
	_resume_level()
	return true


## Resumes after two physics frames: the key that closed the screen
## (Esc = also "quit the level") is no longer "just pressed".
func _resume_level() -> void:
	var level := _paused_level
	for i in 2:
		await get_tree().physics_frame
	if level != _paused_level or overlay_screen != null:
		return  # level left, or another screen reopened in the meantime
	_paused_level = null
	if is_instance_valid(level) and level.is_inside_tree():
		level.process_mode = Node.PROCESS_MODE_INHERIT
		# Music resumes; a level paused (P key / Start) before
		# F9 / F10 stays paused, music at half volume (level.gd:set_paused).
		Music.set_paused(false)


static func _is_level(scene: Node) -> bool:
	return scene != null and scene.scene_file_path == "res://scenes/level.tscn"


## F2 or "CRT filter" option (F10): next mode, saved in
## display.cfg. The C has no dedicated key (the setting is in Setup >
## Graphics, graphics_menu.c:crt_set); the rewrite keeps F2,
## cycling through the six modes.
func cycle_crt() -> void:
	DisplaySettings.set_crt_mode((crt_mode + 1) % DisplaySettings.CrtMode.size())
	crt_mode = DisplaySettings.crt_mode()


## Boolean compatibility: turns the CRT on (closest mode) or off.
func toggle_crt() -> void:
	set_crt_mode(
		DisplaySettings.CrtMode.OFF if crt_mode != DisplaySettings.CrtMode.OFF else DisplaySettings.CrtMode.PLUS
	)


## Sets the CRT mode (Setup > Graphics) and saves it.
func set_crt_mode(mode: int) -> void:
	DisplaySettings.set_crt_mode(mode)
	crt_mode = DisplaySettings.crt_mode()
	crt = crt_mode != DisplaySettings.CrtMode.OFF


func _unhandled_input(event: InputEvent) -> void:
	# Menu mouse state (keyboard.c): position in game pixels,
	# toggles mouseInactive.
	# Paused level (pause menu, help, Quit box): treated as a
	# menu. Level being played: path unchanged, the ship reads the raw movement.
	var menu_mouse := not _level_running()
	if event is InputEventMouse and menu_mouse:
		event = mouse_event_in_game_pixels(event)
	if TyrianMouse.is_synthetic_motion(event):
		# Jump caused by pointer capture / release: only the known
		# position changes, neither hover nor mouseInactive.
		TyrianMouse.position = Vector2i(event.position.floor())
		return
	TyrianMouse.note(event)
	if event is InputEventMouse:
		_push_mouse(event, menu_mouse)
		return
	if event is InputEventJoypadButton or event is InputEventJoypadMotion:
		var cur0 := current_game_scene()
		menu_repeat.feed(event, repeat_clock.call())
		# Stick held past the threshold: its variations are not new
		# menu presses (joystick.c: edges, then repeat at 300 ms).
		if not _is_level(cur0) and menu_repeat.is_held_motion(event):
			return
		# Screen that uses a global key itself (Destruct: F10 and Shift,
		# destruct.c): it is passed through unchanged.
	if event is InputEventKey:
		var cur := current_game_scene()
		var reserved: Variant = cur.get("reserved_keys") if cur != null else null
		if reserved is Array and event.physical_keycode in reserved:
			game_viewport.push_input(event)
			return
		# keyboard.c: Alt+Enter toggles fullscreen / windowed, the key
		# does not reach the game (neither the repeat nor the release).
	if event is InputEventKey and event.alt_pressed and event.keycode in [KEY_ENTER, KEY_KP_ENTER]:
		if event.pressed and not event.echo:
			toggle_fullscreen()
		get_viewport().set_input_as_handled()
		return
		# Sound mute key (divergence no. 57): everywhere, except while
		# entering a key or a name (the key is data there).
	if (
		event is InputEventKey
		and event.pressed
		and not event.echo
		and not _typing_or_capturing()
		and TyrianControls.global_pressed(event, TyrianControls.KEY_MUTE)
	):
		toggle_mute()
		get_viewport().set_input_as_handled()
		return
		# Outside levels only: F2 CRT filter, F3 graphics rendering, F9 controls,
		# F10 display (the C has nothing on these keys outside levels). In a level they
		# are the C ones (input_map.c:enhanced_keys[]), handled by level_hotkeys.gd.
	if event is InputEventKey and event.pressed and not event.echo and not _is_level(current_game_scene()):
		match event.physical_keycode:
			KEY_F2:
				cycle_crt()
				return
			KEY_F3:
				DisplaySettings.cycle_art_mode()
				return
			KEY_F10:
				open_options("res://scenes/display_options.tscn")
				return
			KEY_F9:
				# Configurable controls (block 14, ADR 0051).
				open_options("res://scenes/controls_options.tscn")
				return
		# Block 14: the gamepad drives the menus (equivalent key); not in
		# a level, where it goes through the game actions (TyrianControls).
	var menu_key := TyrianControls.menu_key_for(event)
	if menu_key != 0:
		var cur := current_game_scene()
		# Screens passed to InputMap actions (ADR 0120): the gamepad event
		# reaches them as is (all bindings, not just the six
		# MENU_KEY keys).
		if cur != null and cur.has_method("handle_action"):
			game_viewport.push_input(event)
			return
		if cur != null and not ("sim" in cur) and cur.get("waiting") != true:
			var kev := InputEventKey.new()
			kev.physical_keycode = menu_key
			kev.keycode = menu_key
			kev.pressed = true
			game_viewport.push_input(kev)
			return
	game_viewport.push_input(event)


## video.c:mapWindowPointToScreen: the window position (Godot's logical base:
## 320x240 in Fit 4:3, 320x180 in Fit 16:9) is mapped back to the game's 320x200
## pixels, the image being stretched over the whole output rectangle. Without
## this, hover lands on the wrong row as soon as the base is not 320x200.
## Center mode (no content scaling): the window position is in window pixels
## and the K×320×200 image sits at `_screen_base` (centered in a window that
## the player may have resized): that offset is removed first, otherwise the
## pointer was mapped as if the image started at the window's top left corner.
func mouse_event_in_game_pixels(event: InputEventMouse) -> InputEventMouse:
	if _screen == null or (Vector2i(_screen.size) == GAME_SIZE and _screen_base == Vector2.ZERO):
		return event
	var factor := Vector2(GAME_SIZE) / _screen.size
	var mapped: InputEventMouse = event.duplicate()
	mapped.position = (event.position - _screen_base) * factor
	mapped.global_position = (event.global_position - _screen_base) * factor
	if mapped is InputEventMouseMotion:
		mapped.relative = event.relative * factor
	return mapped


## Mouse: the window (stretch "canvas_items", integer factor) already yields
## the position in game pixels (320×200); pushed as is (local
## coordinates) so that it stays so in HD mode, where the viewport is 1280×800
## with size_2d_override (pushed non-locally, it would be divided by 4).
## A paused level (pause menu, help, Quit box) too: pushed non-
## locally, in HD (art_mode=2) its positions were divided by 4 and hover
## and click always landed on the first row. The level being played keeps
## the relative movement read by the simulation. System pointer hidden over the game image,
## visible in the black bars (keyboard.c, SDL_ShowCursor).
func _push_mouse(event: InputEventMouse, menu_mouse: bool) -> void:
	if (
		event is InputEventMouseMotion
		and DisplayServer.get_name() != "headless"
		and Input.mouse_mode in [Input.MOUSE_MODE_VISIBLE, Input.MOUSE_MODE_HIDDEN]
	):
		Input.mouse_mode = (
			Input.MOUSE_MODE_HIDDEN if TyrianMouse.inside_screen(event.position) else Input.MOUSE_MODE_VISIBLE
		)
	game_viewport.push_input(event, menu_mouse)


## mainint.c / tyrian2.c: mouseSetRelative(true) during the game (SDL_SetRelativeMouseMode),
## false in menus. Without capture, the pointer bumps against the window edge
## or enters a black bar, and the ship stops following the mouse.
## Captured only while a level is running, played (neither demo nor replay), window active.
func _update_mouse_capture() -> void:
	if DisplayServer.get_name() == "headless":
		return
	var want := false
	if _level_running():
		var cur := current_game_scene()
		want = (
			cur.get("interactive") != false
			and cur.get("demo") != true
			and cur.get("replaying") != true
			and DisplayServer.window_is_focused()
		)
	if want and Input.mouse_mode in [Input.MOUSE_MODE_VISIBLE, Input.MOUSE_MODE_HIDDEN]:
		TyrianMouse.expect_synthetic_motion()
		Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
		# Capture recenters the pointer: this jump is not a player gesture.
		current_game_scene().set("_ignore_next_motion", true)
		# keyboard.c:mouseSetRelative resets the raw counters to zero.
		current_game_scene().set("_mouse_raw", Vector2i.ZERO)
	elif not want and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		TyrianMouse.expect_synthetic_motion()
		Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
		var cur := current_game_scene()
		if cur != null:
			cur.set("_mouse_raw", Vector2i.ZERO)


func _process(delta: float) -> void:
	_poll_window(delta)
	_update_mouse_capture()
	bullet_time_update()
	repeat_tick(repeat_clock.call())


## Gamepad repeats due at instant `now` (ms), sent to the current
## menu screen as an InputMap action; never in a level (the simulation
## reads the held state of the commands, not events), nor during key
## capture (controls screen).
func repeat_tick(now: int) -> void:
	var due := menu_repeat.tick(now)
	if due.is_empty():
		return
	var cur := current_game_scene()
	if cur == null or _is_level(cur) or cur.get("waiting") == true:
		return
	for action in due:
		var ev := InputEventAction.new()
		ev.action = action
		ev.pressed = true
		repeat_pushed += 1
		game_viewport.push_input(ev)


## "HD sprites" mode: rendering of the game viewport at HD_SCALE × its logical
## size (size_2d_override keeps the scenes' 320×200 coordinates).
func _apply_viewport_scale() -> void:
	var k := DisplaySettings.HD_SCALE if DisplaySettings.hd_sprites() else 1
	game_viewport.size = GAME_SIZE * k
	game_viewport.size_2d_override = GAME_SIZE if k > 1 else Vector2i.ZERO
	game_viewport.size_2d_override_stretch = k > 1
	_apply_scale(false)


## Scaler and Scaling Mode (graphics_menu.c; video.c:calc_dst_render_rect).
##
## The scaler enlarges the 320×200 image by a factor K (video_scale.c:scalers[]),
## the Scaling Mode decides how the result is placed in the window:
##   Center  : native size K×320 × K×200 centered, no stretching;
##   Integer : largest integer multiple that fits in the window;
##   Fit 8:5 : stretched to the 8:5 ratio (that of 320×200), centered;
##   Fit 4:3 : stretched into a centered 4:3 rectangle (logical base 320×240);
##   Fit 16:9: same for 16:9 (base 320×180);
##   Fill    : stretched to the whole window, ignoring the ratio.
## The C resizes the window when the scaler changes (video.c:init_scaler,
## window = K × 320×200); we do the same outside fullscreen. In HD mode (AI)
## the viewport is already enlarged ×4: the 4:3/16:9 modes do not distort.
func _apply_scale(resize_window := true) -> void:
	if _screen == null:
		return
	var w := get_window()
	var k: int = DisplaySettings.SCALER_FACTORS[clampi(scaler, 0, DisplaySettings.SCALER_FACTORS.size() - 1)]
	var native := GAME_SIZE * k
	if resize_window and DisplayServer.get_name() != "headless" and w.mode == Window.MODE_WINDOWED:
		w.size = native
		_center_window_on_screen(native)
	if scaling_mode == 0:  # Center: native size centered, no scaling.
		w.content_scale_mode = Window.CONTENT_SCALE_MODE_DISABLED
		_screen.size = Vector2(native)
		_screen_base = (Vector2(w.size) - Vector2(native)) * 0.5
		_apply_shake_position()
		return
	_screen_base = Vector2.ZERO
	var base := GAME_SIZE
	var stretch := Window.CONTENT_SCALE_STRETCH_FRACTIONAL
	var aspect := Window.CONTENT_SCALE_ASPECT_KEEP
	match scaling_mode:
		1:  # Integer
			stretch = Window.CONTENT_SCALE_STRETCH_INTEGER
		2:  # Fit 8:5 (aspect ratio of 320×200)
			pass
		3:  # Fit 4:3
			if not DisplaySettings.hd_sprites():
				base = Vector2i(320, 240)
		4:  # Fit 16:9
			if not DisplaySettings.hd_sprites():
				base = Vector2i(320, 180)
		5:  # Fill
			aspect = Window.CONTENT_SCALE_ASPECT_IGNORE
	w.content_scale_mode = Window.CONTENT_SCALE_MODE_CANVAS_ITEMS
	w.content_scale_aspect = aspect
	w.content_scale_size = base
	if "content_scale_stretch" in w:
		w.content_scale_stretch = stretch
	_screen.size = Vector2(base)
	_apply_shake_position()


## video.c:last_output_rect (w, h): size, in window pixels, of
## the game image actually displayed (scaler and Scaling Mode included). Used to
## convert raw mouse movement (scaleWindowDistanceToScreen).
func output_rect_size() -> Vector2i:
	if _screen == null:
		return GAME_SIZE
	var k := get_window().get_final_transform().get_scale()
	return Vector2i(roundi(_screen.size.x * k.x), roundi(_screen.size.y * k.y))


## video.c:init_scaler: after the resize, window_center_in_display.
## Without it the shrunken window stays stuck to its top left corner.
func _center_window_on_screen(size: Vector2i) -> void:
	var screen := DisplayServer.window_get_current_screen()
	var area := DisplayServer.screen_get_usable_rect(screen)
	get_window().position = area.position + (area.size - size) / 2


## Window size and position before fullscreen, restored on return.
var _windowed_rect := Rect2i()

## Window memory (divergence no. 58): sampled twice a second, written
## when it has not moved from one sample to the next (not on every pixel of a
## drag), and on exit.
const WINDOW_POLL_SECONDS := 0.5
var _window_poll_left := WINDOW_POLL_SECONDS
var _window_seen := Rect2i()
var _window_saved := Rect2i()


## Inactive in headless, for tools (capture, replay) and when the mode is forced
## ("-- --windowed": player-smoke and player-journeys reposition the window themselves).
func _window_memory_active() -> bool:
	return (
		DisplayServer.get_name() != "headless"
		and not SetupScreen._is_tool_run()
		and not OS.get_cmdline_user_args().has("--windowed")
	)


## Default window: project start size, centered on the primary screen.
func default_window_rect() -> Rect2i:
	var size := Vector2i(
		int(ProjectSettings.get_setting("display/window/size/window_width_override")),
		int(ProjectSettings.get_setting("display/window/size/window_height_override"))
	)
	var area := DisplayServer.screen_get_usable_rect(DisplayServer.get_primary_screen())
	return Rect2i(area.position + (area.size - size) / 2, size)


## At launch: puts the window back where the player left it; if that rectangle
## is no longer on any connected screen, default size centered on the primary screen.
func apply_saved_window() -> void:
	if not _window_memory_active():
		return
	var saved: Rect2i = SetupScreen.saved_window_rect()
	if saved.size == Vector2i.ZERO:
		return
	var rect := saved
	if not SetupScreen.window_rect_on_screen(saved, SetupScreen.connected_screens()):
		rect = default_window_rect()
	_windowed_rect = rect
	_window_saved = saved
	if not is_fullscreen():
		_set_window_rect(rect)
		_window_seen = rect


## Window size and position through the Window object: DisplayServer.window_set_size
## changes the system window without updating Window.size, and the image stayed
## laid out for the old size (one enlarged corner, or an offset image).
func _set_window_rect(rect: Rect2i) -> void:
	get_window().size = rect.size
	get_window().position = rect.position


## Window rectangle if it is in windowed mode, zero size otherwise.
func _live_window_rect() -> Rect2i:
	if DisplayServer.window_get_mode() != DisplayServer.WINDOW_MODE_WINDOWED:
		return Rect2i()
	return Rect2i(DisplayServer.window_get_position(), DisplayServer.window_get_size())


func _poll_window(delta: float) -> void:
	_window_poll_left -= delta
	if _window_poll_left > 0.0:
		return
	_window_poll_left = WINDOW_POLL_SECONDS
	var now := _live_window_rect()
	if now.size != Vector2i.ZERO and now == _window_seen and now != _window_saved:
		_save_window(now)
	_window_seen = now


func _save_window(rect: Rect2i) -> void:
	_window_saved = rect
	SetupScreen.remember_window(rect)


## On exit: saves the window as it is (in fullscreen, the previous one).
func save_window_now() -> void:
	if not _window_memory_active():
		return
	var now := _live_window_rect()
	if now.size == Vector2i.ZERO:
		now = _windowed_rect
	if now.size != Vector2i.ZERO and now != _window_saved:
		_save_window(now)


static func is_fullscreen() -> bool:
	var mode := DisplayServer.window_get_mode()
	return mode == DisplayServer.WINDOW_MODE_FULLSCREEN or mode == DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN


## True when the current screen is waiting for a key or text to record.
func _typing_or_capturing() -> bool:
	var cur := current_game_scene()
	if cur == null:
		return false
	return (
		cur.get("waiting") == true
		or cur.get("_capturing") == true
		or cur.get("_typing") == true
		or (cur.get("capture_row") != null and int(cur.get("capture_row")) >= 0)
	)


## Mutes or restores all sound (music and effects). Flag saved separately:
## the volumes and music_disabled / samples_disabled do not change.
func toggle_mute() -> void:
	set_muted(not bool(SetupScreen.settings.sound_muted))


func set_muted(on: bool) -> void:
	SetupScreen.settings.sound_muted = on
	SetupScreen.apply_settings()
	SetupScreen.save_settings()
	var cur := current_game_scene()
	if _is_level(cur) and cur.has_method("hud_queue_message"):
		cur.hud_queue_message(TyrianText.L("msg_sound_off" if on else "msg_sound_on"))


## keyboard.c:toggle_fullscreen (Alt+Enter).
func toggle_fullscreen() -> void:
	set_fullscreen(not is_fullscreen())
	if DisplayServer.get_name() != "headless":
		# Kept like the C's fullscreen_display: 0 windowed, n fullscreen on screen n.
		var display := 0
		if is_fullscreen():
			display = DisplayServer.window_get_current_screen() + 1
		SetupScreen.remember_display(display)


## Exclusive fullscreen: exact screen size, like SDL_WINDOW_FULLSCREEN_DESKTOP
## in the C (Godot's borderless fullscreen measured 1920x1082 on Windows).
## On return to windowed, the previous size and position are restored.
func set_fullscreen(on: bool) -> void:
	if DisplayServer.get_name() == "headless" or on == is_fullscreen():
		return
	if on:
		_windowed_rect = Rect2i(DisplayServer.window_get_position(), DisplayServer.window_get_size())
		if _window_memory_active() and _windowed_rect != _window_saved:
			_save_window(_windowed_rect)
		get_window().mode = Window.MODE_EXCLUSIVE_FULLSCREEN
		return
		# Windows + OpenGL: going straight from exclusive fullscreen to windowed
		# leaves a fake title bar in the client area and the rendering no longer
		# shows up there (measured on the maintainer's machine, also in an empty Godot
		# project; neither a resize nor RedrawWindow clears it). Going through
		# the minimized state within the same frame recreates the surface: no trace left.
	if _needs_minimized_restore():
		leaving_fullscreen = true
		get_window().mode = Window.MODE_MINIMIZED
	get_window().mode = Window.MODE_WINDOWED
	if _windowed_rect.size != Vector2i.ZERO:
		_set_window_rect(_windowed_rect)
	_finish_windowed_restore()


## True when the return to windowed goes through the minimized state: OpenGL rendering on Windows.
static func _needs_minimized_restore() -> bool:
	return OS.get_name() == "Windows" and RenderingServer.get_current_rendering_driver_name().begins_with("opengl3")


## True during the return from fullscreen to windowed through the minimized state: the focus
## loss it causes is not an Alt+Tab (the level does not pause).
var leaving_fullscreen := false


## Lets the window take its final size (2 frames), restores size and
## position if the window manager changed them, then redraws.
func _finish_windowed_restore() -> void:
	await get_tree().process_frame
	await get_tree().process_frame
	# The focus lost by the minimize comes back a few frames later.
	var frames := 0
	while leaving_fullscreen and not DisplayServer.window_is_focused() and frames < 30:
		await get_tree().process_frame
		frames += 1
	await get_tree().process_frame
	leaving_fullscreen = false
	if is_fullscreen():
		return
	if (
		_windowed_rect.size != Vector2i.ZERO
		and Rect2i(DisplayServer.window_get_position(), DisplayServer.window_get_size()) != _windowed_rect
	):
		_set_window_rect(_windowed_rect)
		await get_tree().process_frame
	RenderingServer.force_draw()
	repaints_done += 1


## Number of forced redraws after a return to windowed (used by tests).
var repaints_done := 0


## To call after a DisplaySettings change: clears the atlas caches
## and reloads the current scene with the new textures.
func apply_display_settings() -> void:
	TyrianAssets.clear_sheet_cache()
	_apply_viewport_scale()
	xbrz = DisplaySettings.art_mode() == DisplaySettings.ArtMode.SHARP
	crt_mode = DisplaySettings.crt_mode()
	crt = crt_mode != DisplaySettings.CrtMode.OFF
	scaler = DisplaySettings.scaler()
	scaling_mode = DisplaySettings.scaling_mode()
	effects_changed.emit()


## Shader parameters per CRT mode - copy of shader.c:shader_render
## (line intensity, curvature, halo, aberration, grid, vignette).
## Scanlines reads none: the shader follows the C's SDL path (video.c:550).
const CRT_PARAMS := {
	DisplaySettings.CrtMode.OFF: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
	DisplaySettings.CrtMode.SCANLINES: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
	DisplaySettings.CrtMode.PLUS: [0.8, 0.3, 0.0, 0.0, 0.0, 1.0],
	DisplaySettings.CrtMode.PREMIUM: [0.85, 0.35, 0.35, 0.0015, 0.0, 1.0],
	DisplaySettings.CrtMode.APERTURE: [0.2, 0.0, 0.2, 0.0, 1.0, 1.0],
	DisplaySettings.CrtMode.SOFT: [0.10, 0.0, 0.55, 0.0, 0.0, 1.0],
}


## mainint.c:JE_gammaCheck: the new gamma (DisplaySettings) is applied to the image.
func apply_gamma() -> void:
	_apply()


## bullet_time.c:bullet_time_active (toggle or held key).
func bullet_time_active() -> bool:
	return bullet_time


## bullet_time.c:bullet_time_activate + input_map.c:input_action_bullet_time:
## 4 s of slow motion, refused during slow motion, the cooldown, outside a level, or if
## Setup > Gameplay forbids it (mainint.c: bullet_time_enabled). Returns true if
## slow motion starts.
func bullet_time_activate() -> bool:
	if not bullet_time_allowed or not _level_running():
		return false
	var now: int = bullet_time_clock.call()
	if bullet_time or now < _bullet_time_cooldown_end:
		return false
		# mainint.c: neither in a demo (play_demo) nor in replay playback.
	var cur := current_game_scene()
	if cur.get("demo") == true or cur.get("replaying") == true:
		return false
	_bullet_time_end = now + BULLET_TIME_DURATION_MS
	bullet_time_uses += 1
	bullet_time = true
	return true


## bullet_time.c:bullet_time_reset: leaving a level or mode. The cooldown
## is not reset, like in the C.
func bullet_time_reset() -> void:
	bullet_time = false


## Remaining cooldown seconds (input_map.c: "Cooldown: %us", rounded
## up); 0 if ready.
func bullet_time_cooldown_seconds() -> int:
	var remaining: int = _bullet_time_cooldown_end - int(bullet_time_clock.call())
	return maxi(0, (remaining + 999) / 1000)


## Level being played: neither paused (P key, pause menu, quit box,
## help), nor suspended under an options screen.
func _level_running() -> bool:
	var cur := current_game_scene()
	if not _is_level(cur) or level_paused():
		return false
	return not (
		cur.get("paused") == true
		or cur.get("_pause_menu") != null
		or cur.get("_quit_box") != null
		or cur.get("_help_screen") != null
	)


## bullet_time.c:bullet_time_update, once per frame. The deadlines are
## absolute instants: time spent outside the game (pause, menus, loading,
## or a gap >= 1 s between two frames) is added to both, so the
## budget and the cooldown only run while the level is running.
func bullet_time_update() -> void:
	var now: int = bullet_time_clock.call()
	var gap := now - _bullet_time_last_update
	_bullet_time_last_update = now
	if not _level_running():
		if bullet_time and not _is_level(current_game_scene()):
			bullet_time_reset()
		_bullet_time_end += gap
		_bullet_time_cooldown_end += gap
		return
	if gap >= 1000:
		_bullet_time_end += gap
		_bullet_time_cooldown_end += gap
	if bullet_time and now >= _bullet_time_end:
		bullet_time = false
		_bullet_time_cooldown_end = now + BULLET_TIME_COOLDOWN_MS


## game_speed of the C (nortsong.c): boss slow motion (takes priority, as game_speed is
## overwritten every frame by kill_slowmo_update), otherwise bullet time, otherwise 1.
func tick_speed() -> float:
	if kill_slowmo > 0.0:
		return kill_slowmo
	return BULLET_TIME_SCALE if bullet_time_active() else 1.0


var _tick_credit := 0.0


## tyrian2.c:741 divides the loop delay by game_speed: at speed 0.25
## the level advances one tick out of four. In Godot 4, Engine.time_scale does not
## change the number of physics ticks per second (35 stay 35); the
## level therefore skips the excess ticks. Returns true if this tick is played.
func consume_tick() -> bool:
	var speed := tick_speed()
	if speed >= 1.0:
		_tick_credit = 0.0
		return true
	_tick_credit += speed
	if _tick_credit >= 1.0 - 1e-6:
		_tick_credit -= 1.0
		return true
	return false


func _apply_time_scale() -> void:
	Engine.time_scale = 1.0


func _apply() -> void:
	if _material == null:
		return
	_apply_time_scale()
	var wanted: Shader = POST_SHADER_XBRZ if xbrz or scaler in XBRZ_SCALERS else POST_SHADER
	if _material.shader != wanted:
		_material.shader = wanted
	var active := crt_mode != DisplaySettings.CrtMode.OFF
	var p: Array = CRT_PARAMS[crt_mode]
	_material.set_shader_parameter("crt_enabled", active)
	_material.set_shader_parameter("crt_mode", crt_mode)
	_material.set_shader_parameter("scanline_intensity", p[0])
	_material.set_shader_parameter("curvature", p[1])
	_material.set_shader_parameter("phosphor_glow", p[2])
	_material.set_shader_parameter("chroma_shift", p[3])
	_material.set_shader_parameter("aperture_strength", p[4])
	_material.set_shader_parameter("vignette_strength", p[5])
	_material.set_shader_parameter("xbrz_enabled", xbrz)
	_material.set_shader_parameter("scaler", scaler)
	_material.set_shader_parameter("gamma_gain", 1.0 + DisplaySettings.gamma() / 10.0)
	_material.set_shader_parameter("source_size", Vector2(GAME_SIZE))
	# screen_size: window size in pixels, needed by xBRZ outside fragment()
	# (SCREEN_PIXEL_SIZE unavailable in the shader's helper functions).
	_material.set_shader_parameter("screen_size", Vector2(get_window().size))
	effects_changed.emit()
