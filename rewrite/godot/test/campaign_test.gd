# Campaign loop.
# New game episode 1 -> shop (section 2) -> TYRIAN level (section 3,
# lvl 9) -> simulated level end -> debriefing -> next level; episode
# transition (section 13 of episode 2, ]Q[); chained replays and interactive
# shop compared with the C; import of the C save (ADR 0129).
extends GdUnitTestSuite


func before() -> void:
	# Independent of suite order: no Endless session or side mode
	# left behind by another suite.
	EndlessSession.stop()
	Game.modes = PackedInt32Array()


func after() -> void:
	Game.campaign_active = false
	Music.release_streams()
	Sfx.release_streams()
	for child in Effects.game_viewport.get_children():
		child.queue_free()


func _check(cond: bool, what: String) -> void:
	assert_bool(cond).override_failure_message(what).is_true()


func _golden(name: String) -> PackedStringArray:
	var dir := ProjectSettings.globalize_path("res://").path_join("../golden/campaign")
	return FileAccess.get_file_as_string(dir.path_join(name)).split("\n")


## Equipment from the C header of a golden master (lines 3 and 4).
func _c_loadout(lines: PackedStringArray) -> PackedInt32Array:
	var camp := lines[3].split(" ", false)
	var it := lines[4].split(" ", false)
	var fr := it[4].split(":")
	var rr := it[6].split(":")
	var sk := it[12].split(":")
	return PackedInt32Array(
		[
			int(it[2]),
			int(fr[0]),
			int(fr[1]),
			int(rr[0]),
			int(rr[1]),
			int(it[8]),
			int(it[10]),
			int(sk[0]),
			int(sk[1]),
			int(it[14]),
			int(camp[2])
		]
	)


## Plays a golden master in TyrianSim; returns [sim, last state line].
func _play(lines: PackedStringArray, loadout: PackedInt32Array, carry: PackedInt32Array) -> Array:
	var hdr := lines[1].split(" ", false)
	var camp := lines[3].split(" ", false)
	var sim: Node = ClassDB.instantiate("TyrianSim")
	sim.set_loadout(loadout)
	sim.set_assist(true)
	sim.set_campaign_state(int(camp[4]), carry)
	sim.load_level(Game.data_dir, int(hdr[1]), int(hdr[7]), int(hdr[9]), int(hdr[11]))
	var last: PackedStringArray = []
	for line in lines:
		if line.is_empty() or line.begins_with("#") or not line[0].is_valid_int():
			continue
		var f := line.split(" ", false)
		if f.size() < 8:
			continue
		sim.step(f[1].hex_to_int(), 0)
		last = f
	return [sim, last]


func _ends_like_c(sim: Node, last: PackedStringArray) -> bool:
	var p: Dictionary = sim.player_dict(0)
	return (
		int(p["x"]) == int(last[2])
		and int(p["y"]) == int(last[3])
		and int(p["cash"]) == int(last[6])
		and sim.level_ended()
	)


