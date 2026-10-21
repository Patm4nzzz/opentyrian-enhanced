# Effects gain = that of the C (loudness.c): volumeFactorTable[v] × (level+1)/8.
# At 191 and at the menu level (4): 0.4217 × 5/8 = 0.2636, i.e. ≈ −11.6 dB (report: −11.58, rounded).
extends GdUnitTestSuite

const SfxScript := preload("res://scripts/sfx.gd")


func _c_gain_db(volume: int, level: int) -> float:
	var table := pow(10.0, (255 - volume) * (-30.0 / (20.0 * 255)))
	return 20.0 * log(table * float(level + 1) / 8.0) / log(10.0)


func test_default_volume_matches_c_gain() -> void:
	var sfx: Node = SfxScript.new()
	add_child(sfx)
	sfx.volume = 191
	var got: float = sfx._volume_db() + Sfx.level_db(Sfx.FX_PLAY_VOL)
	sfx.free()
	assert_float(got).is_equal_approx(_c_gain_db(191, 4), 0.01)
	assert_float(got).is_equal_approx(-11.6, 0.05)


func test_slope_and_channel_levels_match_c() -> void:
	var sfx: Node = SfxScript.new()
	add_child(sfx)
	for v in [255, 143, 95, 12]:
		sfx.volume = v
		for level in [0, 2, 4, 7]:
			var got: float = sfx._volume_db() + Sfx.level_db(level)
			assert_float(got).is_equal_approx(_c_gain_db(v, level), 0.01)
	sfx.free()
