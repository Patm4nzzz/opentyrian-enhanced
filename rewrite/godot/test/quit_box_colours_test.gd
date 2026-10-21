# Confirmation box on the title (palette 8, red): its pixels are displayed
# in the game's colors, as in the shop. Before the fix, the box's
# indices went through the title palette: red blotch, title
# dark on dark.
extends GdUnitTestSuite

const OPTION_SHAPES := 5
const TITLE_PIC := 4
const GAME_PIC := 3


## Box (sprite 35 and a text) drawn with the game's indices on two
## blank surfaces, like TyrianScreen._draw_modal_box.
func _game_box() -> RefCounted:
	var scratch: RefCounted = ClassDB.instantiate("TyrianVga")
	scratch.load_pic(0, GAME_PIC, true)
	for s in [1, 2]:
		scratch.clear(s, s - 1)
		scratch.blit_sprite(s, 50, 50, OPTION_SHAPES, 35)
		scratch.out_text(s, 70, 90, "Quit?", 12, 1)
	return scratch


func _title() -> RefCounted:
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	assert_bool(vga.load_pic(0, TITLE_PIC, true)).is_true()
	vga.bar_shade(0, 65, 55, 255, 155)
	return vga


func test_box_on_the_title_shows_the_game_colours() -> void:
	var vga := _title()
	var box := _game_box()
	var expected: Image = box.image(1)
	assert_bool(vga.place_true_colour_box(0, box.indices(1), box.indices(2), box.palette_bytes())).is_true()
	var shown: Image = vga.image(0)
	var wrong := 0
	var total := 0
	for y in range(50, 150):
		for x in range(50, 240):
			if box.get_pixel(1, x, y) != box.get_pixel(2, x, y):
				continue
			total += 1
			if shown.get_pixel(x, y) != expected.get_pixel(x, y):
				wrong += 1
	assert_int(total).is_greater(10000)
	assert_int(wrong).is_equal(0)


func test_without_the_fix_the_title_palette_would_recolour_the_box() -> void:
	# Control: the same indices drawn directly on the title are displayed
	# differently from in the game (the old rendering).
	var vga := _title()
	var box := _game_box()
	vga.blit_sprite(0, 50, 50, OPTION_SHAPES, 35)
	var expected: Image = box.image(1)
	var shown: Image = vga.image(0)
	var different := 0
	for y in range(60, 140):
		for x in range(60, 230):
			if box.get_pixel(1, x, y) != 0 and shown.get_pixel(x, y) != expected.get_pixel(x, y):
				different += 1
	assert_int(different).is_greater(5000)


func test_no_gives_the_title_back_unchanged() -> void:
	var vga := _title()
	var before_pixels: PackedByteArray = vga.indices(0)
	var before_palette: PackedByteArray = vga.palette_bytes()
	var box := _game_box()
	vga.place_true_colour_box(0, box.indices(1), box.indices(2), box.palette_bytes())
	assert_bool(vga.indices(0) == before_pixels).is_false()
	vga.close_true_colour_box(0, false)
	assert_bool(vga.indices(0) == before_pixels).is_true()
	assert_bool(vga.palette_bytes() == before_palette).is_true()


func test_box_on_a_game_palette_screen_is_drawn_directly() -> void:
	# Shop, save: the palette already shows the game's colors, nothing to set.
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	vga.load_pic(0, GAME_PIC, true)
	var box := _game_box()
	assert_bool(vga.place_true_colour_box(0, box.indices(1), box.indices(2), box.palette_bytes())).is_false()
