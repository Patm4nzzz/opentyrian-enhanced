# GdUnit4: presentation never creates a texture from a null
# or empty image (D3: texture_2d_initialize, crash of the Windows exe in
# --headless).
extends GdUnitTestSuite


func test_null_and_empty_images_are_rejected() -> void:
	assert_bool(TyrianScreen.usable_image(null)).is_false()
	assert_bool(TyrianScreen.usable_image(Image.new())).is_false()


func test_real_image_is_accepted() -> void:
	assert_bool(TyrianScreen.usable_image(Image.create(4, 4, false, Image.FORMAT_RGBA8))).is_true()