func test_campaign_flow() -> void:
	var c: RefCounted = Game.campaign()
	_check(c != null, "TyrianCampaign available")
	if c == null:
		return
	_check(c.episode_available(1) and c.episode_available(2), "levels1.dat and levels2.dat loaded")

	Game.start_new_game()
	_check(
		Game.main_level == 2 and Game.shop_section == 2, "new game → shop section 2 (main_level=%d)" % Game.main_level
	)
	_check(Array(Game.map_sections) == [3], "]G[ → destination section 3 (%s)" % [Game.map_sections])
	# Block 20.5 (ADR 0125): a human game goes through the real shop
	# (shop.tscn scene, purchases through TyrianShop) and plays without autopilot.
	var shop_scene: Node = Effects.current_game_scene()
	_check(
		shop_scene != null and shop_scene.scene_file_path == Game.SCENE_SHOP,
		"new game → interactive shop scene (%s)" % [shop_scene.scene_file_path if shop_scene else ""]
	)
	Game.campaign_leave_shop(0)
	var lvl_scene: Node = Effects.current_game_scene()
	_check(
		lvl_scene != null and lvl_scene.scene_file_path == Game.SCENE_LEVEL and lvl_scene.get("assist") == false,
		(
			"shop → level scene without autopilot assists (%s, assist=%s)"
			% [lvl_scene.scene_file_path if lvl_scene else "", lvl_scene.get("assist") if lvl_scene else null]
		)
	)
	_check(
		Game.main_level == 3 and Game.lvl_file_num == 9 and Game.level_name == "TYRIAN",
		"shop → level TYRIAN lvl 9 (section %d, lvl %d, %s)" % [Game.main_level, Game.lvl_file_num, Game.level_name]
	)
	_check(Game.next_level == 4, "nextLevel = 4 (%d)" % Game.next_level)

	# Level end: summary of e1-chain-1.gm (cash 15267); difficulty 2 -> 3
	# from the C header of e1-chain-2.gm.
	_check(int(c.adjust_difficulty(2, 2, 0)) == 2, "adjust_difficulty never lowers (2)")
	Game.debrief = {"all_players_alive": true}
	Game.next_level = 4
	Game.campaign_active = true
	Game.main_level = Game.next_level
	Game.advance_campaign()
	_check(Game.main_level == 4 and Game.shop_section == 4, "debriefing → shop section 4 (%d)" % Game.main_level)
	_check(Array(Game.map_sections) == [5, 6], "]G[ section 4 → asteroids 5, 6 (%s)" % [Game.map_sections])
	Game.campaign_leave_shop(0)
	_check(
		Game.level_name == "ASTEROID1" and Game.lvl_file_num == 1,
		"→ ASTEROID1 lvl 1 (%s, %d)" % [Game.level_name, Game.lvl_file_num]
	)

	# Episode transition: episode 2, section 13 (end of game, ]Q[).
	Game.episode = 2
	Game.main_level = 13
	Game.advance_campaign()
	# End pages (text, score and tip, "Episode 3"): Space every
	# 0.2 s, like a hurried player (a key during the glow finishes it,
	# another leaves the page).
	var t_end := Time.get_ticks_msec()
	while Time.get_ticks_msec() - t_end < 60000:
		var cur: Node = Effects.current_game_scene()
		if cur == null or cur.scene_file_path != Game.SCENE_STORY:
			break
		var sp := InputEventKey.new()
		sp.physical_keycode = KEY_SPACE
		sp.keycode = KEY_SPACE
		sp.pressed = true
		Input.parse_input_event(sp)
		await get_tree().process_frame
		sp = sp.duplicate()
		sp.pressed = false
		Input.parse_input_event(sp)
		await get_tree().create_timer(0.2).timeout
	_check(Game.episode == 3, "end of episode 2 → episode 3 (%d)" % Game.episode)
	_check(
		Game.level_name == "GAUNTLET" or Game.shop_section >= 1,
		"episode 3: shop or GAUNTLET (section %d)" % Game.main_level
	)

	# Real debriefing: TYRIAN finished, 15267 credits, 2 cubes; a key at
	# the "Press a key" screen leads to the section 4 shop.
	Game.episode = 1
	Game.campaign_active = true
	Game.level_name = "TYRIAN"
	Game.next_level = 4
	Game.debrief = {
		"level_name": "TYRIAN",
		"cash": 15267,
		"enemy_killed": 30,
		"total_enemy": 40,
		"cube_max": 2,
		"all_players_alive": true
	}
	var deb: Node = Game.goto_scene(Game.SCENE_DEBRIEF)
	var t0 := Time.get_ticks_msec()
	while is_instance_valid(deb) and not deb.idle and Time.get_ticks_msec() - t0 < 30000:
		await get_tree().process_frame
	_check(
		is_instance_valid(deb) and deb.idle,
		"debriefing shown up to 'Press a key' (%d ms)" % (Time.get_ticks_msec() - t0)
	)
	if not is_instance_valid(deb):
		return
	var img: Image = deb.vga.image(0)
	var lit := 0
	for y in range(15, 30):
		for x in range(15, 200):
			if img.get_pixel(x, y).v > 0.2:
				lit += 1
	_check(lit > 50, "text 'Completed: TYRIAN' drawn (%d lit pixels)" % lit)
	img.save_png("user://debrief.png")
	var ev := InputEventKey.new()
	ev.physical_keycode = KEY_SPACE
	ev.keycode = KEY_SPACE
	ev.pressed = true
	Input.parse_input_event(ev)
	# Statistics screen (mainint.c, "Level Statistics Screen"): Enter closes it.
	t0 = Time.get_ticks_msec()
	while is_instance_valid(deb) and not deb.stats_shown and Time.get_ticks_msec() - t0 < 10000:
		await get_tree().process_frame
	_check(is_instance_valid(deb) and deb.stats_shown, "statistics screen shown")
	var enter := InputEventKey.new()
	enter.physical_keycode = KEY_ENTER
	enter.keycode = KEY_ENTER
	enter.pressed = true
	Input.parse_input_event(enter)
	Input.flush_buffered_events()
	enter = enter.duplicate()
	enter.pressed = false
	Input.parse_input_event(enter)
	t0 = Time.get_ticks_msec()
	while Game.shop_section != 4 and Time.get_ticks_msec() - t0 < 10000:
		await get_tree().process_frame
	_check(Game.main_level == 4 and Game.shop_section == 4, "key → section 4 (shop) (%d)" % Game.main_level)


