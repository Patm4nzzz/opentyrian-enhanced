# Setup gameplay effects (gameplay_menu.c, graphics_menu.c, mutators_menu.c):
# Freeze Frames (hitstop.c), Damage Numbers (damagenumbers.c), Rumble
# (rumble.c), Explosions (particles.c:explosion_style) and Mutators. Each
# option is set with real keys in Setup, saved to
# user://setup.cfg, then its effect is measured in a level played tick by
# tick (same inputs, same seed).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")
const C := preload("res://scripts/controls.gd")

## Permanent fire and left/right sweep (tyrian-sim, tests/determinism.rs).
const FIRE := 16
const LEFT := 4
const RIGHT := 8


func before() -> void:
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.campaign_active = false


func before_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()


func after_test() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path(SetupScreen.SETTINGS_PATH))
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Game.mutators = 0
	Effects.kill_slowmo = -1.0
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame
	await get_tree().process_frame


func after() -> void:
	Music.release_streams()
	Sfx.release_streams()


# --- Setup driven by keyboard ----------------------------------------------------


## Key pressed then released, through the real input path
## (Effects passes it to the game screen).
func _key(code: int) -> void:
	var ev := C.key_event(code)
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	var up: InputEventKey = ev.duplicate()
	up.pressed = false
	Input.parse_input_event(up)
	Input.flush_buffered_events()
	await get_tree().process_frame


func _open_setup() -> Node:
	var setup: Node = Game.goto_scene(Game.SCENE_SETUP)
	for i in 300:
		await get_tree().process_frame
		if setup.idle:
			break
	assert_bool(setup.idle).override_failure_message("Setup never ready").is_true()
	return setup


func _current_label(setup: Node) -> String:
	var m: Dictionary = setup.menus.current()
	return m.items[m.sel].label


## Goes down to the `label` row of the current menu.
func _goto_row(setup: Node, label: String) -> void:
	for i in 20:
		if _current_label(setup) == label:
			return
		await _key(KEY_DOWN)
	assert_str(_current_label(setup)).is_equal(label)


func _labels(items: Array) -> Array:
	return items.map(func(it): return it.label)


## Rereads the saved file (load_settings) after resetting to defaults.
func _reload_settings() -> void:
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	SetupScreen.load_settings()


# --- Level played tick by tick ---------------------------------------------------


func _sim() -> Object:
	for n in Effects.game_viewport.find_children("TyrianSim", "", true, false):
		return n
	return null


## Episode 1 level (TYRIAN), fixed seed, driven by step_sim alone.
## C equipment (mainint.c:JE_initPlayerData) like the
## determinism tests; `level_num`/`power` for a level that fires harder.
func _start_level(seed := 7, level_num := 9, power := 1) -> Node:
	Game.episode = 1
	Game.lvl_file_num = level_num
	Game.start_cur_loc = 0
	Game.rng_seed = seed
	Game.difficulty = 2
	Game.loadout = PackedInt32Array([1, 4, power, 0, 1, 4, 2, 0, 0, 0, 10000])
	var level: Node = Game.goto_scene(Game.SCENE_LEVEL)
	level.interactive = false
	await get_tree().process_frame
	return level


func _mask(t: int) -> int:
	return FIRE | (LEFT if (t / 90) % 2 == 0 else RIGHT)


## Plays `ticks` ticks; `each` (optional) receives (tick, sim) after each one.
func _play(level: Node, ticks: int, each := Callable()) -> void:
	for t in ticks:
		level.step_sim(_mask(t))
		if each.is_valid():
			each.call(t, level.sim)


# --- Tests ---


