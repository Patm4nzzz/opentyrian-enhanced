## Rear weapon mode (player[0].weapon_mode): the C sets it to 1 at the start of
## a game and never between a shop and a level or between two levels. The mode
## chosen in the shop ("/" in the rear gun list) is the one the next level
## starts with, and the shop opens with the mode of the level that just ended
## (docs/divergences.md, entry 68).
extends GdUnitTestSuite

## Sonic Wave: a rear weapon with two modes (weaponPort[11].opnum).
const SONIC_WAVE := 11


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Game.campaign_active = false
	Game.campaign_carry = PackedInt32Array()
	Music.release_streams()
	Sfx.release_streams()


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	Game.campaign_active = false
	Game.campaign_carry = PackedInt32Array()
	Game.shop_menu_rows = {}


## Shop of section 4 of episode 1 (same state as shop_rows_test).
func _open_shop() -> Node:
	Game.loadout = PackedInt32Array([1, 1, 1, SONIC_WAVE, 1, 4, 2, 0, 0, 0, 5000])
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


func _slash() -> InputEventKey:
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_SLASH
	ev.keycode = KEY_SLASH
	ev.pressed = true
	return ev


func test_new_game_starts_in_mode_one() -> void:
	Game.campaign_carry = PackedInt32Array()
	assert_int(Game.rear_weapon_mode()).is_equal(1)


## "/" in Upgrade Ship > Rear Gun: mode 2 is handed to the next level through
## Game.campaign_carry (TyrianSim.set_campaign_state).
func test_mode_chosen_in_the_shop_goes_to_the_next_level() -> void:
	Game.campaign_carry = PackedInt32Array()
	var shop: Node = _open_shop()
	await get_tree().process_frame
	assert_int(int(shop._state["weapon_mode"])).is_equal(1)
	for a in [
		TyrianControls.DOWN, TyrianControls.DOWN, TyrianControls.ACCEPT, TyrianControls.DOWN, TyrianControls.DOWN
	]:
		shop.handle_action(a)
	assert_int(int(shop._state["category"])).is_equal(4)
	shop.handle_action(TyrianControls.ACCEPT)
	shop.handle_event(_slash())
	assert_int(int(shop._state["weapon_mode"])).override_failure_message("'/' did not change the mode").is_equal(2)
	assert_int(Game.rear_weapon_mode()).is_equal(2)
	# The level reads the mode from the carried state.
	var sim: Node = ClassDB.instantiate("TyrianSim")
	sim.set_loadout(Game.loadout)
	sim.set_campaign_state(2, Game.campaign_carry)
	assert_bool(sim.load_level(Game.data_dir, 1, 9, 2, 7)).is_true()
	assert_int(sim.hud_state()[6]).override_failure_message("level not started in rear mode 2").is_equal(2)
	sim.free()


## The shop opens with the mode of the level that just ended.
func test_shop_opens_with_the_mode_of_the_level() -> void:
	Game.campaign_carry = PackedInt32Array()
	Game.set_rear_weapon_mode(2)
	var shop: Node = _open_shop()
	await get_tree().process_frame
	assert_int(int(shop._state["weapon_mode"])).is_equal(2)
