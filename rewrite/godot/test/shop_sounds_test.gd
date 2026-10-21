## Shop sounds compared with the c-final C (game_menu.c:JE_itemScreen, SHOP track round 8).
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


## Shop of section 4 of episode 1 (same state as menus_test).
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


## game_menu.c:1758-1761: Esc plays S_SPRING (not S_CURSOR); game_menu.c:1949-1954:
## Left / Right only play S_CURSOR in Options and a purchase list,
## silent at the root and in Upgrade Ship.
func test_shop_escape_and_left_right_sounds_like_c() -> void:
	var shop: Node = _open_shop()
	var mark: int = Sfx.played.size()
	shop.handle_action(TyrianControls.LEFT)
	shop.handle_action(TyrianControls.RIGHT)
	assert_array(Sfx.played.slice(mark)).is_empty()
	# Upgrade Ship (sel 4): Left muted.
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(1)
	mark = Sfx.played.size()
	shop.handle_action(TyrianControls.LEFT)
	assert_array(Sfx.played.slice(mark)).is_empty()
	# A purchase list: Left plays S_CURSOR; Esc = S_SPRING alone.
	shop.handle_action(TyrianControls.ACCEPT)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_UPGRADE_SUB)
	mark = Sfx.played.size()
	shop.handle_action(TyrianControls.LEFT)
	assert_array(Sfx.played.slice(mark)).contains([Sfx.CURSOR])
	mark = Sfx.played.size()
	shop.handle_action(TyrianControls.BACK)
	assert_array(Sfx.played.slice(mark)).contains([Sfx.SPRING])
	assert_array(Sfx.played.slice(mark)).not_contains([Sfx.CURSOR])
