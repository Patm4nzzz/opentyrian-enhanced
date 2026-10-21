## Shop purchase list (MENU_UPGRADE_SUB): the hover only changes the selection
## once the pointer has rested on a row (shop.gd:HOVER_DWELL_MS).
##
## Owner's bug (recording of 2026-10-20, Rear Gun): moving from "Heavy Guided
## Bombs" down-left to the power arrows crossed the "None" row of the choices
## column (170 < x < 308) for a few milliseconds; the hover made it the
## selection (gun refunded, hatch closed, arrows gone) and it stayed there
## since nothing is hovered outside the column.
extends GdUnitTestSuite

## shop.gd:HOVER_DWELL_MS.
const DWELL_MS := 150

var _now := 1000


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true
	Game.campaign_active = false


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true


static func _motion(x: float, y: float) -> InputEventMouseMotion:
	var ev := InputEventMouseMotion.new()
	ev.position = Vector2(x, y)
	ev.relative = Vector2(1, 1)
	return ev


## Rows of MENU_UPGRADE_SUB: (y - 38) / 26 + 2 (game_menu.c:mouseSelectionY).
static func _row_y(row: int) -> float:
	return 38.0 + 26.0 * (row - 2) + 1.0


## Front Gun list of the episode 1 shop, 40000 credits (several affordable guns).
func _open_front_gun_list() -> Node:
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 40000])
	Game.campaign_active = true
	Game.episode = 1
	Game.main_level = 4
	var r: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
	Game.shop_section = int(r.get("section", 4))
	Game.map_planets = PackedInt32Array()
	var mi := 0
	while r.has("planet%d" % mi):
		Game.map_planets.append(r["planet%d" % mi])
		mi += 1
	Game.set_shop(r, 0)
	Game.debrief = {"cube_max": 0}
	var shop: Node = Game.goto_scene(Game.SCENE_SHOP)
	var w := 0
	while (shop.get("_shop") == null or not shop.idle) and w < 600:
		await get_tree().process_frame
		w += 1
	# Root: Upgrade Ship (sel 4), then Front Gun (sel 3).
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	shop.handle_action(TyrianControls.DOWN)
	shop.handle_action(TyrianControls.ACCEPT)
	# set(): on a shop without the dwell, the test fails on behavior, not on a script error.
	shop.set("hover_clock", func() -> int: return _now)
	TyrianMouse.inactive = false
	return shop


func _sel(shop: Node) -> int:
	return int(shop._state["sel"])


func test_hover_applies_after_dwell() -> void:
	var shop: Node = await _open_front_gun_list()
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_UPGRADE_SUB)
	var done: int = int(shop._state["menu_choices"])
	var start := _sel(shop)
	var target := 3 if start == 2 else 2
	assert_int(target).is_less(done)
	_now = 1000
	shop.handle_mouse(_motion(220, _row_y(target)))
	assert_int(_sel(shop)).override_failure_message("hover applied before the dwell").is_equal(start)
	_now = 1000 + DWELL_MS - 1
	shop.handle_mouse(_motion(221, _row_y(target)))
	assert_int(_sel(shop)).is_equal(start)
	# Pointer at rest on the row: applied by _process, no further movement needed.
	_now = 1000 + DWELL_MS
	await get_tree().process_frame
	assert_int(_sel(shop)).override_failure_message("hover not applied after the dwell").is_equal(target)


## The owner's gesture: on a gun, the pointer leaves the column down-left
## toward the right power arrow (x 119-131, y 149-168) and crosses the "Done"
## row on the way. The gun stays selected and its arrows stay usable.
func test_crossing_a_row_on_the_way_to_the_power_arrows_keeps_the_gun() -> void:
	var shop: Node = await _open_front_gun_list()
	var done: int = int(shop._state["menu_choices"])
	var start := _sel(shop)
	var gun := 3 if start == 2 else 2
	_now = 1000
	shop.handle_mouse(_motion(220, _row_y(gun)))
	_now += DWELL_MS
	await get_tree().process_frame
	assert_int(_sel(shop)).is_equal(gun)
	# Swept through every lower row in a few ms, then out of the column.
	for row in range(gun + 1, done + 1):
		_now += 10
		shop.handle_mouse(_motion(200, _row_y(row)))
	_now += 10
	shop.handle_mouse(_motion(160, 155))
	_now += 10
	shop.handle_mouse(_motion(126, 158))
	_now += 5000
	await get_tree().process_frame
	assert_int(_sel(shop)).override_failure_message("a row crossed in passing became the selection").is_equal(gun)
	assert_int(int(shop._state["menu"])).is_equal(shop.MENU_UPGRADE_SUB)


## The keyboard takes over (mouseInactive): a pending hover is dropped.
func test_key_drops_pending_hover() -> void:
	var shop: Node = await _open_front_gun_list()
	var start := _sel(shop)
	var target := 3 if start == 2 else 2
	_now = 1000
	shop.handle_mouse(_motion(220, _row_y(target)))
	TyrianMouse.inactive = true
	_now += 5000
	await get_tree().process_frame
	assert_int(_sel(shop)).is_equal(start)