## C order (gameplay_menu.c:113-129, graphics_menu.c) and Mutators submenu.
func test_rows_follow_c_order() -> void:
	var setup: Node = auto_free(load("res://scenes/setup.tscn").instantiate())
	assert_array(_labels(setup._gameplay_items())).contains_exactly(
		[
			"gp_bullet_time",
			"gp_freeze_frames",
			"gp_damage_numbers",
			"gp_combo",
			"gp_auto_fire",
			"gp_rumble",
			"gp_language",
			"gp_mutators",
			"setup_done"
		]
	)
	assert_array(_labels(setup._mutators_items())).contains_exactly(
		[
			"mut_fast_enemies",
			"mut_armored",
			"mut_bullet_hell",
			"mut_glass_cannon",
			"mut_no_powerups",
			"mut_one_life",
			"mut_mirror",
			"mut_risk_reward",
			"mut_reset_all",
			"setup_done"
		]
	)
	var gfx: Array = _labels(setup._graphics_items())
	assert_str(gfx[gfx.size() - 2]).is_equal("gfx_exfx")
	assert_str(gfx[gfx.size() - 1]).is_equal("setup_done")


## C defaults (save_json.c): everything on, colored numbers, Retro+.


## C defaults (save_json.c): everything on, colored numbers, Retro+.
func test_defaults_match_c() -> void:
	assert_bool(SetupScreen.settings.hitstop_enabled).is_true()
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_true()
	assert_bool(SetupScreen.settings.colored_damage_numbers).is_true()
	assert_bool(SetupScreen.settings.rumble_enabled).is_true()
	assert_int(SetupScreen.settings.explosion_style).is_equal(1)
	assert_int(SetupScreen.settings.mutators).is_equal(0)


## Setup > Gameplay by keyboard: Freeze Frames, Damage Numbers (cycle
## off/on/colored), Rumble; saved and read back.


## Setup > Gameplay by keyboard: Freeze Frames, Damage Numbers (cycle
## off/on/colored), Rumble; saved and read back.
func test_gameplay_rows_by_keyboard_persisted() -> void:
	var setup: Node = await _open_setup()
	await _goto_row(setup, "setup_gameplay")
	await _key(KEY_ENTER)
	assert_str(setup.menus.current().header).is_equal("gp_header")
	await _goto_row(setup, "gp_freeze_frames")
	await _key(KEY_ENTER)
	assert_bool(SetupScreen.settings.hitstop_enabled).is_false()
	await _goto_row(setup, "gp_damage_numbers")
	# Colored -> off (right wraps) -> on.
	await _key(KEY_RIGHT)
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_false()
	await _key(KEY_RIGHT)
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_true()
	assert_bool(SetupScreen.settings.colored_damage_numbers).is_false()
	await _goto_row(setup, "gp_rumble")
	await _key(KEY_LEFT)
	assert_bool(SetupScreen.settings.rumble_enabled).is_false()
	_reload_settings()
	assert_bool(SetupScreen.settings.hitstop_enabled).is_false()
	assert_bool(SetupScreen.settings.damage_numbers_enabled).is_true()
	assert_bool(SetupScreen.settings.colored_damage_numbers).is_false()
	assert_bool(SetupScreen.settings.rumble_enabled).is_false()


## Setup > Graphics > Explosions by keyboard: Modern saved and read back.


## Setup > Graphics > Explosions by keyboard: Modern saved and read back.
func test_explosion_style_by_keyboard_persisted() -> void:
	var setup: Node = await _open_setup()
	await _goto_row(setup, "setup_graphics")
	await _key(KEY_ENTER)
	await _goto_row(setup, "gfx_exfx")
	await _key(KEY_RIGHT)
	assert_int(SetupScreen.settings.explosion_style).is_equal(2)
	await _key(KEY_ENTER)  # PICKER without popup: one more notch (Classic)
	assert_int(SetupScreen.settings.explosion_style).is_equal(0)
	_reload_settings()
	assert_int(SetupScreen.settings.explosion_style).is_equal(0)


## Setup > Gameplay > Mutators: toggles and Reset All on Game.mutators,
## saved; applied to the simulation of a normal game (Armored:
## enemy armor doubled, mutators.c / enemies.rs).


