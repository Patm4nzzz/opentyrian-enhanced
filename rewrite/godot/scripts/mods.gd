## Installed mods (autoload `Mods`, ADR 0080).
##
## All reading and validation of mods is done in Rust (TyrianMods,
## crate tyrian-mods): this script only chooses the folders, remembers
## the player's preferences and supplies the scenes with the override
## textures (mod PNGs, cosmetic tint included).
##
## Folders read, from highest to lowest priority (for equal names):
##   1. userdata/mods                — the player's mods, and those created in the editor;
##   2. <repo>/rewrite/mods          — mods shipped with the rewrite.
## A single format (ADR 0083): mods of the C version (<data>/mods) are not
## read; those in the repo are converted into rewrite/mods.
##
## Command line (not remembered): `-- --mods=off` disables everything,
## `-- --mods=a,b` enables ONLY folders a and b (screenshots, tests).
## A golden master replay (capture tool, --replay) forces `off`.
extends Node

static var USER_MODS: String = UserData.path("mods")
static var PREFS_FILE: String = UserData.path("mods.cfg")

## Emitted after a (re)load: scenes re-read textures and items.
signal reloaded

var api: RefCounted
var _prefs := ConfigFile.new()
## "table:index" -> Texture2D (null remembered if the PNG is unreadable)
var _textures := {}
## List imposed by --mods (null = player's preferences).
var _forced: Variant = null
## Sheet byte of the coins, gems and data cubes (spriteSheet11 of tyrian.shp).
const COIN_SHEET := 11
## items.json table of each kind of items().
const ITEM_TABLES := {
	"ship": "ships", "weapon": "weapon_ports", "sidekick": "sidekicks", "shield": "shields", "special": "specials"
}
## "table:id" -> item of items(), for the episode `_item_rows_episode` (-1: to rebuild).
var _item_rows := {}
var _item_rows_episode := -1


func _ready() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(USER_MODS))
	_prefs.load(PREFS_FILE)
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--mods="):
			var v := arg.trim_prefix("--mods=")
			_forced = PackedStringArray() if v == "off" else v.split(",", false)
		elif arg.begins_with("--replay="):
			_forced = PackedStringArray()
	reload()
	# Direct launch of a level with imposed mods (screenshots):
	# starting equipment from the mods, like a new game.
	if _forced != null and not (_forced as PackedStringArray).is_empty():
		Game.loadout = starter_loadout(Game.loadout)


func dirs() -> PackedStringArray:
	return PackedStringArray(
		[
			ProjectSettings.globalize_path(USER_MODS),
			ProjectSettings.globalize_path("res://").path_join("../mods").simplify_path(),
			# Package: the example mods are placed next to the executable (package.sh;
			# macOS: in the bundle resources, GamePaths).
			GamePaths.shipped("mods"),
		]
	)


func available() -> bool:
	return api != null


## Re-reads all folders; to be called when leaving the mods menu.
func reload() -> int:
	_textures.clear()
	_item_rows_episode = -1
	if not ClassDB.class_exists("TyrianMods"):
		push_warning("Mods: GDExtension missing, mods disabled")
		api = null
		return 0
	if api == null:
		api = ClassDB.instantiate("TyrianMods")
	for folder in _prefs.get_section_keys("enabled") if _prefs.has_section("enabled") else []:
		api.set_enabled(folder, bool(_prefs.get_value("enabled", folder)))
	var n: int = api.load(Game.data_dir, dirs())
	if _forced != null:
		for m in api.list(Game.episode):
			api.set_enabled(m["folder"], (_forced as PackedStringArray).has(m["folder"]))
		n = api.reload()
	# Portraits, planets and weapon pictures of the mods: drawn by the 8-bit
	# screens themselves (TyrianVga.blit_sprite).
	api.install_screen_sprites(Game.episode)
	reloaded.emit()
	return n


func set_enabled(folder: String, enabled: bool) -> void:
	_prefs.set_value("enabled", folder, enabled)
	_prefs.save(PREFS_FILE)
	if api != null:
		api.set_enabled(folder, enabled)


func list() -> Array:
	return api.list(Game.episode) if api != null else []


func diagnostics() -> Array:
	return api.diagnostics(Game.episode) if api != null else []


## Hooks the mods onto a TyrianSim node before its load_level.
func attach(sim: Node) -> void:
	if api != null and sim.has_method("set_mods"):
		sim.set_mods(api)


