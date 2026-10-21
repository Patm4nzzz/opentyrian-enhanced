# DPS counter (F6): literal "DPS:%u" label of the C (enemy_render.c), whatever
# the language.
extends GdUnitTestSuite

const Overlays := preload("res://scripts/level_overlays.gd")


func after_test() -> void:
	TyrianText.language = "en"


func test_dps_label_is_literal_in_every_language() -> void:
	for lang in ["en", "fr", "de", "es", "pt"]:
		TyrianText.language = lang
		assert_str(Overlays.dps_text(42)).override_failure_message("language " + lang).is_equal("DPS:42")