## Setup > Gameplay > Mutators: toggles and Reset All on Game.mutators,
## saved; applied to the simulation of a normal game (Armored:
## enemy armor doubled, mutators.c / enemies.rs).
func test_mutators_submenu_applied_to_sim() -> void:
	var setup: Node = await _open_setup()
	await _goto_row(setup, "setup_gameplay")
	await _key(KEY_ENTER)
	await _goto_row(setup, "gp_mutators")
	await _key(KEY_ENTER)
	assert_str(setup.menus.current().header).is_equal("mut_header")
	await _goto_row(setup, "mut_glass_cannon")
	await _key(KEY_ENTER)
	assert_int(Game.mutators).is_equal(0x08)
	await _goto_row(setup, "mut_reset_all")
	await _key(KEY_ENTER)
	assert_int(Game.mutators).is_equal(0)
	# Back to the start of the list (wraparound) then Armored.
	await _key(KEY_DOWN)
	await _key(KEY_DOWN)
	await _key(KEY_DOWN)
	assert_str(_current_label(setup)).is_equal("mut_armored")
	await _key(KEY_ENTER)
	assert_int(Game.mutators).is_equal(0x02)
	Game.mutators = 0
	_reload_settings()
	assert_int(Game.mutators).override_failure_message("mutators not read back").is_equal(0x02)
	# Same game without firing with and without Armored: same enemies, doubled armor.
	var armors := {}
	for mask in [0, 0x02]:
		Game.mutators = mask
		var level: Node = await _start_level()
		for t in 150:
			level.step_sim(0)
		var e: PackedInt32Array = level.sim.enemies()
		var a := []
		for k in range(0, e.size(), 8):
			a.append(e[k + 7])
		armors[mask] = a
		Game.goto_scene(Game.SCENE_TITLE)
		await get_tree().process_frame
	assert_int(armors[0].size()).is_greater(0)
	assert_int(armors[0x02].size()).is_equal(armors[0].size())
	var doubled := 0
	for k in armors[0].size():
		if armors[0][k] < 255:
			assert_int(armors[0x02][k]).is_equal(mini(armors[0][k] * 2, 254))
			doubled += 1
	assert_int(doubled).override_failure_message("no enemy compared").is_greater(0)


## Explosions: particles per style (Classic: no debris; Retro+:
## debris without fireball; Modern: fireballs), identical simulation
## for Classic and Retro+ (rendering only).


## Explosions: particles per style (Classic: no debris; Retro+:
## debris without fireball; Modern: fireballs), identical simulation
## for Classic and Retro+ (rendering only).
func test_explosion_styles_particles() -> void:
	var stats := {}
	var hashes := {}
	for style in [0, 1, 2]:
		SetupScreen.settings.explosion_style = style
		SetupScreen.apply_settings()
		var level: Node = await _start_level()
		var max_fireballs := [0]
		var max_live := [0]
		await _play(
			level,
			900,
			func(_t: int, sim: Object) -> void:
				var s: PackedInt64Array = sim.fx_stats()
				max_live[0] = maxi(max_live[0], s[0])
				max_fireballs[0] = maxi(max_fireballs[0], s[1])
		)
		var s: PackedInt64Array = level.sim.fx_stats()
		stats[style] = [s[3], max_fireballs[0], max_live[0]]
		hashes[style] = level.sim.state_hash()
		Game.goto_scene(Game.SCENE_TITLE)
		await get_tree().process_frame
	prints("styles (created, max fireballs, max alive):", stats)
	assert_int(stats[1][0]).override_failure_message("Retro+ without more particles than Classic").is_greater(
		stats[0][0]
	)
	assert_int(stats[0][1]).is_equal(0)
	assert_int(stats[1][1]).is_equal(0)
	assert_int(stats[2][1]).override_failure_message("Modern without fireball").is_greater(0)
	assert_int(hashes[1]).override_failure_message("the Retro+ style changed the simulation").is_equal(hashes[0])

## Freeze Frames: 3-frame freeze on a big target (hitstop.c), the
## enemies do not move during the freeze; disabled, no freeze and
## simulation identical to that without any effect (golden master).
