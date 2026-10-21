# The six scaling modes and the two sprite sets have a name of their own in
# each language (the maintainer's decision, 2026-10-12: no more English values in
# the French menus; the C wrote them hard-coded, video.c:40-47).
extends GdUnitTestSuite

const KEYS := [
	"gfx_scale_center",
	"gfx_scale_integer",
	"gfx_scale_8_5",
	"gfx_scale_4_3",
	"gfx_scale_16_9",
	"gfx_scale_fill",
	"gfx_sprite_remastered",
	"gfx_sprite_upscaled"
]


func test_scaling_and_sprite_names_are_translated() -> void:
	var expected := {
		"en": ["Center", "Integer", "Fit 8:5", "Fit 4:3", "Fit 16:9", "Fill", "Remastered", "Upscaled (xBRZ)"],
		"fr":
		["Centré", "Entier", "Ajusté 8:5", "Ajusté 4:3", "Ajusté 16:9", "Remplir", "Remasterisé", "Agrandi (xBRZ)"],
	}
	for lang in expected:
		for i in KEYS.size():
			assert_str(TyrianText.L(KEYS[i], lang)).override_failure_message("%s %s" % [lang, KEYS[i]]).is_equal(
				expected[lang][i]
			)


func test_every_language_keeps_the_xbrz_algorithm_name() -> void:
	for lang in ["en", "fr", "de", "es", "it", "pt"]:
		assert_str(TyrianText.L("gfx_sprite_upscaled", lang)).contains("xBRZ")
