# F1 help: "■" (U+25A0) of the help_* texts is written as "-" by the C
# (locale.c:366); the glyph must be the hyphen's, width included.
extends GdUnitTestSuite


func test_help_separator_square_draws_as_hyphen() -> void:
	for font in [0, 1, 2]:
		assert_array(TyrianText.glyphs("\u25a0", font)).is_equal(TyrianText.glyphs("-", font))
