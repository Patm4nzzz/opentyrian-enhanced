## Rows kept by the shop during a visit (game_menu.c:335-337: curSel[]
## reset to the first row only on entering JE_itemScreen).
extends GdUnitTestSuite


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


## Shop of section 4 of episode 1 (same state as shop_sounds_test).
func _open_shop() -> Node:
	Game.loadout = PackedInt32Array([1, 1, 11, 0, 1, 4, 2, 0, 0, 0, 15267])
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


## Upgrade Ship on "Rear Gun", back to the root, Options, back: Upgrade
## Ship reopened must start again on "Rear Gun" (the C keeps curSel[MENU_UPGRADES]).
func test_upgrade_ship_keeps_its_row_after_options() -> void:
	var shop: Node = _open_shop()
	await get_tree().process_frame
	for a in [
		TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT, TyrianControls.DOWN, TyrianControls.DOWN
	]:
		shop.handle_action(a)
	assert_int(int(shop._state["category"])).is_equal(4)
	shop.handle_action(TyrianControls.BACK)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	await get_tree().process_frame
	await get_tree().process_frame
	# Return from Options, like save_menu.gd (Esc at the root of Options).
	Game.shop_resume_sel = 5
	shop = Game.goto_scene(Game.SCENE_SHOP)
	await get_tree().process_frame
	assert_int(int(shop._state["sel"])).is_equal(5)
	shop.handle_action(TyrianControls.UP)
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(1)
	assert_int(int(shop._state["sel"])).override_failure_message("Upgrade Ship: Rear Gun row lost").is_equal(4)
