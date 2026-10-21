## State readout for the player journeys (autoload `JourneyState`, inactive
## without an argument).
##
##   OpenTyrian2000-Enhanced.x86_64 -- --journey-state=<file.json>
##
## Writes four times per second, into <file.json>, the state the player
## should see change when acting: current scene, chosen row and labels
## of the open menu, shop (selected item, owned item, money),
## campaign equipment and money, level (pause, pause menu, Quit
## box, armor, money), Setup and display settings, Endless.
## rewrite/scripts/player_journeys.py reads this file to check the EFFECT of
## each action (money debited, weapon mounted, option changed and kept),
## not just the image. Read-only: nothing is modified in the game.
extends Node

const PERIOD_S := 0.25

var _path := ""
var _clock := 0.0


func _ready() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--journey-state="):
			_path = arg.trim_prefix("--journey-state=")
	set_process(not _path.is_empty())
	process_mode = Node.PROCESS_MODE_ALWAYS


func _process(delta: float) -> void:
	_clock += delta
	if _clock < PERIOD_S:
		return
	_clock = 0.0
	var tmp := _path + ".tmp"
	var f := FileAccess.open(tmp, FileAccess.WRITE)
	if f == null:
		return
	f.store_string(JSON.stringify(snapshot()))
	f.close()
	DirAccess.rename_absolute(tmp, _path)


## Full state (JSON dictionary).
func snapshot() -> Dictionary:
	var d := {"frame": Engine.get_process_frames(), "msec": Time.get_ticks_msec()}
	var scene: Node = Effects.current_game_scene() if Effects.has_method("current_game_scene") else null
	d["scene"] = scene.scene_file_path if scene != null else ""
	d["scene_class"] = (
		scene.get_script().resource_path.get_file() if scene != null and scene.get_script() != null else ""
	)
	# Point (0, 0) of the 320x200 game and size of one game pixel, in
	# window pixels: the driver aims clicks with it, whatever the window.
	var screen_rect: Variant = Effects.get("_screen")
	if screen_rect != null:
		var xf: Transform2D = get_window().get_final_transform() * (screen_rect as Control).get_global_transform()
		var k: Vector2 = xf.get_scale() * (screen_rect as Control).size / Vector2(320, 200)
		d["window"] = {
			"size": [get_window().size.x, get_window().size.y],
			"origin": [xf.origin.x, xf.origin.y],
			"pixel": [k.x, k.y],
			"mode": get_window().mode
		}
	d["game"] = _game()
	d["settings"] = _settings()
	if scene != null:
		d["menu"] = _menu(scene)
		if "_shop" in scene:
			d["shop"] = _shop(scene)
		if "sim" in scene:
			d["level"] = _level(scene)
		for key in [
			"idle",
			"_specs_open",
			"modal_active",
			"_arcade_menu",
			"_arcade_sel",
			"selected",
			"_typing",
			"_saving",
			"_message"
		]:
			if key in scene:
				var v: Variant = scene.get(key)
				if v is bool or v is int or v is String:
					d.get_or_add("screen", {})[key] = v
		if "_help" in scene:
			d.get_or_add("screen", {})["help_open"] = scene.get("_help") != null
		if "_arcade_menu" in scene and int(scene.get("_arcade_menu")) != 0 and not scene.get("menus").stack.is_empty():
			d["arcade_hits"] = _arcade_hits(scene)
	d["christmas"] = Christmas.active
	d["endless"] = {"active": EndlessSession.active(), "final_score": EndlessSession.final_score}
	if EndlessSession.active():
		d["endless"]["score"] = EndlessSession.score()
		d["endless"]["lives"] = EndlessSession.lives()
	return d


func _game() -> Dictionary:
	var lo: PackedInt32Array = Game.loadout
	return {
		"campaign_active": Game.campaign_active,
		"episode": Game.episode,
		"difficulty": Game.difficulty,
		"initial_difficulty": Game.initial_difficulty,
		"loadout": Array(lo),
		"cash": int(lo[10]) if lo.size() > 10 else 0,
		"shop_section": Game.shop_section,
		"main_level": Game.main_level,
		"level_name": Game.level_name,
		"last_level_name": Game.last_level_name,
		"modes": Array(Game.modes),
		"two_players": Game.two_player_game(),
	}


