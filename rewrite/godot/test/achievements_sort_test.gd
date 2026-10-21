# T2-1: the achievements list is sorted on the CP437 bytes of the name (achievements.c:1365,
# name already passed through locale.c:utf8_to_cp437), not on UTF-8: "á" counts as "a".
extends GdUnitTestSuite

const SCREEN := preload("res://scripts/achievements_screen.gd")


func test_accented_a_sorts_as_plain_a() -> void:
	# UTF-8: "Cámara" (0xC3...) would come after "Cazador"; CP437: "Camara" before.
	assert_int(SCREEN.natural_strcmp("Cámara lenta", "Cazador de jefes")).is_less(0)
	assert_int(SCREEN.natural_strcmp("Cazador de jefes", "Cámara lenta")).is_greater(0)


func test_numbers_still_compared_by_value() -> void:
	assert_int(SCREEN.natural_strcmp("Level 50", "Level 100")).is_less(0)
