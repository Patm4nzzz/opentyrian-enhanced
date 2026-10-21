# Insertion and persistence tests for the high scores (highscore.c).
# Verifies: qualifying score inserted at the right rank, "*" mark if mutators
# are active, board read back from the JSON file at the next startup.
extends GdUnitTestSuite

const HS := preload("res://scripts/high_scores.gd")
## Path of the JSON file in the test folder.
const TEST_PATH := "user://test_highscores.json"


func before_test() -> void:
	# Isolate the tests: base board with low scores.
	var tables := []
	for _i in 20:
		(
			tables
			. append(
				[
					{"score": 300, "name": "AAA", "difficulty": 2},
					{"score": 200, "name": "BBB", "difficulty": 2},
					{"score": 100, "name": "CCC", "difficulty": 2},
				]
			)
		)
	HS.t2k_high_scores = tables
	# Delete the existing test file.
	DirAccess.remove_absolute(ProjectSettings.globalize_path(TEST_PATH))
	# Redirect SCORES_PATH to the test file (via static variable).


func after() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(TEST_PATH))


## Same call sequence as story_screens.gd (the real flow): multiplier, rank,
## shift, entry, write. Returns the entered name, or null if the score does not qualify.
func _insert(score: int, table: int, difficulty: int, marked: bool, mult: float, name: String) -> Variant:
	var effective := HS.apply_multiplier(score, mult)
	var slot := HS.qualifying_slot(table, effective)
	if slot < 0:
		return null
	HS.shift_down(table, slot)
	HS.set_entry(table, slot, effective, name, difficulty, marked)
	HS.save_scores()
	return HS.t2k_high_scores[table][slot].name


func test_score_inserted_at_rank_0() -> void:
	# Score > 300: rank 0.
	var name: Variant = _insert(500, 10, 2, false, 1.0, "HERO")
	assert_str(str(name)).is_equal("HERO")
	assert_int(HS.t2k_high_scores[10][0].score).is_equal(500)
	assert_str(HS.t2k_high_scores[10][0].name).is_equal("HERO")
	# The old scores are shifted.
	assert_int(HS.t2k_high_scores[10][1].score).is_equal(300)
	assert_int(HS.t2k_high_scores[10][2].score).is_equal(200)


func test_score_not_qualifying_returns_null() -> void:
	# Score <= 100 (last): not in the top 3.
	var result: Variant = _insert(50, 10, 2, false, 1.0, "NOPE")
	assert_object(result).is_null()
	# Board unchanged.
	assert_int(HS.t2k_high_scores[10][0].score).is_equal(300)


func test_mutator_mark_appended() -> void:
	# Qualifying score with active mutators: " *" mark attached.
	var name: Variant = _insert(500, 10, 2, true, 1.0, "ROGUE")
	# The name in the board must end with " *".
	var stored: String = HS.t2k_high_scores[10][0].name
	assert_bool(stored.ends_with(" *")).override_failure_message("missing marker: '%s'" % stored).is_true()
	assert_bool(HS._is_marked(stored)).is_true()


func test_risk_reward_multiplier_applied() -> void:
	# MUT_RISK_REWARD (0x80): multiplier 1.25 → 500 × 1.25 = 625.
	# The multiplier is computed in game.gd and passed to set_entry.
	var mult := 1.25
	_insert(500, 10, 2, true, mult, "RR")
	assert_int(HS.t2k_high_scores[10][0].score).is_equal(625)


## B-04: Risk/Reward multiplier = 1 + 0.25 × (other mutators), truncation (C).
func test_mutator_multiplier_counts_other_mutators() -> void:
	assert_float(HS.mutator_multiplier(0x00)).is_equal(1.0)
	assert_float(HS.mutator_multiplier(0x07)).override_failure_message("without Risk/Reward: 1").is_equal(1.0)
	assert_float(HS.mutator_multiplier(0x80)).override_failure_message("alone: 1 + 0 = 1").is_equal(1.0)
	assert_float(HS.mutator_multiplier(0x80 | 0x01)).is_equal(1.25)
	assert_float(HS.mutator_multiplier(0x80 | 0x07)).override_failure_message("3 others: 1.75").is_equal(1.75)
	assert_float(HS.mutator_multiplier(0xFF)).is_equal(2.75)


func test_apply_multiplier_truncates() -> void:
	# (int32_t)(1001 * 1.25) = 1251 (1251.25 truncated); roundi would have given 1251 too,
	# 3 × 1.75 = 5.25 -> 5 (roundi: 5), 7 × 1.25 = 8.75 -> 8 (roundi: 9).
	assert_int(HS.apply_multiplier(7, 1.25)).override_failure_message("truncation, not rounding").is_equal(8)
	assert_int(HS.apply_multiplier(500, 1.0)).is_equal(500)
	assert_int(HS.apply_multiplier(2000000000, 2.75)).is_equal(2147483647)


func test_save_and_reload() -> void:
	# Insert a score, save, clear the board, reload.
	_insert(500, 11, 2, false, 1.0, "SAVED")
	# Save directly (save_scores writes to SCORES_PATH).
	HS.save_scores()
	# Clear the board in memory.
	var blank := []
	for _i in 20:
		(
			blank
			. append(
				[
					{"score": 0, "name": "", "difficulty": 0},
					{"score": 0, "name": "", "difficulty": 0},
					{"score": 0, "name": "", "difficulty": 0},
				]
			)
		)
	HS.t2k_high_scores = blank
	# Reload from the file.
	var ok := HS.load_scores()
	assert_bool(ok).override_failure_message("load_scores() false: file missing or invalid").is_true()
	assert_int(HS.t2k_high_scores[11][0].score).is_equal(500)
	assert_str(HS.t2k_high_scores[11][0].name).is_equal("SAVED")