## Equipment of a new game after the `starter` sections.
func starter_loadout(loadout: PackedInt32Array) -> PackedInt32Array:
	return api.starter_loadout(Game.episode, loadout) if api != null else loadout


## shipgraphic of a ship, mods included (-1 if unknown).
func ship_graphic(ship: int) -> int:
	return api.ship_graphic(Game.episode, ship) if api != null else -1


## Items added by the mods (kind, id, name, cost, itemgraphic, always_available...).
func items() -> Array:
	return api.items(Game.episode) if api != null else []


## Item `id` of the items.json table `table` ("ships", "shields"...) when a mod
## adds it, {} otherwise. The mods' items are not in items.json (the game's own
## tables): the screens that draw the armor and shield bars, the ship
## illustration or an item name ask here first.
func item_row(table: String, id: int) -> Dictionary:
	if _item_rows_episode != Game.episode:
		_item_rows_episode = Game.episode
		_item_rows.clear()
		for it in items():
			var t: String = ITEM_TABLES.get(String(it["kind"]), "")
			if not t.is_empty():
				_item_rows["%s:%d" % [t, int(it["id"])]] = it
	return _item_rows.get("%s:%d" % [table, id], {})


## True if an enabled mod provides at least one image for `table` (shots, ships...).
func has_textures(table: String) -> bool:
	_build_table_cache(table)
	return bool(_textures.get("%s:any" % table, false))


## Override texture for `table` (shots, ships, shop_weapons...) at `index`,
## or null. The PNG has already been validated (header, size) by the Rust code.
func texture(table: String, index: int) -> Texture2D:
	var key := "%s:%d" % [table, index]
	if _textures.has(key):
		return _textures[key]
	_build_table_cache(table)
	return _textures.get(key, null)


## Indices of `table` for which an enabled mod provides an image.
func texture_indices(table: String) -> PackedInt32Array:
	_build_table_cache(table)
	var out := PackedInt32Array()
	var prefix := "%s:" % table
	for key: String in _textures:
		var index := key.trim_prefix(prefix)
		if key.begins_with(prefix) and index.is_valid_int() and _textures[key] != null:
			out.append(int(index))
	return out


func _build_table_cache(table: String) -> void:
	var marker := "%s:built" % table
	if _textures.has(marker) or api == null:
		return
	_textures[marker] = null
	for s in api.sprites(Game.episode):
		if String(s["table"]) != table:
			continue
		var img := Image.load_from_file(String(s["path"]))
		if img == null or img.is_empty():
			continue
		if float(s["hue"]) >= 0.0:
			_tint(img, float(s["hue"]))
		var tex := ImageTexture.create_from_image(img)
		var index := int(s["index"])
		_textures["%s:any" % table] = true
		if table == "enemies":
			# One image per (sheet, index): the first image of the named enemy.
			_textures["enemies:%d:%d" % [int(s.get("sheet", 0)), index]] = tex
		elif table == "coins":
			_store_coin_frames(tex, index)
		else:
			_textures["%s:%d" % [table, index]] = tex


## A coin PNG is the strip of its animation, like the templates of
## data/reference/sprites/coins: frames of 12x14 (a coin of one sheet cell,
## frame k is image index + k) or of 24x28 (a coin of 2x2 cells, frame k is
## image index + 2k). Any other size is taken as a single image.
func _store_coin_frames(tex: Texture2D, index: int) -> void:
	var size := Vector2i(tex.get_size())
	var frame := Vector2i(12, 14) if size.y == 14 else Vector2i(24, 28)
	if size.y != frame.y or size.x % frame.x != 0 or size.x == frame.x:
		_textures["coins:%d" % index] = tex
		return
	@warning_ignore("integer_division")
	for k in size.x / frame.x:
		var part := AtlasTexture.new()
		part.atlas = tex
		part.region = Rect2(k * frame.x, 0, frame.x, frame.y)
		@warning_ignore("integer_division")
		_textures["coins:%d" % (index + k * (frame.x / 12))] = part


## Image a mod gives to image `index` of the enemy sheet `sheet` (sheet byte of
## TyrianSim.hd_enemy_sprites), or null: a coin of sheet 11 (tyrian.shp, "coins"
## table) or the first image of an original enemy ("enemies" table).
func enemy_texture(sheet: int, index: int) -> Texture2D:
	if sheet == COIN_SHEET:
		var coin := texture("coins", index)
		if coin != null:
			return coin
	_build_table_cache("enemies")
	return _textures.get("enemies:%d:%d" % [sheet, index], null)


