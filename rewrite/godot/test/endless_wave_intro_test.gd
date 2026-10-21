# Endless wave entry banner (endless.c:endless_draw_hud): absent under
# the Eyes Closed anomaly (the C leaves the function before the banner); "..."
# when the level name is empty.
extends GdUnitTestSuite


class FakeRun:
	extends RefCounted
	var effects := 0
	var name := "TYRIAN"

	func anomaly_effects() -> int:
		return effects

	func level_name() -> String:
		return name

	func wave() -> int:
		return 3


var _saved: RefCounted


func before_test() -> void:
	_saved = EndlessSession.run


func after_test() -> void:
	EndlessSession.run = _saved


func test_banner_shown_at_wave_start() -> void:
	EndlessSession.run = FakeRun.new()
	var lines := HudRenderer.wave_intro_lines(0)
	assert_str(lines[0]).is_not_empty()
	assert_str(lines[1]).contains("TYRIAN")


func test_no_banner_with_eyes_closed() -> void:
	var r := FakeRun.new()
	r.effects = 0x20
	EndlessSession.run = r
	var lines := HudRenderer.wave_intro_lines(0)
	assert_str(lines[0]).is_empty()
	assert_str(lines[1]).is_empty()


func test_empty_level_name_shows_dots() -> void:
	var r := FakeRun.new()
	r.name = ""
	EndlessSession.run = r
	assert_str(HudRenderer.wave_intro_lines(0)[1]).contains("...")


## endless.c:1300: real clock (SDL_GetTicks), 2,000 ms.
func test_banner_ends_after_2s() -> void:
	EndlessSession.run = FakeRun.new()
	assert_str(HudRenderer.wave_intro_lines(1999)[0]).is_not_empty()
	assert_str(HudRenderer.wave_intro_lines(2000)[0]).is_empty()


## The banner follows the level's real clock, not the simulation ticks:
## advancing the simulation without letting time pass does not turn it off.
func test_banner_follows_real_clock_not_ticks() -> void:
	var src := FileAccess.get_file_as_string("res://scripts/hud_renderer.gd")
	assert_bool(src.contains("wave_intro_lines(wave_intro_elapsed_ms)")).is_true()
	assert_bool(src.contains("tick * 2875")).override_failure_message("banner still counted in ticks").is_false()
