## Palettized 8-bit screen (ADR 0020): displays a `TyrianVga` (GDExtension)
## the way the C version presents `VGAScreen` in `JE_showVGA`.
##
## Interface scenes draw with the same calls as the C code
## (`vga.blit_sprite`, `vga.draw_font_hv`, `vga.fade_*`...) then call
## `present()` at each `JE_showVGA()`. The C delay (`setDelay(n)` +
## `service_wait_delay`, nortsong.c) is written `await wait_units(n)`.
##
## With `--frames=<folder>` (capture tool), each `present()` writes
## the image in the OTFR format of golden_master.c: the image sequence is compared
## one by one with that of the C version (rewrite/scripts/visual_diff.py).
class_name TyrianScreen
extends Node2D

## nortsong.c:delayPeriod = 0x4300 × 12 / 14318180 s (≈ 14.4 ms).
const DELAY_PERIOD := float(0x4300) * 12.0 / 14318180.0

## Surfaces of video.c.
const VGA_SCREEN := 0
const VGA_SCREEN2 := 1
## Area of the menu scenery where the title image leaves text leftovers
## ("© 1994-1999 ... DANC..."): a black band covers them, like the foot of the
## achievements panel.
const BACKDROP_MASK_LEFT := 204
const BACKDROP_MASK_TOP := 191
const GAME_SCREEN := 2

## Fonts (font.h) = tables of tyrian.shp (sprite.h).
const LARGE_FONT := 0
const NORMAL_FONT := 1
const SMALL_FONT := 2
const PLANET_SHAPES := 3
const FACE_SHAPES := 4
const OPTION_SHAPES := 5
const WEAPON_SHAPES := 6
## Alignments (font.h:FontAlignment).
const LEFT := 0
const CENTERED := 1
const RIGHT := 2

## Global counter of presented images (all scenes) for --frames.
static var frame_counter := 0

## Shared interface layer (block 20.3, ADR 0122). Screens draw
## in the 8-bit framebuffer (`vga`, pixel-level render compared with the C code);
## anything added that is foreign to the C code — mod elements, HD sprites, help — goes into
## `overlay`, a node placed above the 320×200 image and never written
## into the framebuffer: captures (`--frames`, OTFR) and hence pixel
## parity do not depend on it.
## Layer providers: Callable(screen: TyrianScreen) called on
## entry of each screen, after its `_ready` (mods, HD pack).
static var overlay_providers: Array[Callable] = []
var overlay: Node2D
## Emitted at each `present()` (JE_showVGA): the layer follows the displayed image.
signal presented(surf: int)


## Adds a layer provider (no duplicates).
static func add_overlay_provider(provider: Callable) -> void:
	if not overlay_providers.has(provider):
		overlay_providers.append(provider)


static var _shared_vga: RefCounted
var vga: RefCounted
var _sprite: Sprite2D
var _texture: ImageTexture
## HD (AI) mode: enlarged interface (HdInterface).
var _hd: HdInterface
## Key received during an interruptible wait (wait_delayorinput).
var _input_seen := false
## Modal box open (quit_request_msg): keys go to it.
var modal_active := false
## Action received by the modal box (null: no key yet).
## Internal hover keys of the Yes / No box (not menu actions).
const HOVER_YES := &"modal_hover_yes"
const HOVER_NO := &"modal_hover_no"
var _modal_key: Variant = null
## Mouse cursor of the C code (mouse.c:JE_mouseStart) in `overlay`: screen that
## draws it (`mouse_cursor`; false for the C screens without JE_mouseStart:
## debriefing, jukebox), pointer (MouseCursor.NORMAL...: mouseCursor) and
## filter of JE_mouseStartFilter (title: 0xF0), 0 without a filter.
var mouse_cursor := true
var mouse_pointer := 0
var mouse_filter_hue := 0


