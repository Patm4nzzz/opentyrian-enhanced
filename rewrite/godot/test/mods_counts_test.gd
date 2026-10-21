## Mods menu, card of the chosen mod ("weapons ... ships ..." counters) right
## after enabling it with Enter, before leaving the menu: the counters
## come from the enabled mod, not from the state loaded before ("ships 8" for Alien
## Ship Pack). The C (mod_manager.c:1600-1650)
## shows its information from the mod.json, always correct.
extends GdUnitTestSuite

const MenuScript := preload("res://scripts/mods_menu.gd")
const FOLDER := "alien_ship_pack"


func after_test() -> void:
	Mods.set_enabled(FOLDER, false)
	Mods.reload()


func test_counts_shown_right_after_enabling() -> void:
	Mods.set_enabled(FOLDER, false)
	Mods.reload()
	var m: Node = auto_free(MenuScript.new())
	add_child(m)
	var row := -1
	for i in m.mods.size():
		if m.mods[i]["folder"] == FOLDER:
			row = i
	assert_int(row).override_failure_message("shipped mod alien_ship_pack missing").is_greater_equal(0)
	m.menus.stack.back().sel = row
	m.toggle()
	assert_bool(m.mods[row]["enabled"]).is_true()
	assert_int(int(m.mods[row]["ships"])).override_failure_message("sheet: ships 0 right after activation").is_equal(8)
