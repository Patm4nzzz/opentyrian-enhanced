# Samples are loaded at launch: a menu sound never reads
# the disk during the game (lazy loading froze the main
# loop for several seconds at the first SPRING of Ship Specs).
extends GdUnitTestSuite


func test_every_sample_is_loaded_before_first_play() -> void:
	var sfx: Node = load("res://scripts/sfx.gd").new()
	add_child(sfx)
	var missing: Array = []
	for number in range(1, Sfx.SAMPLE_COUNT + 1):
		var path := TyrianAssets.IMPORTED.path_join("sfx/%d.wav" % number)
		if ResourceLoader.exists(path) and sfx._streams.get(number) == null:
			missing.append(number)
	sfx.free()
	assert_array(missing).is_empty()