func _init() -> void:
	# A single VGA for all scenes, like the C globals VGAScreen,
	# VGAScreen2 and the palette: a screen inherits the image and the
	# palette left by the previous one (return from Setup to the title without a fade).
	if _shared_vga == null and ClassDB.class_exists("TyrianVga"):
		_shared_vga = ClassDB.instantiate("TyrianVga")
	vga = _shared_vga
	if vga == null:
		push_error("GDExtension tyrian-godot missing: TyrianVga unavailable")
		return
	_sprite = Sprite2D.new()
	_sprite.centered = false
	_sprite.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	add_child(_sprite)
	_hd = HdInterface.new(_sprite, vga)
	overlay = Node2D.new()
	overlay.name = "Overlay"
	overlay.z_index = 1
	add_child(overlay)
	overlay.add_child(MouseCursor.new(self))


## Data loaded on scene entry: at `_init`, the main scene is
## instantiated before the `_ready` of the Game autoload which sets `data_dir`.
## (_notification is called at each inheritance level, without `super()`.)
func _notification(what: int) -> void:
	if what == NOTIFICATION_READY and vga != null:
		ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
		for provider in overlay_providers:
			if provider.is_valid():
				provider.call(self)


## True if `img` can feed an ImageTexture (neither null nor empty).
static func usable_image(img: Image) -> bool:
	return img != null and not img.is_empty() and img.get_width() > 0 and img.get_height() > 0


## `JE_showVGA()`: converts the surface through the hardware palette.
func present(surf := VGA_SCREEN) -> void:
	if vga == null or not is_inside_tree():
		return
	var img: Image = vga.image(surf)
	if not usable_image(img):
		# D3: a null or empty image would give an uninitialized texture
		# (Dummy renderer: "Parameter "t" is null", crash of the Windows exe).
		return
	if _texture == null:
		_texture = ImageTexture.create_from_image(img)
		_sprite.texture = _texture
	else:
		_texture.update(img)
	_hd.update(surf)
	presented.emit(surf)
	var dir: String = Capture.frames_dir() if Capture != null else ""
	if not dir.is_empty():
		# --frames-dedup: like GOLDEN_FRAMES_DEDUP=1 on the C side, an image
		# identical to the previous one (indices + palette) is not written.
		if Capture.has_option("frames-dedup"):
			var h := _frame_hash(surf)
			if h == _last_frame_hash:
				return
			_last_frame_hash = h
		vga.save_otfr(surf, dir.path_join("%06d.otfr" % frame_counter), -1)
	frame_counter += 1


static var _last_frame_hash := ""


func _frame_hash(surf: int) -> String:
	var ctx := HashingContext.new()
	ctx.start(HashingContext.HASH_SHA1)
	ctx.update(vga.indices(surf))
	ctx.update(vga.palette_bytes())
	return ctx.finish().hex_encode()


## `setDelay(n); service_wait_delay();` — in frame-by-frame capture, no real
## wait (only the order of the images matters).
##
## Cumulative deadline, like the C code which waits for `target` (nortsong.c:57-84): each
## call pushes the deadline back by n × DELAY_PERIOD and only waits if it has not
## yet passed. A timer per call would cost at least one
## whole frame, two at 70 fps (14.3 ms < 14.4 ms): the ENGAGE effect (300 steps)
## would last 9 s instead of 4.3 s and each palette fade twice the C's, with a
## duration that would depend on the screen refresh rate. A too old deadline
## (screen left without waiting, hiccup) restarts from now.
var _delay_next_ms := -1.0


func wait_units(units: int) -> void:
	if not is_inside_tree():
		return
	if Capture.active:
		await get_tree().process_frame
		return
	var now := Time.get_ticks_usec() / 1000.0
	if _delay_next_ms < now - 4.0 * DELAY_PERIOD * 1000.0:
		_delay_next_ms = now
	_delay_next_ms += units * DELAY_PERIOD * 1000.0
	# wait_delay: nothing to wait for if the deadline has passed (the next image
	# replaces this one, like the C code when late); otherwise only the remaining time.
	var left := (_delay_next_ms - now) / 1000.0
	if left > 0.0:
		await get_tree().create_timer(left, true, false, true).timeout


