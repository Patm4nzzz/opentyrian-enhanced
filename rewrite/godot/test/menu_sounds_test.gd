## Setup menu sounds compared with the c-final C (round 8, menus track):
## dropdown list (menu_tree.c:picker_popup), hover on opening a menu
## (menu_tree.c:run_menu) and volume stop (nortsong.c:JE_changeVolume).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")


func before() -> void:
	TyrianControls.install()


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()
	TyrianMouse.inactive = true


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	TyrianMouse.inactive = true


func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	TyrianMouse.inactive = true
	TyrianMouse.position = Vector2i(-1, -1)
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()


func _setup() -> Node:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	for i in 600:
		await get_tree().process_frame
		if setup.idle:
			break
	assert_bool(setup.idle).override_failure_message("Setup ready").is_true()
	return setup


func _sounds_since(mark: int) -> Array:
	return Sfx.played.slice(mark)


## Setup > Graphics > Scaler: list opened by Enter. Up/Down silent,
## Enter on the original value silent, Enter on another value = S_CLICK.
func test_popup_sounds_like_c() -> void:
	var setup: Node = await _setup()
	setup.handle_action(TyrianControls.ACCEPT)  # Graphics
	assert_str(setup.menus.current().header).is_equal("gfx_header")
	setup.handle_action(TyrianControls.DOWN)  # Scaler
	setup.handle_action(TyrianControls.ACCEPT)
	assert_bool(setup.menus.popup_open()).override_failure_message("Scaler list open").is_true()
	var mark: int = Sfx.played.size()
	setup.handle_action(TyrianControls.DOWN)
	setup.handle_action(TyrianControls.UP)
	(
		assert_array(_sounds_since(mark))
		. override_failure_message("Up/Down in the list: no sound (menu_tree.c:750-753)")
		. is_empty()
	)
	setup.handle_action(TyrianControls.ACCEPT)
	(
		assert_array(_sounds_since(mark))
		. override_failure_message("value unchanged: no sound (menu_tree.c:821-825)")
		. is_empty()
	)
	setup.handle_action(TyrianControls.ACCEPT)
	assert_bool(setup.menus.popup_open()).is_true()
	setup.handle_action(TyrianControls.DOWN)
	mark = Sfx.played.size()
	setup.handle_action(TyrianControls.ACCEPT)
	assert_array(_sounds_since(mark)).override_failure_message("value changed: S_CLICK").is_equal([Sfx.CLICK])


## Pointer placed on another row when a menu opens: S_CLICK
## (menu_tree.c:580-584), not the cursor sound.
func test_pointer_on_row_at_menu_open_plays_click() -> void:
	var setup: Node = await _setup()
	var m: Dictionary = setup.menus.current()
	var target := Vector2i(-1, -1)
	for y in 200:
		if setup.menus.row_at(m, Vector2i(100, y)) == 2:
			target = Vector2i(100, y)
			break
	assert_int(target.y).override_failure_message("row 2 found").is_greater_equal(0)
	TyrianMouse.inactive = false
	TyrianMouse.position = target
	var mark: int = Sfx.played.size()
	setup.menus.open("test", m.items, m.options.merged({"draw": false}))
	assert_int(setup.menus.selection()).is_equal(2)
	assert_array(_sounds_since(mark)).override_failure_message("hover on opening: S_CLICK").is_equal([Sfx.CLICK])


## Setup > Sound > Music: Right at the maximum = S_CURSOR then S_CLINK; below the stop, S_CURSOR alone.
func test_volume_limit_plays_clink() -> void:
	var setup: Node = await _setup()
	setup.handle_action(TyrianControls.DOWN)
	setup.handle_action(TyrianControls.ACCEPT)  # Sound
	assert_str(setup.menus.current().header).is_equal("snd_header")
	SetupScreen.settings["music_volume"] = 100
	var mark: int = Sfx.played.size()
	setup.handle_action(TyrianControls.RIGHT)
	assert_array(_sounds_since(mark)).is_equal([Sfx.CURSOR])
	SetupScreen.settings["music_volume"] = 250
	mark = Sfx.played.size()
	setup.handle_action(TyrianControls.RIGHT)
	assert_int(int(SetupScreen.settings["music_volume"])).is_equal(255)
	assert_array(_sounds_since(mark)).override_failure_message("top stop: S_CLINK (nortsong.c:304-308)").is_equal(
		[Sfx.CURSOR, Sfx.CLINK]
	)
	SetupScreen.settings["music_volume"] = 5
	mark = Sfx.played.size()
	setup.handle_action(TyrianControls.LEFT)
	assert_int(int(SetupScreen.settings["music_volume"])).is_equal(0)
	assert_array(_sounds_since(mark)).override_failure_message("bottom stop: S_CLINK").is_equal([Sfx.CURSOR, Sfx.CLINK])
