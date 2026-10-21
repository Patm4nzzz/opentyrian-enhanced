## Every mod ship slot drawn with its five poses (ADR 0134): with the two
## shipped ship packs enabled, the 14 slots (19..32) are taken and level.gd
## finds an image for each banking step (shipgraphic + bank * 2, bank -2..2),
## the five being the pack's five PNGs. Before ADR 0134 the ship image table
## held 44 PNGs: the second pack's ships were drawn partly or not at all.
## The original ships keep their sheet image (no mod image at their index).
extends GdUnitTestSuite

const FOLDERS := ["alien_ship_pack", "bio_ships_pack"]
const FIRST_MOD_SHIP := 19
const LAST_MOD_SHIP := 32


func after_test() -> void:
	for f in FOLDERS:
		Mods.set_enabled(f, false)
	Mods.reload()


func _enable_ship_packs() -> void:
	for m in Mods.list():
		Mods.set_enabled(String(m["folder"]), FOLDERS.has(String(m["folder"])))
	Mods.reload()


func test_fourteen_mod_ships_have_five_distinct_poses() -> void:
	_enable_ship_packs()
	var used := {}
	for ship in range(FIRST_MOD_SHIP, LAST_MOD_SHIP + 1):
		var g: int = Mods.ship_graphic(ship)
		assert_int(g).override_failure_message("ship %d: no shipgraphic" % ship).is_greater(500)
		var poses := []
		for bank in range(-2, 3):
			# level.gd:_draw_player.
			var t := Mods.texture("ships", g + bank * 2)
			assert_object(t).override_failure_message("ship %d, bank %d: no image" % [ship, bank]).is_not_null()
			(
				assert_bool(poses.has(t))
				. override_failure_message("ship %d, bank %d: same image as another pose" % [ship, bank])
				. is_false()
			)
			poses.append(t)
			(
				assert_bool(used.has(g + bank * 2))
				. override_failure_message("ship %d: index %d shared" % [ship, g + bank * 2])
				. is_false()
			)
			used[g + bank * 2] = true
	assert_int(used.size()).is_equal(14 * 5)


func test_original_ships_keep_their_own_image() -> void:
	_enable_ship_packs()
	var ships: Array = TyrianAssets.read_json("items.json")["episodes_1_3"]["ships"]
	for ship in range(1, mini(ships.size(), FIRST_MOD_SHIP)):
		var g := int(ships[ship].get("shipgraphic", 0))
		if g <= 0:
			continue
		for bank in range(-2, 3):
			(
				assert_object(Mods.texture("ships", g + bank * 2))
				. override_failure_message("original ship %d: mod image at %d" % [ship, g + bank * 2])
				. is_null()
			)