## `setDelay(n); wait_delayorinput();`: returns true if a key interrupted it.
func wait_units_or_input(units: int) -> bool:
	_input_seen = false
	if not is_inside_tree():
		return false
	if Capture.active:
		await get_tree().process_frame
		return false
	var now := Time.get_ticks_usec() / 1000.0
	if _delay_next_ms < now - 4.0 * DELAY_PERIOD * 1000.0:
		_delay_next_ms = now
	_delay_next_ms += units * DELAY_PERIOD * 1000.0
	var tree := get_tree()
	while Time.get_ticks_usec() / 1000.0 < _delay_next_ms and not _input_seen:
		await tree.process_frame
	return _input_seen


## Phases in progress that hide the pointer of a pointer screen. The C code only
## shows the pointer in the image presented between JE_mouseStart and
## JE_mouseReplace (mouse.c:117-170); an image presented by JE_showVGA alone
## has none, including each step of a fade (palette.c:141-176), except those that the
## shop on return from help (game_menu.c:1333-1342, 1386-1402) and the name entry
## (highscore.c:475-482) make between the two calls.
var _pointer_hidden := 0


## Start of a phase presented without JE_mouseStart; `pop_pointer_hidden` ends it.
func push_pointer_hidden() -> void:
	_pointer_hidden += 1


func pop_pointer_hidden() -> void:
	_pointer_hidden -= 1


## True when MouseCursor must draw this screen's pointer (active mouse
## aside): pointer screen, outside a pointer-less phase.
func pointer_shown() -> bool:
	return mouse_cursor and _pointer_hidden == 0


## `fade_palette(colors, steps, first, last)`: one `present()` per step.
func fade_palette(steps: int, first := 0, last := 255, surf := VGA_SCREEN) -> void:
	push_pointer_hidden()
	await fade_palette_over_pointer(steps, first, last, surf)
	pop_pointer_hidden()


## Fade between JE_mouseStart and JE_mouseReplace: the pointer stays displayed.
func fade_palette_over_pointer(steps: int, first := 0, last := 255, surf := VGA_SCREEN) -> void:
	vga.fade_palette_init(first, last)
	for s in range(steps, 0, -1):
		vga.fade_step(s, first, last)
		present(surf)
		await wait_units(1)


## `fade_solid(color, steps, first, last)`.
func fade_solid(color: Color, steps: int, first := 0, last := 255, surf := VGA_SCREEN) -> void:
	push_pointer_hidden()
	vga.fade_solid_init(color, first, last)
	for s in range(steps, 0, -1):
		vga.fade_step(s, first, last)
		present(surf)
		await wait_units(1)
	pop_pointer_hidden()


func fade_black(steps: int, surf := VGA_SCREEN) -> void:
	await fade_solid(Color.BLACK, steps, 0, 255, surf)


func fade_white(steps: int, surf := VGA_SCREEN) -> void:
	await fade_solid(Color.WHITE, steps, 0, 255, surf)


func _input(event: InputEvent) -> void:
	# newkey || newmouse (wheel excluded: SDL2 does not make it a button).
	if (
		(event is InputEventKey or event is InputEventJoypadButton) and event.is_pressed()
		or TyrianMouse.is_button_press(event)
	):
		_input_seen = true


## Shared menu inputs (block 20.2, ADR 0120): any screen that defines
## `handle_action(action: StringName)` receives the InputMap actions of
## TyrianControls (keyboard and gamepad). A key without an action arrives as
## &"": several C loops re-present the image on any key.
## The mouse goes to `handle_mouse`.
func _unhandled_input(event: InputEvent) -> void:
	TyrianMouse.note(event)
	if event is InputEventMouse:
		handle_mouse(event)
		return
	if has_method("handle_action"):
		handle_event(event)


