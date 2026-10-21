# GdUnit4: Mods menu, mod_manager.c parity (B-17): R key = reload
# (S_ITEM), mod in error can be toggled, toggle sound S_CLICK.
extends GdUnitTestSuite

const MenuScript := preload("res://scripts/mods_menu.gd")
const FOLDER := "mods_menu_test_bad"

var _dir: String


func before_test() -> void:
	_dir = ProjectSettings.globalize_path(Mods.USER_MODS).path_join(FOLDER)
	DirAccess.make_dir_recursive_absolute(_dir)
	var f := FileAccess.open(_dir.path_join("mod.json"), FileAccess.WRITE)
	f.store_string("{ this is not json")
	f.close()
	Mods.reload()


func after_test() -> void:
	Mods._remove_tree(_dir)
	Mods.reload()


func _menu() -> Node:
	var m: Node = auto_free(MenuScript.new())
	add_child(m)
	return m


func _select_bad(m: Node) -> void:
	for i in m.mods.size():
		if m.mods[i]["folder"] == FOLDER:
			m.menus.stack.back().sel = i
			return
	fail("test mod missing from the list")


func test_errored_mod_can_be_toggled_with_click_sound() -> void:
	var m := _menu()
	_select_bad(m)
	assert_str(String(m.mods[m.index]["error"])).is_not_empty()
	var before: bool = m.mods[m.index]["enabled"]
	m.toggle()
	assert_bool(m.mods[m.index]["enabled"]).override_failure_message("mod in error not toggled").is_equal(not before)
	assert_int(m.last_sfx).is_equal(Sfx.CLICK)


func test_r_key_reloads_with_item_sound() -> void:
	var m := _menu()
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_R
	ev.keycode = KEY_R
	ev.pressed = true
	m._unhandled_input(ev)
	assert_int(m.last_sfx).override_failure_message("R did not reload (S_ITEM)").is_equal(Sfx.ITEM)


## The "Install a mod" box does not outlive the Mods screen: exit via
## leave(), or screen freed another way (_exit_tree).
func _dialogs(m: Node) -> Array:
	return m.get_children().filter(func(c: Node) -> bool: return c is FileDialog and not c.is_queued_for_deletion())


func test_install_dialog_closes_when_leaving_screen() -> void:
	var m := _menu()
	m.choose_zip()
	assert_int(_dialogs(m).size()).override_failure_message("dialog not open").is_equal(1)
	m._close_dialog()
	assert_int(_dialogs(m).size()).override_failure_message("dialog left open").is_equal(0)


func test_install_dialog_never_outlives_scene() -> void:
	var m := _menu()
	m.choose_zip()
	var dialog: FileDialog = _dialogs(m)[0]
	remove_child(m)
	(
		assert_bool(is_instance_valid(dialog) and dialog.visible)
		. override_failure_message("dialog visible after leaving the tree")
		. is_false()
	)


func test_install_dialog_not_duplicated() -> void:
	var m := _menu()
	m.choose_zip()
	m.choose_zip()
	assert_int(_dialogs(m).size()).is_equal(1)


## D4 (Windows): hide() does not close the native box; a choice made later
## in the box left open installs nothing.
func test_late_choice_after_leaving_installs_nothing() -> void:
	var m := _menu()
	m.choose_zip()
	var dialog: FileDialog = _dialogs(m)[0]
	m._close_dialog()
	(
		assert_bool(dialog.file_selected.is_connected(m._on_zip_chosen))
		. override_failure_message("dialog still attached to the screen")
		. is_false()
	)


## D4 (Windows, round 5 final gate): the native box cannot be closed by
## code; while it is open, Esc and the menu keys do not
## leave the Mods screen (otherwise the box stays open behind the game).
func test_keys_ignored_while_install_dialog_open() -> void:
	var m := _menu()
	m.choose_zip()
	var left := [false]
	m.tree_exiting.connect(func() -> void: left[0] = true)
	for key in [KEY_ESCAPE, KEY_DOWN, KEY_ENTER, KEY_R]:
		var ev := InputEventKey.new()
		ev.pressed = true
		ev.keycode = key
		ev.physical_keycode = key
		m._unhandled_input(ev)
	assert_bool(left[0]).override_failure_message("screen left with the dialog open").is_false()
	assert_int(_dialogs(m).size()).override_failure_message("dialog closed by a key").is_equal(1)


## Cancelling the box hands control back to the screen.
func test_cancel_gives_keys_back() -> void:
	var m := _menu()
	m.choose_zip()
	var dialog: FileDialog = _dialogs(m)[0]
	dialog.canceled.emit()
	assert_bool(m.dialog_open()).override_failure_message("dialog still counted open after Cancel").is_false()


## Real FileDialog order: the box hides (visibility_changed) THEN emits
## file_selected. The choice must still install the mod.
func test_choice_after_dialog_hides_installs() -> void:
	var m := _menu()
	m.choose_zip()
	var dialog: FileDialog = _dialogs(m)[0]
	dialog.hide()
	assert_bool(m.dialog_open()).override_failure_message("dialog still counted open after hide").is_false()
	dialog.file_selected.emit("/nonexistent/round5_gate.zip")
	(
		assert_bool(m.install_failed)
		. override_failure_message("the choice made after hide() tried to install nothing")
		. is_true()
	)


## mod_manager.c:1667-1683: Up on the first row and Down on the last do not
## wrap; Space does not confirm (INPUT_MENU_CONFIRM = Enter only).
func test_navigation_is_bounded_and_space_does_not_validate() -> void:
	var m := _menu()
	var menu: Dictionary = m.menus.stack.back()
	menu.sel = 0
	m.menus.handle_action(TyrianControls.UP)
	assert_int(m.menus.selection()).override_failure_message("Up on the first row wrapped around").is_equal(0)
	menu.sel = menu.items.size() - 1
	m.menus.handle_action(TyrianControls.DOWN)
	assert_int(m.menus.selection()).override_failure_message("Down on the last row wrapped around").is_equal(
		menu.items.size() - 1
	)
	_select_bad(m)
	var before: bool = m.mods[m.index]["enabled"]
	m.menus.handle_action(TyrianControls.SELECT)
	assert_bool(m.mods[m.index]["enabled"]).override_failure_message("Space toggled the mod").is_equal(before)
	m.menus.handle_action(TyrianControls.ACCEPT)
	assert_bool(m.mods[m.index]["enabled"]).override_failure_message("Enter did not toggle the mod").is_equal(
		not before
	)
