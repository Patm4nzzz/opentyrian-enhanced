# Mouse cursor in HD art (divergence no. 55, the maintainer's decision 2026-10-10):
# ×4 texture built from the enlarged newsh1.png; Original unchanged.
extends GdUnitTestSuite

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
	Effects.apply_display_settings()


func _palette() -> PackedByteArray:
	var v: RefCounted = ClassDB.instantiate("TyrianVga")
	return v.palette_bytes()


func test_original_pointer_is_unchanged_320x200() -> void:
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.ORIGINAL)
	var cursor := MouseCursor.new()
	var img := cursor._build_image(MouseCursor.NORMAL, 0, _palette())
	assert_int(img.get_width()).is_equal(320)
	assert_int(img.get_height()).is_equal(200)
	cursor.free()


func test_hd_pointer_is_four_times_original_and_built_from_hd_sheet() -> void:
	if not DisplaySettings.hd_available():
		return
	DisplaySettings.set_art_mode(DisplaySettings.ArtMode.HD)
	var cursor := MouseCursor.new()
	var pal := _palette()
	# Empty palette in test: build one, only the size and origin matter.
	if pal.size() < 768:
		pal = PackedByteArray()
		pal.resize(768)
		pal.fill(128)
	for kind in [MouseCursor.NORMAL, MouseCursor.UP, MouseCursor.DOWN, MouseCursor.LEFT, MouseCursor.RIGHT]:
		var low := cursor._build_image(kind, 0, pal)
		var tex: ImageTexture = cursor._build_hd(kind, 0, pal, low)
		assert_int(tex.get_width()).is_equal(24 * DisplaySettings.HD_SCALE)
		assert_int(tex.get_height()).is_equal(28 * DisplaySettings.HD_SCALE)
		# Content coming from the HD sheet: at least one opaque pixel.
		var opaque := 0
		var img := tex.get_image()
		for y in img.get_height():
			for x in img.get_width():
				if img.get_pixel(x, y).a > 0.0:
					opaque += 1
		assert_bool(opaque > 0).is_true()
	var sheet := MouseCursor._hd_sheet()
	assert_int(sheet.scale).is_equal(DisplaySettings.HD_SCALE)
	assert_int(sheet.texture.get_width()).is_equal(508 * DisplaySettings.HD_SCALE)
	cursor.free()