## Replays e1-chain-1 then e1-chain-2 in TyrianSim with the campaign
## carry-over (set_campaign_state / campaign_carry): position and money at the
## last tick of each level equal to those of the C.
func test_chained_replay_matches_c() -> void:
	var carry := PackedInt32Array()
	for k in [1, 2]:
		var lines := _golden("e1-chain-%d.gm" % k)
		var r := _play(lines, _c_loadout(lines), carry)
		var sim: Node = r[0]
		var last: PackedStringArray = r[1]
		var p: Dictionary = sim.player_dict(0)
		_check(
			_ends_like_c(sim, last),
			(
				"Godot replay e1-chain-%d: end (%d,%d) cash %d = C (%s,%s) cash %s"
				% [k, p["x"], p["y"], p["cash"], last[2], last[3], last[6]]
			)
		)
		carry = sim.campaign_carry()
		sim.free()


## Interactive shop (TyrianShop): end of e1-shop-1 replayed, then the
## GOLDEN_SHOP keys (selling the cannon before power 11, buying
## Multi-Cannon power 2) must give back the equipment and money from the
## C header of e1-shop-2, and that level, played with this equipment, finish
## like the C.
func test_interactive_shop_matches_c() -> void:
	var carry := PackedInt32Array()
	var shop_loadout := PackedInt32Array()
	for k in [1, 2]:
		var lines := _golden("e1-shop-%d.gm" % k)
		var c_loadout := _c_loadout(lines)
		if k == 2:
			_check(shop_loadout == c_loadout, "Godot shop (TyrianShop): %s = C header %s" % [shop_loadout, c_loadout])
		var r := _play(lines, shop_loadout if k == 2 else c_loadout, carry)
		var sim: Node = r[0]
		var last: PackedStringArray = r[1]
		var p: Dictionary = sim.player_dict(0)
		_check(
			_ends_like_c(sim, last),
			(
				"e1-shop-%d played in Godot: end (%d,%d) cash %d = C (%s,%s) cash %s"
				% [k, p["x"], p["y"], p["cash"], last[2], last[3], last[6]]
			)
		)
		if k == 1:
			var stats: Dictionary = sim.level_stats()
			var script: Dictionary = Game.campaign().run_script(1, 4, 2, 1, true)
			var shop: RefCounted = ClassDB.instantiate("TyrianShop")
			_check(
				shop.open(Game.data_dir, 1, stats["loadout"], script["items_available"], 2),
				"TyrianShop.open (section 4)"
			)
			var names := {"D": "down", "U": "up", "L": "left", "R": "right", "E": "select", "X": "escape"}
			for key in "DDE DE D E R UU E X".split(" ", false):
				for ch in key:
					shop.key(names[ch])
			shop_loadout = shop.loadout()
		carry = sim.campaign_carry()
		sim.free()


## One-time import of a save from the C version (ADR 0129): its
## single-player slots become native files that restart the
## campaign; an existing native slot is not overwritten.
func test_c_save_import() -> void:
	var path := ProjectSettings.globalize_path("res://").path_join(
		"../crates/tyrian-formats/tests/saves/harness181.dat"
	)
	var dir := OS.get_user_data_dir().path_join("import_test")
	DirAccess.make_dir_recursive_absolute(dir)
	for f in DirAccess.get_files_at(dir):
		DirAccess.remove_absolute(dir.path_join(f))
	var n: int = Game.campaign().import_c_save(path, dir)
	_check(n > 0, "C save imported: %d slot(s) (%s)" % [n, Game.campaign().last_error])
	var slots: Array = Game.campaign().load_campaign_slots(dir)
	var used := slots.filter(func(x): return x != null)
	_check(
		used.size() == n and used.all(func(x): return str(x["error"]) == ""), "imported slots read back (%s)" % [used]
	)
	_check(Game.campaign().import_c_save(path, dir) == 0, "second import: no slot overwritten")
	if used.is_empty():
		return
	var s: Dictionary = used[-1]
	Game.load_campaign_slot(s)
	_check(
		Game.campaign_active and Game.episode == int(s["episode"]) and Game.main_level >= int(s["section"]),
		"load → episode %d section %d (%s)" % [Game.episode, Game.main_level, Game.level_name]
	)
	for f in DirAccess.get_files_at(dir):
		DirAccess.remove_absolute(dir.path_join(f))
	DirAccess.remove_absolute(dir)
	Game.campaign_active = false
	# load_campaign_slot changed scene: the old one, removed from the tree,
	# is only freed on the next frame.
	await get_tree().process_frame