func _settings() -> Dictionary:
	var setup: Dictionary = preload("res://scripts/setup.gd").settings.duplicate()
	setup["display_art_mode"] = int(DisplaySettings.art_mode())
	setup["display_crt_mode"] = DisplaySettings.crt_mode()
	setup["display_scaler"] = DisplaySettings.scaler()
	setup["display_scaling_mode"] = DisplaySettings.scaling_mode()
	setup["display_gamma"] = DisplaySettings.gamma()
	setup["physics_ticks_per_second"] = Engine.physics_ticks_per_second
	setup["music_user_volume"] = Music.user_volume
	setup["music_is_muted"] = Music.muted
	setup["sfx_is_enabled"] = Sfx.enabled
	setup["music_player_db"] = Music._player.volume_db if Music._player != null else 0.0
	return setup


## Common engine menu open on screen (or on the pause menu).
func _menu(node: Node) -> Dictionary:
	if not "menus" in node:
		return {}
	var engine: Variant = node.get("menus")
	if engine == null or not "stack" in engine or engine.stack.is_empty():
		return {}
	var m: Dictionary = engine.stack.back()
	var labels: Array = []
	for it in m.items:
		labels.append(str(it.get("label", "")))
	return {
		"depth": engine.stack.size(),
		"header": str(m.get("header", "")),
		"sel": int(m.sel),
		"labels": labels,
		"popup": engine.popup_open(),
		"top": int(m.get("top", 0)),
		"hits": _menu_hits(engine, m),
		"popup_hits": _popup_hits(engine)
	}


## Center (320x200 coordinates) of each choice of the open dropdown list
## (menu_tree.c:picker_popup: 13 px rows every 15 px), selection included.
func _popup_hits(engine: Variant) -> Dictionary:
	if not engine.popup_open():
		return {}
	var p: Dictionary = engine.get("_popup")
	var pts := {}
	for i in int(p.it.count):
		pts[str(i)] = [int(p.x) + int(p.w) / 2, int(p.y) + 2 + i * 15 + 6]
	return {"sel": int(p.sel), "hits": pts}


## Clickable point (center, 320x200 coordinates) of each row of menu `m`,
## computed by the same function as the player's click (MenuEngine.row_at).
## Kept as long as the menu and its window do not change.
var _hits_key := ""
var _hits := {}


func _menu_hits(engine: Variant, m: Dictionary) -> Dictionary:
	if m.options.get("external", false):
		return {}
	var key := "%d:%d:%d:%d" % [engine.get_instance_id(), engine.stack.size(), m.items.size(), int(m.get("top", 0))]
	if key == _hits_key:
		return _hits
	var pts := {}
	for y in range(0, 200, 1):
		for x in range(2, 318, 4):
			var r: int = engine.row_at(m, Vector2i(x, y))
			if r >= 0:
				pts.get_or_add(str(r), []).append(Vector2i(x, y))
	_hits = {}
	for r in pts:
		var list: Array = pts[r]
		var ys: Array = list.map(func(p: Vector2i) -> int: return p.y)
		ys.sort()
		var my: int = ys[ys.size() / 2]
		var xs: Array = (
			list.filter(func(p: Vector2i) -> bool: return p.y == my).map(func(p: Vector2i) -> int: return p.x)
		)
		_hits[r] = [xs[xs.size() / 2], my]
	_hits_key = key
	return _hits


## Arcade Game Menu rows under the mouse (same computation as shop.gd:_arcade_mouse).
func _arcade_hits(scene: Node) -> Dictionary:
	var engine: Variant = scene.get("menus")
	var m: Dictionary = engine.current()
	var pts := {}
	for y in range(0, 200):
		var p := Vector2i(240, y)
		var row: int = engine.row_at(m, p)
		if int(scene.get("_arcade_menu")) == 9:
			row = scene.call("_arcade_row_2p_at", p, m.items.size() + 1)
		if row >= 0:
			pts.get_or_add(str(row + 2), []).append(y)
	var out := {}
	for r in pts:
		out[r] = [240, pts[r][pts[r].size() / 2]]
	return out


## Shop rows under the mouse (TyrianShop.row_under_mouse, like the click).
func _shop_hits(scene: Node, shop: Variant) -> Dictionary:
	var scroll: int = int(scene.get("_shop_scroll"))
	var pts := {}
	for y in range(0, 200, 1):
		var r: int = shop.row_under_mouse(240, y, scroll)
		if r >= 0:
			pts.get_or_add(str(r), []).append(y)
	var out := {}
	for r in pts:
		var ys: Array = pts[r]
		out[r] = [240, ys[ys.size() / 2]]
	return out


