## Mod items in the shop (player report on v1.0-rc1): a mod ship showed no
## armor, and a mod weapon did not seem to fire in the demonstration.
##   - armor and shield bars: read from the mods' item when a mod adds the id
##     (Mods.item_row), items.json only knows the game's own tables;
##   - demonstration: the shots drawn by a mod PNG are laid over the shop image
##     (TyrianShop.weapon_view_shots), the 8-bit image only knows the original
##     sheets.
extends GdUnitTestSuite

const SHOP := preload("res://scripts/shop.gd")
const SHIPS := "alien_ship_pack"
const WEAPONS := "alien_weapon_pack"
const VGA_SCREEN := 0


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Game.campaign_active = false
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	Game.campaign_active = false
	Game.shop_menu_rows = {}
	for folder in [SHIPS, WEAPONS]:
		Mods.set_enabled(folder, false)
	Mods.reload()


func _enable(folder: String) -> void:
	Game.episode = 1
	Mods.set_enabled(folder, true)
	Mods.reload()


## Item of Mods.items() by kind and name ({} if the mod is not loaded).
func _mod_item(kind: String, item_name: String) -> Dictionary:
	for it in Mods.items():
		if String(it["kind"]) == kind and String(it["name"]).strip_edges() == item_name:
			return it
	return {}


## Shop of section 4 of episode 1 (same state as shop_rows_test).
func _open_shop(loadout: Array) -> Node:
	Game.loadout = PackedInt32Array(loadout)
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	Game.shop_items_available = r.get("items_available", [])
	var m: PackedInt32Array = r.get("map", PackedInt32Array())
	Game.map_sections = PackedInt32Array()
	Game.map_planets = PackedInt32Array()
	for i in range(1, m.size(), 2):
		Game.map_planets.append(m[i - 1])
		Game.map_sections.append(m[i])
	Game.debrief = {"cube_max": 0}
	return Game.goto_scene(Game.SCENE_SHOP)


## Zica Ember: "damage_resistance": 12 in the mod. The armor bar of the shop
## (bar_draw_shadow(42, 152, 3, 14, dmg, 2, 13)) starts with a pixel of color
## 14 + 15 at (42, 152); with an armor of 0 nothing is drawn there.
func test_mod_ship_shows_its_armor_in_the_shop() -> void:
	_enable(SHIPS)
	var ship := _mod_item("ship", "Zica Ember")
	assert_dict(ship).override_failure_message("Zica Ember not provided by %s" % SHIPS).is_not_empty()
	var id := int(ship["id"])
	assert_int(int(ship.get("dmg", -1))).is_equal(12)
	var items: Dictionary = TyrianAssets.read_json("items.json")["episodes_1_3"]
	assert_int(int(SHOP.item_row(items, "ships", id).get("dmg", 0))).is_equal(12)
	# An original ship still comes from items.json (USP Talon, armor 10).
	assert_int(int(SHOP.item_row(items, "ships", 1).get("dmg", 0))).is_equal(int(items["ships"][1]["dmg"]))
	var shop: Node = _open_shop([id, 1, 1, 0, 1, 4, 2, 0, 0, 0, 5000])
	await get_tree().process_frame
	var pixels: PackedByteArray = shop.vga.indices(VGA_SCREEN)
	assert_int(pixels[152 * 320 + 42]).override_failure_message("no armor bar for the mod ship").is_equal(14 + 15)


## Without the mod the same id has no armor: the bar comes from the mod.
func test_armor_row_follows_the_enabled_mods() -> void:
	_enable(SHIPS)
	var id := int(_mod_item("ship", "Zica Ember")["id"])
	Mods.set_enabled(SHIPS, false)
	Mods.reload()
	assert_dict(Mods.item_row("ships", id)).is_empty()


