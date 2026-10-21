# Game modes: mode flags, loaded level, 300 ticks played without level
# end.
extends GdUnitTestSuite

const GALAGA := 3
const SetupScreen := preload("res://scripts/setup.gd")
const ModesMenuScene := preload("res://scenes/modes_menu.tscn")


func after() -> void:
	Game.modes = PackedInt32Array()
	Game.modes_extra = []
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


## B-02: Setup mutators are added to the modes built by New Game (modes[6] = 0).
func test_setup_mutators_merged_into_new_game_modes() -> void:
	var saved_m := Game.mutators
	var saved_modes := Game.modes
	Game.modes = PackedInt32Array([0, 1, 1, 0, 1, 0, 0, 0, 1, 1])
	Game.mutators = 0x02 | 0x80
	var merged := Game.modes_with_mutators(true)
	assert_int(merged[6]).override_failure_message("Setup mutators missing from the modes").is_equal(0x82)
	assert_int(Game.modes_with_mutators(false)[6]).override_failure_message("replay: unchanged").is_equal(0)
	assert_int(Game.modes[6]).override_failure_message("Game.modes not modified").is_equal(0)
	Game.mutators = saved_m
	Game.modes = saved_modes


## B-18: a toggle on the modes screen is saved to the settings.
func test_modes_menu_mutator_toggle_persists() -> void:
	var menu: Node = auto_free(ModesMenuScene.instantiate())
	get_tree().root.add_child(menu)
	await get_tree().process_frame
	Game.mutators = 0
	SetupScreen.settings.mutators = 0
	var item = menu.menus.current().items[1]
	item.set.call(true)
	assert_int(Game.mutators).is_equal(1)
	assert_int(int(SetupScreen.settings.mutators)).override_failure_message("setting not persisted").is_equal(1)
	Game.mutators = 0
	SetupScreen.settings.mutators = 0


func test_galaga_level_runs() -> void:
	var menu: Node = auto_free(ModesMenuScene.instantiate())
	get_tree().root.add_child(menu)
	await get_tree().process_frame
	menu.mode_index = GALAGA
	Game.mutators = 0
	Game.anomalies = 0
	menu.start()
	for i in 10:
		await get_tree().process_frame
	var sim: Object = null
	for n in get_tree().root.find_children("TyrianSim", "", true, false):
		sim = n
	assert_object(sim).override_failure_message("no TyrianSim").is_not_null()
	if sim == null:
		return
	assert_bool(sim.is_loaded()).is_true()
	assert_int(Game.episode).is_equal(1)
	assert_int(Game.lvl_file_num).is_equal(18)
	# Player devices at 0 (R-09.1: the C's inputDevice read as 0 by default).
	assert_array(Array(Game.modes)).is_equal([0, 1, 1, 0, 1, 0, 0, 0, 0, 0])
	var start: int = sim.tick()
	for i in 300:
		sim.step(16, 0)
	assert_int(sim.tick() - start).is_equal(300)
	assert_bool(sim.level_ended()).is_false()
