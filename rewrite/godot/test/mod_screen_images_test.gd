## Mod images of the 8-bit screens (sprite.c:blit_sprite with
## sprite_override_for_table): a PNG named after a planet, a portrait or a
## weapon picture takes the place of that sprite wherever a screen blits it
## (navigation map and title logos, data cube face, ship illustration of the
## shop), mapped to the palette of its screen. A PNG larger than the original
## sprite is not used, like the C (docs/divergences.md, entry 69).
extends GdUnitTestSuite

const TEMP_MOD := "qa_screen_images"
const SIZE := 8
## [sprites folder, PNG name, sprite table of tyrian.shp, sprite index].
const CASES := [["planets", "010", 3, 10], ["portraits", "buce", 4, 2], ["weapon_icons", "003", 6, 3]]
## A planet PNG larger than its sprite (28x23): the original is drawn.
const OVERSIZED := ["planets", "011", 3, 11]
const AT := Vector2i(40, 30)


func before() -> void:
	Game.episode = 1


func after() -> void:
	_remove_temp_mod()


func _mod_dir() -> String:
	return ProjectSettings.globalize_path(Mods.USER_MODS).path_join(TEMP_MOD)


func _write_temp_mod() -> void:
	var dir := _mod_dir()
	var img := Image.create(SIZE, SIZE, false, Image.FORMAT_RGBA8)
	img.fill(Color(1, 0, 0, 1))
	for c: Array in CASES:
		DirAccess.make_dir_recursive_absolute(dir.path_join("sprites/%s" % c[0]))
		img.save_png(dir.path_join("sprites/%s/%s.png" % [c[0], c[1]]))
	var big := Image.create(200, 150, false, Image.FORMAT_RGBA8)
	big.fill(Color(1, 0, 0, 1))
	big.save_png(dir.path_join("sprites/%s/%s.png" % [OVERSIZED[0], OVERSIZED[1]]))
	var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
	f.store_string('{"format_version": 1, "name": "QA screen images"}')
	f.close()


func _remove_temp_mod() -> void:
	var dir := _mod_dir()
	Mods.set_enabled(TEMP_MOD, false)
	for c: Array in CASES + [OVERSIZED]:
		DirAccess.remove_absolute(dir.path_join("sprites/%s/%s.png" % [c[0], c[1]]))
	for c: Array in CASES:
		DirAccess.remove_absolute(dir.path_join("sprites/%s" % c[0]))
	for rel in ["mod.json", "sprites"]:
		DirAccess.remove_absolute(dir.path_join(rel))
	DirAccess.remove_absolute(dir)
	Mods.reload()


## Palette indices of the 320x200 screen after blitting one sprite at AT.
func _blit(vga: RefCounted, table: int, index: int) -> PackedByteArray:
	vga.clear(0, 0)
	vga.blit_sprite(0, AT.x, AT.y, table, index)
	return vga.indices(0)


## Number of drawn pixels, and whether they all have the same color.
func _drawn(px: PackedByteArray) -> Array:
	var count := 0
	var colors := {}
	for v in px:
		if v != 0:
			count += 1
			colors[v] = true
	return [count, colors.size()]


func test_planet_portrait_and_weapon_picture_take_the_mod_image() -> void:
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	var originals := {}
	for c: Array in CASES + [OVERSIZED]:
		originals[c[1]] = _blit(vga, c[2], c[3])
		(
			assert_int(int(_drawn(originals[c[1]])[0]))
			. override_failure_message("%s: no original sprite" % c[1])
			. is_greater(SIZE * SIZE)
		)
	_write_temp_mod()
	Mods.set_enabled(TEMP_MOD, true)
	Mods.reload()
	for c: Array in CASES:
		var px := _blit(vga, c[2], c[3])
		var drawn := _drawn(px)
		# The 8x8 PNG of one color: 64 pixels of a single palette color, at AT.
		assert_int(int(drawn[0])).override_failure_message("%s/%s: mod image not drawn" % [c[0], c[1]]).is_equal(
			SIZE * SIZE
		)
		assert_int(int(drawn[1])).is_equal(1)
		assert_int(px[AT.y * 320 + AT.x]).is_not_equal(0)
		assert_int(px[(AT.y + SIZE) * 320 + AT.x]).is_equal(0)
	# Larger than the sprite it replaces: the original sprite is drawn.
	assert_bool(_blit(vga, OVERSIZED[2], OVERSIZED[3]) == originals[OVERSIZED[1]]).is_true()
	# Without the mod: the original sprites again.
	Mods.set_enabled(TEMP_MOD, false)
	Mods.reload()
	for c: Array in CASES:
		(
			assert_bool(_blit(vga, c[2], c[3]) == originals[c[1]])
			. override_failure_message("%s/%s: mod image kept after the mod was disabled" % [c[0], c[1]])
			. is_true()
		)
