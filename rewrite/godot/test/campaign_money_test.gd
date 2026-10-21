# Money earned in the campaign ("money" workstream): the Enhanced C multiplies the
# gain of a destroyed enemy by the combo (collision.c:collision_process_kill_chain,
# "evalue * (combo_enabled ? combo_multiplier : 1)"); the original C version does not
# know combos (tyrian2.c, "cash += evalue"). Decision: the
# campaign keeps the original's gains, the Combo option only weighs on
# Endless (EndlessSession.configure -> TyrianSim.set_combos). Tick-by-tick
# measurement (local traces, not versioned; TYRIAN seed 1: original 14,978,
# Enhanced C combos off 14,983, combos on 19,807, Godot 14,983).
extends GdUnitTestSuite

const C := preload("res://scripts/controls.gd")
const ES := preload("res://scripts/endless_session.gd")


func before() -> void:
	ES.stop()
	Game.modes = PackedInt32Array()
	C.install()
	C.reset_defaults()


func after() -> void:
	Game.campaign_active = false
	Game.combos_enabled = true
	CampaignBot.replay_assist = false
	C.reset_defaults()
	ES.stop()
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func after_test() -> void:
	Input.action_release(&"tyrian_fire")
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	Effects.overlay_screen = null
	await get_tree().process_frame


## Input masks of a C golden master (column 2 of each tick line).
func _masks(name: String) -> PackedInt32Array:
	var path := ProjectSettings.globalize_path("res://").path_join("../golden").path_join(name)
	var out := PackedInt32Array()
	for line in FileAccess.get_file_as_string(path).split("\n"):
		var w := line.split(" ", false)
		if w.size() > 8 and w[0].is_valid_int():
			out.append(w[1].hex_to_int())
	return out


## TYRIAN level (episode 1, lvlfile 9) in the campaign, like Game.advance_campaign.
func _campaign_level() -> Node:
	Game.campaign_active = true
	Game.mini_game = false
	Game.initial_difficulty = 2
	Game.difficulty = 2
	Game.rng_seed = 1
	Game.campaign_carry = PackedInt32Array()
	Game.loadout = PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 10000])
	Game.start_level_at(1, 9, 0, {"load_scene": false})
	var scene: Node = Game.goto_scene(Game.SCENE_LEVEL)
	scene.set_physics_process(false)
	return scene


## Campaign, Combo option on (default): the e1-tyrian-autopilot inputs
## replayed in the real level scene give the original's money (and the
## C's with combos off), never multiplied.
func test_campaign_cash_follows_original_with_combo_option_on() -> void:
	Game.combos_enabled = true
	CampaignBot.replay_assist = true  # golden master helpers "assist 1"
	var scene := _campaign_level()
	var sim: Node = scene.sim
	var masks := _masks("e1-tyrian-autopilot.gm")
	var cash := int(sim.player_dict(0)["cash"])
	var gains := 0
	var max_mult := 1
	for t in masks.size():
		if sim.level_ended():
			break
		scene.advance(1, func(_i): return masks[t])
		max_mult = maxi(max_mult, int(sim.combo_multiplier()))
		var c := int(sim.player_dict(0)["cash"])
		if c != cash:
			gains += 1
			cash = c
	assert_bool(sim.level_ended()).is_true()
	assert_int(cash).is_equal(14983)
	assert_int(max_mult).override_failure_message("combo applied in campaign").is_equal(1)
	assert_int(gains).is_greater(200)


## Real keys: held fire (InputEventKey, action tyrian_fire) destroys
## enemies through the game loop (level.gd:_physics_process); each gain
## is the enemy's value (5 to 25 credits at the start of TYRIAN), with no combo
## tier, whereas a chain of 5 destructions would have doubled it. Without the
## helpers (which bring the weapon to power 11): a power-up option
## picked up strengthens the weapon instead of paying the 1,000-credit
## consolation prize (player.c:power_up_weapon), measured identical in all three versions.
func test_held_fire_key_gains_are_unmultiplied() -> void:
	Game.combos_enabled = true
	CampaignBot.replay_assist = false
	var scene := _campaign_level()
	var sim: Node = scene.sim
	scene.set_physics_process(true)
	var ev := InputEventKey.new()
	ev.keycode = KEY_SPACE
	ev.physical_keycode = KEY_SPACE
	ev.pressed = true
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	assert_bool(Input.is_action_pressed(&"tyrian_fire")).is_true()
	var cash := int(sim.player_dict(0)["cash"])
	var gains: Array[int] = []
	var end := Time.get_ticks_msec() + 30000
	while gains.size() < 8 and Time.get_ticks_msec() < end:
		await get_tree().physics_frame
		var c := int(sim.player_dict(0)["cash"])
		if c != cash:
			gains.append(c - cash)
			cash = c
	ev.pressed = false
	Input.parse_input_event(ev)
	Input.flush_buffered_events()
	assert_int(gains.size()).is_greater_equal(8)
	assert_int(sim.combo_multiplier()).is_equal(1)
	for g in gains:
		# A wave destroyed at once pays the sum of its values (4 × 5).
		assert_int(g).is_between(5, 4 * 25)
