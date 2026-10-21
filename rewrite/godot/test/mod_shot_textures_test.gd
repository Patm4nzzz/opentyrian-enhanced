## Shots drawn by a mod PNG: Mods.has_textures("shots") decides the rendering
## path of the shots in a level (level.gd:_update_shots). The extension's fast path
## only knows the original sheets; without this check, the shots of Alien Weapon Pack
## and Ion Piercer (example_nova_weapons) were invisible.
extends GdUnitTestSuite

const FOLDER := "alien_weapon_pack"


func after_test() -> void:
	Mods.set_enabled(FOLDER, false)
	Mods.reload()


func test_weapon_pack_shot_images_are_known() -> void:
	Mods.set_enabled(FOLDER, true)
	Mods.reload()
	assert_bool(Mods.has_textures("shots")).override_failure_message("mod shot PNGs not seen").is_true()


func test_no_mod_no_shot_images() -> void:
	for m in Mods.list():
		Mods.set_enabled(String(m["folder"]), false)
	Mods.reload()
	assert_bool(Mods.has_textures("shots")).is_false()