## Zica Thermal Lance mounted as the front gun: in Upgrade Ship > Front Gun,
## on the row of the lance, the demonstration shows its shots, each one drawn
## by a mod PNG inside the demonstration window.
func test_mod_weapon_fires_in_the_shop_demonstration() -> void:
	_enable(WEAPONS)
	var weapon := _mod_item("weapon", "Zica Thermal Lance")
	assert_dict(weapon).override_failure_message("Zica Thermal Lance not provided by %s" % WEAPONS).is_not_empty()
	var id := int(weapon["id"])
	assert_array(Array(Mods.texture_indices("shots"))).is_not_empty()
	var shop: Node = _open_shop([1, id, 1, 0, 1, 4, 2, 0, 0, 0, 5000])
	await get_tree().process_frame
	# Root menu: Upgrade Ship; then the Front Gun row and its item list.
	for a in [TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT, TyrianControls.DOWN]:
		shop.handle_action(a)
	assert_int(int(shop._state["category"])).is_equal(3)
	shop.handle_action(TyrianControls.ACCEPT)
	var most := 0
	var outside := 0
	for row in int(shop._state["menu_choices"]):
		if int(shop._state["loadout"][1]) == id:
			for i in 60:
				shop._anim_frame()
				most = maxi(most, shop._mod_shots.size())
				for shot in shop._mod_shots:
					var at: Vector2 = shot[1]
					if at.x < 0 or at.x > 142 or at.y < 0 or at.y > 170:
						outside += 1
			break
		shop.handle_action(TyrianControls.DOWN)
	assert_int(int(shop._state["loadout"][1])).override_failure_message("lance row not found in the list").is_equal(id)
	assert_int(most).override_failure_message("no mod shot drawn in the demonstration").is_greater(0)
	assert_int(outside).is_equal(0)
	# Leaving the list removes the shots laid over the image.
	shop.handle_action(TyrianControls.BACK)
	assert_array(shop._mod_shots).is_empty()


## Temporary mod written in the player's mods folder: a new sidekick with its
## icon in sprites/sidekicks/, a sidekick copied from an original one without
## an image of its own, and a PNG that replaces the shop icon of the
## Pulse-Cannon (sprites/pickups/weapons/pulse_cannon.png).
const TEMP_MOD := "qa_shop_icons"


func _write_temp_mod() -> String:
	var dir := ProjectSettings.globalize_path(Mods.USER_MODS).path_join(TEMP_MOD)
	for sub in ["sprites/sidekicks", "sprites/pickups/weapons"]:
		DirAccess.make_dir_recursive_absolute(dir.path_join(sub))
	var img := Image.create(24, 28, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 1, 1, 1))
	img.save_png(dir.path_join("sprites/sidekicks/qa_pod_icon.png"))
	img.save_png(dir.path_join("sprites/pickups/weapons/pulse_cannon.png"))
	var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
	(
		f
		. store_string(
			(
				'{"format_version": 1, "name": "QA shop icons", "sidekicks": ['
				+ '{"name": "QA Pod", "base": "Single Shot Option", "icon_sprite": "qa_pod_icon", "always_available": true},'
				+ '{"name": "QA Copy", "base": "Dual Shot Option", "always_available": true}]}'
			)
		)
	)
	f.close()
	return dir


func _remove_temp_mod(dir: String) -> void:
	Mods.set_enabled(TEMP_MOD, false)
	for rel in ["sprites/sidekicks/qa_pod_icon.png", "sprites/pickups/weapons/pulse_cannon.png", "mod.json"]:
		DirAccess.remove_absolute(dir.path_join(rel))
	for rel in ["sprites/sidekicks", "sprites/pickups/weapons", "sprites/pickups", "sprites", ""]:
		DirAccess.remove_absolute(dir.path_join(rel) if not rel.is_empty() else dir)
	Mods.reload()


## game_menu.c:JE_drawItem: every kind of item reads its own table of mod
## images, by image number: the icon of a new sidekick, and the replaced icon
## of an original weapon. A mod item copied from an original one, without an
## image of its own, shows the original image instead of nothing.
func test_shop_shows_mod_icons_for_sidekicks_and_replaced_originals() -> void:
	var dir := _write_temp_mod()
	_enable(TEMP_MOD)
	var pod := _mod_item("sidekick", "QA Pod")
	var copy := _mod_item("sidekick", "QA Copy")
	assert_dict(pod).override_failure_message("temporary mod not loaded").is_not_empty()
	var shop: Node = _open_shop([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 5000])
	await get_tree().process_frame
	(
		assert_object(shop._mod_image(6, int(pod["id"])))
		. override_failure_message("no PNG for the mod sidekick's icon")
		. is_not_null()
	)
	assert_object(shop._mod_image(2, 1)).override_failure_message("replaced Pulse-Cannon icon not used").is_not_null()
	# Multi-Cannon (2): no mod image, the original sheet applies.
	assert_object(shop._mod_image(2, 2)).is_null()
	# The copy has no PNG: its row gives the original icon of Dual Shot Option.
	assert_object(shop._mod_image(6, int(copy["id"]))).is_null()
	var items: Dictionary = TyrianAssets.read_json("items.json")["episodes_1_3"]
	var copied := int(SHOP.item_row(items, "sidekicks", int(copy["id"])).get("itemgraphic", 0))
	assert_int(copied).is_equal(int(items["sidekicks"][2]["itemgraphic"]))
	assert_int(copied).is_greater(0)
	_remove_temp_mod(dir)