## tyrian2.c:JE_main, death in a normal level: no debriefing;
## JE_loadGame(11) ("LAST LEVEL", written before the level) then JE_loadMap
## with the players dead (saveLevel = shop section): back to the
## shop with the equipment and money from before the level, without the pay of the
## lost level.
func test_game_over_returns_to_shop_from_last_level() -> void:
	Game.start_new_game()
	var before := Game.loadout.duplicate()
	Game.campaign_leave_shop(0, before)
	_check(
		Game.level_name == "TYRIAN" and Game.main_level == 3,
		"shop → TYRIAN (%s, %d)" % [Game.level_name, Game.main_level]
	)
	_check(not Game.bonus_level_current, "TYRIAN is not a bonus level")
	var slot = Game.campaign_slots()[Game.AUTOSAVE_SLOT - 1]
	_check(slot != null and str(slot["error"]) == "", "'LAST LEVEL' written before the level (%s)" % [slot])
	var lost := before.duplicate()
	lost[10] = 0
	Game.campaign_level_finished(
		{"all_players_alive": false, "cash": 0, "loadout": lost, "cube_max": 0, "enemy_killed": 3, "total_enemy": 40}
	)
	var scene: Node = Effects.current_game_scene()
	_check(
		scene != null and scene.scene_file_path == Game.SCENE_SHOP,
		"game over → shop, without debriefing (%s)" % [scene.scene_file_path if scene else ""]
	)
	_check(Game.main_level == 2, "shop section (%d)" % Game.main_level)
	_check(
		Game.main_level == 2 and Game.loadout == before,
		"section 2 shop, equipment from before the level (%d, %s)" % [Game.main_level, Game.loadout]
	)
	Game.campaign_active = false
	await get_tree().process_frame


## Pause menu > Quit (tyrian2.c:JE_main, playerEndLevel): JE_loadGame(11)
## then the game menu (shop) of the section before the level, with
## the equipment and money from before the level; the level does not restart.
func test_pause_quit_returns_to_shop_with_pre_level_state() -> void:
	Game.start_new_game()
	var before := Game.loadout.duplicate()
	Game.campaign_leave_shop(0, before.duplicate())
	_check(
		Game.level_name == "TYRIAN" and Game.main_level == 3,
		"shop → TYRIAN (%s, %d)" % [Game.level_name, Game.main_level]
	)
	# Money and equipment changed during the level: exiting loses them.
	Game.loadout[10] = 999999
	Game.campaign_abandon_level()
	await get_tree().process_frame
	var scene: Node = Effects.current_game_scene()
	_check(
		scene != null and scene.scene_file_path == Game.SCENE_SHOP,
		"quit → shop (%s)" % [scene.scene_file_path if scene else ""]
	)
	_check(
		Game.main_level == 2 and Game.loadout == before,
		"section 2 shop, equipment from before the level (%d, %s)" % [Game.main_level, Game.loadout]
	)
	Game.campaign_active = false
	await get_tree().process_frame


## player_collide.c:307-319: a picked-up warp (evalue > 10000) sets bonusLevel
## and nextLevel = evalue - 10000; tyrian2.c:287 "mainLevel = nextLevel" leads
## to the secret level's section, and the summary announces "Warping to secret
## level!" (mainint.c:2204).
func test_secret_warp_leads_to_its_section() -> void:
	Game.start_new_game()
	Game.campaign_leave_shop(0, Game.loadout.duplicate())
	var normal_next := Game.next_level
	var warp := normal_next + 7
	Game.campaign_level_finished(
		{
			"all_players_alive": true,
			"any_player_alive": true,
			"cash": Game.loadout[10],
			"loadout": Game.loadout.duplicate(),
			"cube_max": 0,
			"enemy_killed": 3,
			"total_enemy": 40,
			"bonus_level": true,
			"next_level": warp
		}
	)
	_check(Game.next_level == warp, "next section = warp %d (got %d, script %d)" % [warp, Game.next_level, normal_next])
	_check(bool(Game.debrief.get("bonus_level", false)), "summary: 'Warping to secret level!'")
	await get_tree().process_frame
	Game.campaign_active = false
	await get_tree().process_frame
