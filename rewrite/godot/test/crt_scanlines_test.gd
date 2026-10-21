## "Scanlines" CRT filter: same effect as the C on brightness.
##
## The C only uses its GL shader if it has an OpenGL context; on Windows
## (Direct3D renderer by default) and on Linux without GL, it takes the SDL path
## of video.c:550-565: image ×(1 + 51/255), then every other line veiled with
## black at 51/255. Measured on the same screen (Setup > Graphics, 1280×684, C under
## WSL with software renderer, without the brightening that Direct3D applies):
## C 0.936 of the unfiltered brightness, veiled lines at 0.79 of the others.
## Headless rendering does not go through the shader: the test checks the formula
## in the source.
extends GdUnitTestSuite

const SHADER := "res://shaders/post_body.gdshaderinc"


func _scanlines_branch() -> String:
	var src := FileAccess.get_file_as_string(SHADER)
	var start := src.find("} else if (crt_mode == 1) {")
	(
		assert_int(start)
		. override_failure_message("no dedicated Scanlines branch (video.c:550) in %s" % SHADER)
		. is_greater_equal(0)
	)
	if start < 0:
		return ""
	return src.substr(start, src.find("} else {", start) - start)


func test_scanlines_follow_c_sdl_path() -> void:
	var branch := _scanlines_branch()
	if branch.is_empty():
		return
	assert_str(branch).contains("1.0 + 51.0 / 255.0")
	assert_str(branch).contains("1.0 - 51.0 / 255.0")
	assert_str(branch).contains("mod(row, 2.0)")
	(
		assert_bool(branch.contains("sin("))
		. override_failure_message("lines from the GL formula instead of the C veil")
		. is_false()
	)