## Screen mouse (game coordinates, 320×200): open modal box
## (JE_quitRequestMsg), otherwise the screen's menu engine (`menus`) when
## the screen is waiting for input. Screens with their own mouse redefine it.
func handle_mouse(event: InputEvent) -> void:
	if modal_active:
		modal_mouse(event)
		return
	if get("_busy") == true or get("idle") == false:
		return
	var engine = get("menus")
	if engine is MenuEngine:
		engine.handle_mouse(event)


## game_menu.c:JE_quitRequestMsg: "while (!newkey && !mousedown)" then,
## button pressed, Yes if 56 < x < 142, No if 151 < x < 237 (123 < y < 149);
## elsewhere, the box stays open. Any button counts (mousedown).
func modal_mouse(event: InputEvent) -> void:
	# Hover: the pointer on "Yes" or "No" moves the selection (Enhanced
	# addition: the C code only reacts to the click); active mouse only.
	if event is InputEventMouseMotion and not TyrianMouse.inactive and event.relative != Vector2.ZERO:
		var hovered: Variant = quit_box_answer(event.position)
		if hovered != null:
			_modal_key = HOVER_YES if hovered else HOVER_NO
		return
	if not TyrianMouse.is_button_press(event):
		return
	var answer: Variant = quit_box_answer(event.position)
	if answer != null:
		_modal_key = TyrianControls.YES if answer else TyrianControls.NO


## Answer of the Yes / No box at point `p`: true, false, or null outside
## the two areas (also for the level exit box, level.gd).
static func quit_box_answer(p: Vector2) -> Variant:
	if p.y > 123 and p.y < 149:
		if p.x > 56 and p.x < 142:
			return true
		if p.x > 151 and p.x < 237:
			return false
	return null


## Event -> handle_action (also called by the capture tool --keys).
func handle_event(event: InputEvent) -> void:
	var action := TyrianControls.menu_action(event)
	if action == &"" and not (event is InputEventKey and event.pressed and not event.echo):
		return
	call("handle_action", action)


## nortvars.c:JE_barDrawShadow (bars of the Sound menu, of the shop; shared
## interface layer, ADR 0122).
func bar_draw_shadow(x: int, y: int, res: int, col: int, amt: int, xsize: int, ysize: int) -> void:
	xsize -= 1
	ysize -= 1
	for z in range(1, amt / res + 1):
		if x + xsize + 2 >= 320:  # surface->pitch: stop at the right edge
			break
		vga.bar_shade(VGA_SCREEN, x + 2, y + 2, x + xsize + 2, y + ysize + 2)
		vga.fill_rectangle_xy(VGA_SCREEN, x, y, x + xsize, y + ysize, col + 12)
		vga.fill_rectangle_xy(VGA_SCREEN, x, y, x + xsize, y, col + 13)
		vga.set_pixel(VGA_SCREEN, x, y, col + 15)
		vga.fill_rectangle_xy(VGA_SCREEN, x, y + ysize, x + xsize, y + ysize, col + 11)
		x += xsize + 2
	amt %= res
	if amt > 0:
		vga.bar_shade(VGA_SCREEN, x + 2, y + 2, x + xsize + 2, y + ysize + 2)
		vga.fill_rectangle_xy(VGA_SCREEN, x, y, x + xsize, y + ysize, col + (12 / res * amt))


## Background of the rewrite's settings screens (Mods, Controls, Display,
## Side modes): a TyrianScreen child of `parent`, image 2 of tyrian.pic
## in VGAScreen2 WITH its palette, like the C code for Setup and Mods
## (setup_menu.c:restore_setup_background, mod_manager.c:1502). The texts
## are then drawn in the framebuffer with the C calls
## (TyrianText.text_shade...): `begin_menu_frame` then `present()`.
## The palette is set here, not inherited: opened by F9 / F10 after a fade
## to black, the screen would stay black.
## The background reads no input and draws no cursor: the host screen
## keeps its own.
static func menu_backdrop(parent: Node) -> TyrianScreen:
	var s := TyrianScreen.new()
	s.mouse_cursor = false
	parent.add_child(s)
	# After add_child: entering the tree re-enables _input / _unhandled_input.
	s.set_process_input(false)
	s.set_process_unhandled_input(false)
	if s.vga != null:
		ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
		s.vga.load_pic(VGA_SCREEN2, 2, true)
		s.vga.fill_rectangle_xy(VGA_SCREEN2, BACKDROP_MASK_LEFT, BACKDROP_MASK_TOP, 319, 199, 0)
	return s


