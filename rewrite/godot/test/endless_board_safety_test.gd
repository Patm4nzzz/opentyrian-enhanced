# M-05: an unreadable Endless board is not overwritten by a one-line board.
extends GdUnitTestSuite


func after_test() -> void:
	for p in [EndlessSession.BOARD_PATH, EndlessSession.BOARD_PATH + ".corrupt"]:
		DirAccess.remove_absolute(ProjectSettings.globalize_path(p))


func test_corrupted_board_is_kept_aside_before_the_new_entry_is_written() -> void:
	var f := FileAccess.open(EndlessSession.BOARD_PATH, FileAccess.WRITE)
	f.store_string("{not json")
	f.close()
	var board := EndlessSession.insert_score([], {"score": 1234, "name": "AAA"})
	assert_int(board.size()).is_equal(1)
	assert_str(FileAccess.get_file_as_string(EndlessSession.BOARD_PATH + ".corrupt")).is_equal("{not json")
	assert_bool(EndlessSession.load_board().size() == 1).is_true()


func test_valid_board_is_not_copied() -> void:
	EndlessSession.insert_score([], {"score": 10, "name": "AAA"})
	EndlessSession.insert_score(EndlessSession.load_board(), {"score": 20, "name": "BBB"})
	assert_bool(FileAccess.file_exists(EndlessSession.BOARD_PATH + ".corrupt")).is_false()