func _shop(scene: Node) -> Dictionary:
	var shop: Variant = scene.get("_shop")
	if shop == null:
		return {"open": false}
	var s: Dictionary = shop.state()
	var out := {"open": true}
	for k in [
		"menu",
		"sel",
		"category",
		"cash",
		"cash_left",
		"owned_item",
		"chosen_item",
		"left_power",
		"right_power",
		"cube_sel",
		"cube_max"
	]:
		if s.has(k):
			out[k] = s[k]
	out["items"] = Array(s.get("items", PackedInt32Array()))
	out["costs"] = Array(s.get("costs", PackedInt32Array()))
	out["hover_loadout"] = Array(s.get("loadout", PackedInt32Array()))
	out["committed_loadout"] = Array(shop.loadout())
	out["owned_value"] = s.get("owned_value", 0)
	out["menu_choices"] = s.get("menu_choices", 0)
	out["scroll"] = scene.get("_shop_scroll")
	if int(s.get("menu", 0)) == 8:
		out["cube_y"] = scene.get("_cube_y")
		out["cube_read"] = scene.cube_read_percent(shop.cube_text(int(s.get("cube_sel", 2)) - 2).size())
	out["marked_rows"] = _marked_rows(scene) if int(s.get("menu", 0)) == 4 else []
	out["hits"] = _shop_hits(scene, shop)
	return out


## Rows of a category list that carry, ON SCREEN, the mark of the
## equipped item (color bar 227 under the row, shop.gd
## _draw_category_row): read from the 8-bit image, not from the state, to
## check what the player sees. Ranks 0..4 of the visible window.
func _marked_rows(scene: Node) -> Array:
	var vga: Variant = scene.get("vga")
	if vga == null:
		return []
	var px: PackedByteArray = vga.indices(TyrianScreen.VGA_SCREEN)
	if px.size() < 320 * 200:
		return []
	var rows: Array = []
	for i in 5:
		var y := 40 + i * 26 + 8
		var n := 0
		for x in range(262, 296):
			if px[y * 320 + x] == 227:
				n += 1
		if n >= 24:
			rows.append(i)
	return rows


func _level(scene: Node) -> Dictionary:
	var sim: Variant = scene.get("sim")
	var d := {
		"paused": bool(scene.get("paused")),
		"pause_menu": scene.get("_pause_menu") != null,
		"quit_box": scene.get("_quit_box") != null,
		"help": scene.get("_help_screen") != null,
		"banner": str(scene.call("_hud_banner_text")) if scene.has_method("_hud_banner_text") else ""
	}
	var pm: Variant = scene.get("_pause_menu")
	if pm != null:
		d["pause_menu_state"] = _menu(pm)
		d["pause_marker_px"] = _pause_marker_pixels(pm)
	var hk: Variant = scene.get("hotkeys")
	if hk != null:
		d["overlays"] = {
			"stats": hk.overlays.stats_active,
			"achievements": hk.overlays.achievements_active,
			"dps": hk.overlays.dps_active
		}
		d["quick_load_pending"] = hk.quick_load_pending
	if sim != null:
		var p: Dictionary = sim.player_dict(0)
		d["armor"] = p.get("armor", 0)
		d["x"] = p.get("x", 0)
		d["alive"] = p.get("alive", false)
		d["cash"] = p.get("cash", 0)
		d["ended"] = sim.level_ended()
	d["recording"] = sim != null and sim.has_method("is_recording") and sim.is_recording()
	d["recording_armed"] = ReplayRecording.armed
	return d


## Pixels of the pause menu's chosen-row marker, ON SCREEN:
## pixels of the presented image that differ from the frozen background (VGAScreen2) in
## the marker column (x 0..PAUSE_X+3, height of the chosen row).
func _pause_marker_pixels(pm: Node) -> int:
	var vga: Variant = pm.get("vga")
	if vga == null or pm.get("menus").stack.is_empty():
		return -1
	var sel: int = pm.get("menus").stack.back().sel
	var a: PackedByteArray = vga.indices(TyrianScreen.VGA_SCREEN)
	var b: PackedByteArray = vga.indices(TyrianScreen.VGA_SCREEN2)
	if a.size() < 64000 or b.size() < 64000:
		return -1
	var y0: int = pm.PAUSE_Y_START + sel * pm.PAUSE_STEP - 2
	var n := 0
	for y in range(maxi(y0, 0), mini(y0 + 14, 200)):
		for x in range(0, pm.PAUSE_X + 3):
			if a[y * 320 + x] != b[y * 320 + x]:
				n += 1
	return n