## Top of a settings screen frame: VGAScreen = VGAScreen2 (like
## mod_manager.c:1507, "Restore background from VGAScreen2 each frame").
func begin_menu_frame() -> void:
	if vga != null:
		vga.copy(VGA_SCREEN, VGA_SCREEN2)


## Routing of an action to the open modal box; false if there is none.
func modal_input(action: StringName) -> bool:
	if not modal_active:
		return false
	_modal_key = action
	return true


## game_menu.c:JE_quitRequestMsg: Yes/No box on the current screen
## (darkened by JE_barShade), "No" preselected.
## Returns true if "Yes" was chosen.
##
## `default_yes`: Yes preselected (JE_saveRequest, save); `save_info`
## (dictionary `name`, `original`): text of the JE_saveRequest box
## instead of the body of JE_quitRequestMsg.
func quit_request_msg(header: String, body: String, default_yes := false, save_info: Variant = null) -> bool:
	var quit_selected := default_yes
	# Save: the caller has already darkened the whole screen (no offset band that shows the list).
	if save_info == null:
		vga.bar_shade(VGA_SCREEN, 65, 55, 255, 155)
	var scratch := _game_palette_vga()
	modal_active = true
	# game_menu.c:2583-2587, 2694-2698: the box draws its pointer through
	# JE_mouseStart, without the title filter (title_screen.c:572).
	var hue_before := mouse_filter_hue
	mouse_filter_hue = 0
	var done := false
	while not done:
		_modal_key = null
		while _modal_key == null:
			_draw_modal_box(scratch, header, body, quit_selected, save_info)
			present()
			await wait_units(4)
		match _modal_key:
			TyrianControls.YES:
				quit_selected = true
				done = true
			TyrianControls.NO:
				quit_selected = false
				done = true
			TyrianControls.LEFT:
				if not quit_selected:
					Sfx.play(Sfx.CURSOR)
				quit_selected = true
			TyrianControls.RIGHT:
				if quit_selected:
					Sfx.play(Sfx.CURSOR)
				quit_selected = false
			TyrianControls.TOGGLE:
				quit_selected = not quit_selected
				Sfx.play(Sfx.CURSOR)
			HOVER_YES:
				if not quit_selected:
					Sfx.play(Sfx.CURSOR)
				quit_selected = true
			HOVER_NO:
				if quit_selected:
					Sfx.play(Sfx.CURSOR)
				quit_selected = false
			TyrianControls.ACCEPT, TyrianControls.SELECT:
				done = true
			TyrianControls.BACK:
				quit_selected = false
				done = true
	modal_active = false
	mouse_filter_hue = hue_before
	# "No" returns the screen from before the box; "Yes" keeps it for the fade that follows.
	vga.close_true_colour_box(VGA_SCREEN, quit_selected)
	Sfx.play((Sfx.SELECT if save_info != null else Sfx.SPRING) if quit_selected else Sfx.CLICK)
	return quit_selected


## HUD image: its palette is the game's, in which the
## JE_quitRequestMsg box is drawn (shop, level).
const GAME_PALETTE_PIC := 3


## Private VGA with the game palette where to draw the box before placing it on
## the current screen; null without the extension.
func _game_palette_vga() -> RefCounted:
	if not ClassDB.class_exists("TyrianVga"):
		return null
	var scratch: RefCounted = ClassDB.instantiate("TyrianVga")
	scratch.load_pic(0, GAME_PALETTE_PIC, true)
	return scratch