## Cosmetic tint (sprite_override.c:apply_tint): replaces the hue of the
## colored pixels, leaves the grays (outlines) intact.
static func _tint(img: Image, hue: float) -> void:
	img.convert(Image.FORMAT_RGBA8)
	for y in img.get_height():
		for x in img.get_width():
			var c := img.get_pixel(x, y)
			if c.a == 0.0 or c.s < 0.05:
				continue
			img.set_pixel(x, y, Color.from_hsv(hue, minf(c.s * 1.2, 1.0), c.v, c.a))


## --- Installing a mod from a .zip (Mods menu) --------------------------------

## Folders added by macOS's archiver, unrelated to the mod.
const ZIP_IGNORED_TOP := "__MACOSX"


## Installs the mod contained in archive `zip_path` into `dest_root/<name>`.
## The mod.json must be at the root of the archive (name = name of the .zip) or
## in a single root folder (name = that folder). Refused, without writing anything:
## unreadable archive, path that would escape the mod folder ("..", absolute path,
## Windows drive, backslash), missing mod.json, mod folder
## already present (no replacement: the player deletes the old one
## with "Open the mods folder").
## Returns {"error": locale key or "", "args": [...], "folder": name}.
static func install_zip(zip_path: String, dest_root: String) -> Dictionary:
	var zip := ZIPReader.new()
	if zip.open(zip_path) != OK:
		return _install_result("gd_mods_zip_unreadable", [zip_path.get_file()])
	var files := PackedStringArray()
	for f in zip.get_files():
		if f.get_slice("/", 0) != ZIP_IGNORED_TOP:
			files.append(f)
	for f in files:
		if not is_safe_zip_path(f):
			zip.close()
			return _install_result("gd_mods_zip_unsafe", [f])
	var prefix := ""
	var folder := zip_path.get_file().get_basename()
	if not files.has("mod.json"):
		var tops := {}
		for f in files:
			tops[f.get_slice("/", 0)] = true
		var top: String = tops.keys()[0] if tops.size() == 1 else ""
		if top.is_empty() or not files.has(top + "/mod.json"):
			zip.close()
			return _install_result("gd_mods_zip_no_json", [])
		prefix = top + "/"
		folder = top
	if folder.is_empty() or not folder.is_valid_filename():
		zip.close()
		return _install_result("gd_mods_zip_unsafe", [folder])
	var dest := dest_root.path_join(folder)
	if DirAccess.dir_exists_absolute(dest) or FileAccess.file_exists(dest):
		zip.close()
		return _install_result("gd_mods_zip_exists", [folder])
	DirAccess.make_dir_recursive_absolute(dest)
	for f in files:
		var rel := f.substr(prefix.length())
		if rel.is_empty() or rel.ends_with("/"):
			DirAccess.make_dir_recursive_absolute(dest.path_join(rel))
			continue
		var path := dest.path_join(rel)
		DirAccess.make_dir_recursive_absolute(path.get_base_dir())
		var out := FileAccess.open(path, FileAccess.WRITE)
		if out == null:
			zip.close()
			_remove_tree(dest)
			return _install_result("gd_mods_zip_write", [rel])
		out.store_buffer(zip.read_file(f))
		out.close()
	zip.close()
	var r := _install_result("", [])
	r["folder"] = folder
	return r


## Archive entry path that stays inside the mod folder: relative, without
## "..", without Windows drive (C:), without backslash (separator
## on Windows, hence a way to slip in "..\").
static func is_safe_zip_path(f: String) -> bool:
	if f.is_empty() or f.begins_with("/") or f.contains("\\") or f.contains(":"):
		return false
	for part in f.split("/"):
		if part == "..":
			return false
	return true


static func _install_result(error: String, args: Array) -> Dictionary:
	return {"error": error, "args": args, "folder": ""}


## Deletes a folder and its contents (interrupted installation).
static func _remove_tree(dir: String) -> void:
	for sub in DirAccess.get_directories_at(dir):
		_remove_tree(dir.path_join(sub))
	for f in DirAccess.get_files_at(dir):
		DirAccess.remove_absolute(dir.path_join(f))
	DirAccess.remove_absolute(dir)
