# HD (AI) mode: a drawing displayed in a palette for which its enlarged image
# does not exist goes through xBRZ, like the C enlarges the 8-bit image. The Quit
# box of the title (game_menu.c:JE_quitRequestMsg, sprite 35 of the options
# table, title palette 8) only has an atlas in the game palette; its
# brown relief, recolored to red, came out embossed and noisy.
extends GdUnitTestSuite

const OPTION_SHAPES := 5
const TITLE_PIC := 4
const BOX := Rect2i(50, 50, 189, 101)

var _saved: PackedByteArray
var _had_file := false


func before_test() -> void:
	_had_file = FileAccess.file_exists(DisplaySettings.PATH)
	if _had_file:
		_saved = FileAccess.get_file_as_bytes(DisplaySettings.PATH)
	DisplaySettings._loaded = false


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(DisplaySettings.PATH))
	if _had_file:
		var f := FileAccess.open(DisplaySettings.PATH, FileAccess.WRITE)
		f.store_buffer(_saved)
		f.close()
	DisplaySettings._loaded = false


## HD image (1280×800) of the Quit box on the title, and the 8-bit image.
func _quit_box_on_title() -> Array:
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	HdInterface.prepare(vga)
	vga.clear(1, 0)
	vga.blit_sprite(1, 50, 50, OPTION_SHAPES, 35)
	assert_bool(vga.load_pic(0, TITLE_PIC, true)).is_true()
	vga.bar_shade(0, 65, 55, 255, 155)
	vga.blit_sprite(0, 50, 50, OPTION_SHAPES, 35)
	return [vga.hd_image(0, Rect2i(0, 0, 320, 200), Rect2i()), vga]


func test_quit_box_on_the_title_is_not_drawn_from_a_foreign_atlas() -> void:
	if not DisplaySettings.hd_available():
		return
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	var r := _quit_box_on_title()
	var hd: Image = r[0]
	var vga: RefCounted = r[1]
	assert_object(hd).is_not_null()
	# Opaque pixels of the box (edge excluded): no HD pixel composed from
	# the brown atlas; they stay transparent and the shader enlarges them
	# from the 8-bit image, like the C.
	var composed := 0
	var total := 0
	for y in range(BOX.position.y + 4, BOX.end.y - 4):
		for x in range(BOX.position.x + 4, BOX.end.x - 4):
			if vga.get_pixel(1, x, y) == 0:
				continue
			total += 1
			if hd.get_pixel(x * 4 + 1, y * 4 + 1).a > 0.0:
				composed += 1
	assert_int(total).is_greater(10000)
	assert_int(composed).is_equal(0)


func test_title_picture_keeps_its_upscaled_image() -> void:
	if not DisplaySettings.hd_available():
		return
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	var hd: Image = _quit_box_on_title()[0]
	# Outside the box and the shadow: the title's enlarged image (pic 4) remains.
	var composed := 0
	for y in range(0, 50):
		for x in range(0, 320):
			if hd.get_pixel(x * 4 + 1, y * 4 + 1).a > 0.0:
				composed += 1
	assert_int(composed).is_greater(320 * 50 * 9 / 10)


## Average color of each composed 4×4 block of the HD image compared with the
## 8-bit color of the same pixel (mean |ΔRGB| over 0..255), pixels of `r` whose
## `mask` (surface 1, origin at r.position) is non-zero, or all if mask = null.
func _hd_colour_error(hd: Image, vga: RefCounted, r: Rect2i, masked: bool) -> float:
	var lo: Image = vga.image(0)
	var err := 0.0
	var n := 0
	for y in range(r.position.y, r.end.y):
		for x in range(r.position.x, r.end.x):
			if masked and vga.get_pixel(1, x - r.position.x, y - r.position.y) == 0:
				continue
			if hd.get_pixel(x * 4 + 1, y * 4 + 1).a == 0.0:
				continue
			var m := Color(0, 0, 0)
			for dy in 4:
				for dx in 4:
					m += hd.get_pixel(x * 4 + dx, y * 4 + dy)
			m /= 16.0
			var c := lo.get_pixel(x, y)
			err += (absf(m.r - c.r) + absf(m.g - c.g) + absf(m.b - c.b)) / 3.0 * 255.0
			n += 1
	return err / maxf(n, 1)


## Matrix of boxes placed on a screen in HD mode: each drawing of the
## options table (Quit box, frames, buttons), placed after the shadow of
## JE_quitRequestMsg on each background image that receives a box in game
## (1 shop and save, 2 settings menus, 4 title, 13 episode end),
## keeps the 8-bit image's color to within 4×4. Measurement: a correct atlas stays
## under 17; before the fix, four drawings placed on the title (2, 5, 6 and
## 38) exceeded 20, up to 27.7.
func test_every_option_drawing_over_every_boxed_screen_keeps_the_8bit_colours() -> void:
	if not DisplaySettings.hd_available():
		return
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	var worst := []
	for pic in [1, 2, 4, 13]:
		var shaded: RefCounted = ClassDB.instantiate("TyrianVga")
		HdInterface.prepare(shaded)
		assert_bool(shaded.load_pic(0, pic, true)).is_true()
		shaded.bar_shade(0, 65, 55, 255, 155)
		var e := _hd_colour_error(
			shaded.hd_image(0, Rect2i(0, 0, 320, 200), Rect2i()), shaded, Rect2i(65, 55, 191, 101), false
		)
		if e > 10.0:
			worst.append("shadow on peak %d: %.1f" % [pic, e])
		for spr in 47:
			var vga: RefCounted = ClassDB.instantiate("TyrianVga")
			HdInterface.prepare(vga)
			vga.load_pic(0, pic, true)
			vga.clear(1, 0)
			vga.blit_sprite(1, 0, 0, OPTION_SHAPES, spr)
			vga.bar_shade(0, 65, 55, 255, 155)
			vga.blit_sprite(0, 50, 50, OPTION_SHAPES, spr)
			var hd: Image = vga.hd_image(0, Rect2i(0, 0, 320, 200), Rect2i())
			e = _hd_colour_error(hd, vga, Rect2i(50, 50, 230, 150), true)
			if e > 20.0:
				worst.append("sprite %d on peak %d: %.1f" % [spr, pic, e])
	assert_array(worst).is_empty()