## One frame of the box. On a screen whose palette is not the game's
## (title: palette 8), the box pixels receive the real game colors
## (TyrianVga.place_true_colour_box) instead of those of the screen; where the
## palette is already right (shop, save) it is drawn directly,
## like JE_quitRequestMsg.
func _draw_modal_box(
	scratch: RefCounted, header: String, body: String, quit_selected: bool, save_info: Variant
) -> void:
	if scratch != null:
		var screen_vga := vga
		vga = scratch
		for s in [1, 2]:
			vga.clear(s, s - 1)
			draw_quit_box(header, body, quit_selected, s, save_info)
		vga = screen_vga
		if vga.place_true_colour_box(VGA_SCREEN, scratch.indices(1), scratch.indices(2), scratch.palette_bytes()):
			return
	draw_quit_box(header, body, quit_selected, VGA_SCREEN, save_info)


## One frame of the JE_quitRequestMsg box (frame, title, text, Yes /
## No) on surface `surf`; also used by the level exit
## confirmation (level.gd), drawn outside the shared framebuffer.
func draw_quit_box(
	header: String, body: String, quit_selected: bool, surf := VGA_SCREEN, save_info: Variant = null
) -> void:
	const QUIT_BOX_SELECTED := -2
	const QUIT_BOX_UNSELECTED := -6
	var yes := TyrianText.L("misc_yes")  # miscText[9]
	var no := TyrianText.L("misc_no")  # miscText[10]
	vga.blit_sprite(surf, 50, 50, OPTION_SHAPES, 35)  # message box
	if save_info is Dictionary:
		# game_menu.c:JE_saveRequest: title, "Save name:", "Original save:".
		TyrianText.text_shade(self, 70, 66, header, 0, 5, TyrianText.FULL_SHADE, surf)
		TyrianText.text_shade(self, 74, 90, TyrianText.L("miscText_1"), 12, 1, TyrianText.FULL_SHADE, surf)
		TyrianText.text_shade(self, 135, 90, str(save_info.name), 12, 1, TyrianText.FULL_SHADE, surf)
		TyrianText.text_shade(self, 74, 100, TyrianText.L("miscText_69"), 12, 1, TyrianText.FULL_SHADE, surf)
		TyrianText.text_shade(self, 140, 100, str(save_info.original), 12, 1, TyrianText.FULL_SHADE, surf)
	else:
		TyrianText.text_shade(self, 70, 60, header, 0, 5, TyrianText.FULL_SHADE, surf)
		TyrianText.help_box(self, 70, 90, body, 30, 12, 1, TyrianText.FULL_SHADE, surf)
	var x := 54 + 45 - TyrianText.width(yes, LARGE_FONT) / 2
	TyrianText.out_text_adjust(
		self, x, 128, yes, 15, QUIT_BOX_SELECTED if quit_selected else QUIT_BOX_UNSELECTED, LARGE_FONT, true, surf
	)
	x = 149 + 45 - TyrianText.width(no, LARGE_FONT) / 2
	TyrianText.out_text_adjust(
		self, x, 128, no, 15, QUIT_BOX_UNSELECTED if quit_selected else QUIT_BOX_SELECTED, LARGE_FONT, true, surf
	)


## Object tables of the episode (items.json), for the ship illustration.
var _items_json: Dictionary

const WEAPON_SPRITES := [
	-1,
	0,
	1,
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
	21,
	5,
	13,
	-1,
	14,
	15,
	0,
	14,
	9,
	8,
	2,
	15,
	0,
	13,
	0,
	8,
	8,
	11,
	1,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	2,
	1,
	0,
	0,
	1,
	1,
	1
]
const FRONT_XY_LIST := [
	-1,
	4,
	9,
	3,
	8,
	2,
	5,
	10,
	1,
	-1,
	-1,
	-1,
	-1,
	7,
	8,
	-1,
	-1,
	0,
	-1,
	4,
	0,
	-1,
	-1,
	3,
	-1,
	4,
	-1,
	4,
	-1,
	-1,
	-1,
	9,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	3,
	9,
	4,
	4,
	9,
	9,
	9
]
const REAR_XY_LIST := [
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	0,
	1,
	2,
	3,
	-1,
	4,
	5,
	-1,
	-1,
	6,
	-1,
	-1,
	1,
	0,
	-1,
	6,
	-1,
	5,
	-1,
	0,
	0,
	3,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	0,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1
]


