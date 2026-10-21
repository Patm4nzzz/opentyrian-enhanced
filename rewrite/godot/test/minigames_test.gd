# Campaign mini-games: "** ALE **" (episode 1, section 41) and
# "SQUADRON" (episode 4, section 45), offered by the shop "Do you have
# the special ship?" (]s[ ]b[ ]x[ ... ]I[). Entry through the shop's
# map, ENGAGE + GALAGA mode set in the level scene, game played
# with the C golden masters' inputs until game over, then back to the
# shop with the saved equipment and money (tyrian2.c: JE_loadGame(11)).
extends GdUnitTestSuite

const SetupScreen := preload("res://scripts/setup.gd")


func before() -> void:
	# The replayed inputs are those of a C golden master, recorded
	# with hitstop disabled (golden_master.c:golden_master_level_start): the
	# enemy freeze would shift the game and it would no longer end on the same tick.
	# The removal of the debriefing's endless loop is verified separately
	# (debrief_exit_test.gd).
	SetupScreen.settings["hitstop_enabled"] = false
	SetupScreen.apply_settings()
	EndlessSession.stop()
	Game.modes = PackedInt32Array()
	Game.modes_extra = []


func after() -> void:
	SetupScreen.reset_settings()
	SetupScreen.apply_settings()
	Game.campaign_active = false
	Game.rng_seed = 1
	Game.mini_game = false
	Game.extra_game = false
	Game.campaign_backup = {}
	Game.modes = PackedInt32Array()
	Game.modes_extra = []
	Game.debrief = {}
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


## Replaced scenes (shop, level) freed before gdUnit4 counts
## orphan nodes.
func after_test() -> void:
	for child in Effects.game_viewport.get_children():
		child.queue_free()
		Effects.game_viewport.remove_child(child)
	await get_tree().process_frame
	await get_tree().process_frame


func _check(cond: bool, what: String) -> void:
	assert_bool(cond).override_failure_message(what).is_true()


## Input masks of the C golden master (column 2, hexadecimal).
func _masks(golden: String) -> PackedInt32Array:
	var path := ProjectSettings.globalize_path("res://").path_join("../golden").path_join(golden)
	var out := PackedInt32Array()
	for line in FileAccess.get_file_as_string(path).split("\n"):
		if line.is_empty() or not line[0].is_valid_int():
			continue
		var f := line.split(" ", false)
		if f.size() >= 18:
			out.append(f[1].hex_to_int())
	return out


## Golden master header: "difficulty N seed S" (level seed).
func _header(golden: String) -> Dictionary:
	var path := ProjectSettings.globalize_path("res://").path_join("../golden").path_join(golden)
	for line in FileAccess.get_file_as_string(path).split("\n"):
		if line.begins_with("episode "):
			var w := line.split(" ", false)
			return {"difficulty": int(w[w.find("difficulty") + 1]), "seed": int(w[w.find("seed") + 1])}
	return {}


func _play(episode: int, shop: int, mini: int, other: int, name: String, golden: String) -> void:
	# Game in progress arrived at the shop: campaign equipment, money.
	# Golden master difficulty and seed, to replay the C game.
	var h := _header(golden)
	var campaign_loadout := PackedInt32Array([2, 3, 5, 0, 1, 5, 3, 0, 0, 0, 123456])
	Game.campaign_active = true
	Game.episode = episode
	Game.initial_difficulty = 2
	Game.difficulty = int(h.get("difficulty", 2))
	Game.rng_seed = int(h.get("seed", 1))
	Game.loadout = campaign_loadout.duplicate()
	Game.campaign_carry = PackedInt32Array()
	Game.campaign_backup = {}
	Game.debrief = {"all_players_alive": true}
	Game.main_level = shop
	Game.advance_campaign()
	var scene: Node = Effects.current_game_scene()
	_check(
		scene != null and scene.scene_file_path == Game.SCENE_SHOP and Game.shop_section == shop,
		"%s: section %d → shop (%d)" % [name, shop, Game.shop_section]
	)
	_check(Game.extra_game, "%s: ]x[ → 'Bonus Game' on the map" % name)
	_check(Array(Game.map_sections) == [mini, other], "%s: map %s" % [name, Game.map_sections])
	_check(
		int(Game.campaign_backup.get("main_level", -1)) == shop,
		"%s: ]b[ saves section %d (%s)" % [name, shop, Game.campaign_backup]
	)

	# First destination of the map: the mini-game.
	Game.campaign_leave_shop(0, campaign_loadout)
	var lvl: Node = Effects.current_game_scene()
	_check(lvl != null and lvl.scene_file_path == Game.SCENE_LEVEL, "%s: shop → level" % name)
	_check(
		Game.level_name == name and Game.mini_game,
		"%s: level %s, mini_game=%s" % [name, Game.level_name, Game.mini_game]
	)
	_check(
		Array(Game.loadout).slice(0, 3) == [13, 39, 3] and Game.loadout[10] == 0,
		"%s: ]e[ Stalker 21.126, Atomic RailGun, money 0 (%s)" % [name, Game.loadout]
	)
	if lvl == null or lvl.get("sim") == null:
		return
	var sim: Node = lvl.sim
	var hud: Dictionary = sim.mode_hud()
	_check(
		bool(hud.get("galaga")) and bool(hud.get("one_player_action")) and not bool(hud.get("two_player")),
		"%s: Galaga mode, one player (%s)" % [name, hud]
	)
	_check(lvl.get("assist") == false, "%s: no help in mini-game" % name)

	# Game played with the C inputs until game over.
	lvl.interactive = false
	var masks := _masks(golden)
	lvl.advance(masks.size(), func(i: int) -> int: return masks[i] if i < masks.size() else 0)
	_check(sim.level_ended(), "%s: level end (game over) after %d ticks" % [name, sim.tick()])
	var stats: Dictionary = sim.level_stats()
	_check(not bool(stats.get("all_players_alive", true)), "%s: exit by death" % name)
	stats["cash"] = int(sim.player_dict(0).get("cash", 0))
	stats["carry"] = sim.campaign_carry()
	Game.campaign_level_finished(stats)

	# Back to the shop, without debriefing, with the ]b[ save.
	scene = Effects.current_game_scene()
	_check(
		scene != null and scene.scene_file_path == Game.SCENE_SHOP and Game.shop_section == shop,
		(
			"%s: game over → shop section %d (%s, %d)"
			% [name, shop, scene.scene_file_path if scene else "", Game.shop_section]
		)
	)
	_check(Game.loadout == campaign_loadout, "%s: equipment and money returned (%s)" % [name, Game.loadout])
	_check(
		Game.difficulty == int(h.get("difficulty", 2)) and not Game.mini_game and Game.modes.is_empty(),
		"%s: normal mode restored" % name
	)

	# Second destination ("Skip It"): the normal continuation, outside the mini-game.
	Game.campaign_leave_shop(1, campaign_loadout)
	_check(not Game.mini_game and Game.modes.is_empty(), "%s: normal continuation without Galaga mode" % name)


func test_ale_from_episode1_shop() -> void:
	await _play(1, 40, 41, 26, "** ALE **", "mode-galaga.gm")


func test_squadron_from_episode4_shop() -> void:
	await _play(4, 44, 45, 42, "SQUADRON", "mode-squadron.gm")
