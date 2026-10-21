## Menu screen colors compared with the C (tag c-final).
##
## Defect reported by the maintainer (2026-10-11): Setup > Mods and Setup > Key
## Bindings ("Controls - player 1") dull, almost black background, blue-gray
## text. These screens put image 2 in darkened true colors
## (modulate 0.4-0.55) and wrote in invented colors, instead of
## drawing it with ITS palette and writing with the C's shades
## (mod_manager.c:1502-1650, menu_tree.c:run_menu).
##
## References measured on the c-final C (exact 320×200 image, Setup > Mods and
## Setup > Key Bindings, fresh profile):
##   - background, strip x 0..8 / y 20..185 (no text): mean RGB (0.4; 3.4; 5.8);
##   - Mods header and chosen mod (JE_textShade 15/4): lightest color
##     (215, 190, 105); unchosen mod (15/2): (174, 109, 44).
## The small-font lines of the rewrite's screens follow these shades.
extends GdUnitTestSuite

const C_BACKGROUND := Color8(0, 3, 6)  # 0.4; 3.4; 5.8 rounded
const C_MODS_HEADER := Color8(215, 190, 105)
const C_SMALL_UNSELECTED := Color8(174, 109, 44)
const SCREENS := [
	"res://scenes/mods.tscn",
	"res://scenes/controls_options.tscn",
	"res://scenes/display_options.tscn",
	"res://scenes/modes_menu.tscn",
]


func before() -> void:
	Effects.close_options()


## Image presented by the screen (8-bit framebuffer converted by its palette),
## null if the screen does not draw into a framebuffer.
func _frame(path: String) -> Image:
	var scene: Node = auto_free(load(path).instantiate())
	add_child(scene)
	await get_tree().process_frame
	var screen: Variant = scene.get("screen")
	(
		assert_object(screen)
		. override_failure_message("%s: no 8-bit framebuffer (true colours darkened, not those of the C)" % path)
		. is_not_null()
	)
	if screen == null:
		return null
	var img: Image = (screen as TyrianScreen).vga.image(TyrianScreen.VGA_SCREEN)
	return img


func _mean(img: Image, rect: Rect2i) -> Color:
	var r := 0.0
	var g := 0.0
	var b := 0.0
	for y in range(rect.position.y, rect.end.y):
		for x in range(rect.position.x, rect.end.x):
			var c := img.get_pixel(x, y)
			r += c.r8
			g += c.g8
			b += c.b8
	var n := float(rect.get_area())
	return Color8(roundi(r / n), roundi(g / n), roundi(b / n))


func _brightest(img: Image, rect: Rect2i) -> Color:
	var best := Color.BLACK
	for y in range(rect.position.y, rect.end.y):
		for x in range(rect.position.x, rect.end.x):
			var c := img.get_pixel(x, y)
			if c.r8 + c.g8 + c.b8 > best.r8 + best.g8 + best.b8:
				best = c
	return best


func _assert_close(got: Color, want: Color, what: String) -> void:
	var d := maxi(maxi(absi(got.r8 - want.r8), absi(got.g8 - want.g8)), absi(got.b8 - want.b8))
	(
		assert_int(d)
		. override_failure_message(
			"%s: %s instead of %s (C)" % [what, [got.r8, got.g8, got.b8], [want.r8, want.g8, want.b8]]
		)
		. is_less_equal(1)
	)


## Background of each settings screen: image 2 in its palette, like the C.
func test_settings_screens_background_matches_c() -> void:
	for path in SCREENS:
		var img: Image = await _frame(path)
		if img == null:
			continue
		_assert_close(_mean(img, Rect2i(0, 20, 9, 166)), C_BACKGROUND, "%s, background" % path)


## Text shades: Mods header, chosen and unchosen rows of the
## Controls (the first row is chosen on opening).
func test_settings_screens_text_matches_c() -> void:
	var mods: Image = await _frame("res://scenes/mods.tscn")
	if mods != null:
		_assert_close(_brightest(mods, Rect2i(100, 5, 120, 8)), C_MODS_HEADER, "Mods, header")
	var controls: Image = await _frame("res://scenes/controls_options.tscn")
	if controls != null:
		var y := ControlsOptions.ROW_Y
		_assert_close(_brightest(controls, Rect2i(20, y, 100, 7)), C_MODS_HEADER, "Controls, selected row")
		_assert_close(
			_brightest(controls, Rect2i(20, y + ControlsOptions.ROW_STEP, 100, 7)),
			C_SMALL_UNSELECTED,
			"Controls, unselected row"
		)


## The whole class: no screen (outside a level) draws in true colors
## (TyrianText nodes, image darkened by modulate). The level keeps its
## fallback RGBA labels, used only without the extension.
func test_no_menu_screen_draws_true_colour() -> void:
	var dir := DirAccess.open("res://scenes")
	for f in dir.get_files():
		if not f.ends_with(".tscn") or f == "level.tscn":
			continue
		var packed: PackedScene = load("res://scenes/" + f)
		var script: Script = null
		for i in packed.get_state().get_node_property_count(0):
			if packed.get_state().get_node_property_name(0, i) == &"script":
				script = packed.get_state().get_node_property_value(0, i)
		if script == null:
			continue
		var src := script.source_code
		(
			assert_bool(src.contains("TyrianText.new()"))
			. override_failure_message("%s: text in true colours (TyrianText.new)" % f)
			. is_false()
		)
		var dimmed := RegEx.create_from_string("modulate\\s*=\\s*Color\\(").search(src)
		assert_object(dimmed).override_failure_message("%s: image darkened by modulate" % f).is_null()