## shop_preview.c:draw_ship_illustration (ship, generator, cannons,
## sidekicks, shield halo).
func _draw_ship_illustration(lo: PackedInt32Array, ship: Dictionary, shield: Dictionary) -> void:
	var sprite_id := int(ship.get("bigshipgraphic", 32)) - 1
	var ship_x := [31, 0, 0, 0, 35, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 36, 30]
	var ship_y := [36, 0, 0, 0, 33, 35, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 30]
	var idx := sprite_id - 27
	var in_range := idx >= 0 and idx < ship_x.size()
	vga.blit_sprite(
		VGA_SCREEN, ship_x[idx] if in_range else 35, ship_y[idx] if in_range else 33, OPTION_SHAPES, sprite_id
	)
	var gen := lo[6]
	var gid := gen + 15 if gen == 1 else gen + 14
	var gx := [62, 64, 67, 66, 63]
	var gy := [84, 85, 86, 84, 97]
	if gid - 16 >= 0 and gid - 16 < 5:
		vga.blit_sprite(VGA_SCREEN, gx[gid - 16], gy[gid - 16], WEAPON_SHAPES, gid)
	var fw := lo[1]
	if fw > 0 and fw < FRONT_XY_LIST.size() and FRONT_XY_LIST[fw] >= 0:
		var fx := [59, 66, 66, 54, 61, 51, 58, 51, 61, 52, 53, 58]
		var fy := [38, 53, 41, 36, 48, 35, 41, 35, 53, 41, 39, 31]
		vga.blit_sprite(VGA_SCREEN, fx[FRONT_XY_LIST[fw]], fy[FRONT_XY_LIST[fw]], WEAPON_SHAPES, WEAPON_SPRITES[fw])
	var rw := lo[3]
	if rw > 0 and rw < REAR_XY_LIST.size() and REAR_XY_LIST[rw] >= 0:
		var rx := [41, 27, 49, 43, 51, 39, 41]
		var ry := [92, 92, 113, 102, 97, 96, 76]
		vga.blit_sprite(VGA_SCREEN, rx[REAR_XY_LIST[rw]], ry[REAR_XY_LIST[rw]], WEAPON_SHAPES, WEAPON_SPRITES[rw])
	_draw_shop_item(6, lo[7], 3, 84)
	_draw_shop_item(7, lo[8], 129, 84)
	vga.blit_sprite_hv(VGA_SCREEN, 28, 23, OPTION_SHAPES, 26, 15, int(shield.get("mpwr", 0)) - 10)


## game_menu.c:JE_drawItem (type 1 ship, 2-7 items; without the mods'
## override tables).
func _draw_shop_item(item_type: int, id: int, x: int, y: int, surf := VGA_SCREEN) -> void:
	if id <= 0:
		return
	if item_type == 1:
		var ships: Array = _items_json["ships"]
		if id >= ships.size():
			return
		var g := int(ships[id].get("shipgraphic", 0))
		if g > 500:
			vga.blit_sprite2(surf, x, y, "main:12", g - 500, 0, 0, true)
		else:
			vga.blit_sprite2(surf, x, y, "main:8", g, 0, 0, true)
		return
	var table: String = (
		{2: "weapon_ports", 3: "weapon_ports", 4: "shields", 5: "generators", 6: "sidekicks", 7: "sidekicks"}
		. get(item_type, "")
	)
	var rows: Array = _items_json.get(table, [])
	if id >= rows.size():
		return
	var g := int(rows[id].get("itemgraphic", 0))
	if g > 0:
		vga.blit_sprite2(surf, x, y, "newsh1.shp", g, 0, 0, true)
