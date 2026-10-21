# B-05: "Mutators active - score x..." notice on the episode-end score page
# (level_load.c, case 'Q': levelWarningText[1] / [2] lines).
extends GdUnitTestSuite

const Story := preload("res://scripts/story_screens.gd")


func after_test() -> void:
	TyrianText.language = "en"


func test_no_mutator_gives_empty_line() -> void:
	assert_str(Story.mutator_notice(0)).is_equal("")


func test_mutator_without_bonus_gives_label_only() -> void:
	assert_str(Story.mutator_notice(0x01)).is_equal(TyrianText.L("mut_score_notice"))


func test_risk_reward_shows_multiplier() -> void:
	var label := TyrianText.L("mut_score_notice")
	assert_str(Story.mutator_notice(0x80)).is_equal(label)
	assert_str(Story.mutator_notice(0x80 | 0x01 | 0x02)).is_equal(label + " x1.50")
