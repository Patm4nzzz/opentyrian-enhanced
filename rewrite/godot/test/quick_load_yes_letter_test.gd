# Quick load (F9): the prompt announces the language's "yes" letter
# ("O / N" in French, key_yes_letter), and that letter confirms, like
# mainint.c:locale_yes_scancode.
extends GdUnitTestSuite

var _lang := ""


func before_test() -> void:
	_lang = TyrianText.language


func after_test() -> void:
	TyrianText.language = _lang


func _key(code: Key) -> InputEventKey:
	var ev := InputEventKey.new()
	ev.pressed = true
	ev.keycode = code
	ev.physical_keycode = code
	return ev


func test_french_o_confirms() -> void:
	TyrianText.language = "fr"
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_O))).is_true()
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_N))).is_false()


func test_german_j_and_spanish_s_confirm() -> void:
	TyrianText.language = "de"
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_J))).is_true()
	TyrianText.language = "es"
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_S))).is_true()
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_O))).is_false()


func test_english_y_is_the_letter() -> void:
	TyrianText.language = "en"
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_Y))).is_true()
	assert_bool(LevelHotkeys.is_locale_yes_letter(_key(KEY_O))).is_false()
