## Access to the resources produced by `tyrian-import` (assets/imported/).
##
## PNGs are loaded through the Godot import when it exists (editor, build),
## otherwise directly from the file (first launch before import).
class_name TyrianAssets
extends RefCounted

const IMPORTED := "res://assets/imported"

static var _json_cache := {}
static var _texture_cache := {}
static var _sheet_cache := {}


static func read_json(rel_path: String) -> Variant:
	if _json_cache.has(rel_path):
		return _json_cache[rel_path]
	var path := IMPORTED.path_join(rel_path)
	var text := FileAccess.get_file_as_string(path)
	if text.is_empty():
		push_error("TyrianAssets : file not found or empty %s" % path)
		return null
	var value: Variant = JSON.parse_string(text)
	_json_cache[rel_path] = value
	return value


## Reads a JSON without caching it (levels: large files read once).
static func read_json_uncached(rel_path: String) -> Variant:
	var text := FileAccess.get_file_as_string(IMPORTED.path_join(rel_path))
	if text.is_empty():
		return null
	return JSON.parse_string(text)


static func texture(rel_path: String) -> Texture2D:
	if _texture_cache.has(rel_path):
		return _texture_cache[rel_path]
	var path := IMPORTED.path_join(rel_path)
	var tex: Texture2D = null
	if ResourceLoader.exists(path):
		tex = load(path)
	else:
		var img := Image.load_from_file(ProjectSettings.globalize_path(path))
		if img != null:
			tex = ImageTexture.create_from_image(img)
	if tex == null:
		push_error("TyrianAssets : texture not found %s" % path)
	_texture_cache[rel_path] = tex
	return tex


## Sprite atlas upscaled ×4 (`upscaled/<rel_path>`, scripts/hd_upscale.py) if the
## "HD sprites" mode is active and the file is present, otherwise null (ADR 0100).
## Loaded directly from the PNG (folder marked .gdignore, outside the .pck).
static func hd_texture(rel_path: String) -> Texture2D:
	if not DisplaySettings.hd_sprites():
		return null
	var key := "hd:" + rel_path
	if _texture_cache.has(key):
		return _texture_cache[key]
	var path := DisplaySettings.UPSCALED.path_join(rel_path)
	var tex: Texture2D = null
	if FileAccess.file_exists(path):
		var img := Image.load_from_file(path)
		if img != null:
			tex = ImageTexture.create_from_image(img)
	_texture_cache[key] = tex
	return tex


## Clears the sheet caches (toggling the "HD sprites" mode).
static func clear_sheet_cache() -> void:
	_sheet_cache.clear()


static func pic(number: int) -> Texture2D:
	return texture("pics/pic_%02d.png" % number)


## Sprite sheet (`sprites/<name>.json` + PNG), null if absent.
static func sheet(name: String) -> SpriteSheet:
	# Christmas mode: shots, ships and bonuses come from tyrianc.shp, with no HD image
	# (the C then ignores the HD overrides of these sheets).
	if Christmas.active:
		name = Christmas.SHEETS.get(name, name)
	if _sheet_cache.has(name):
		return _sheet_cache[name]
	var result: SpriteSheet = null
	if FileAccess.file_exists(IMPORTED.path_join("sprites/%s.json" % name)):
		var meta: Dictionary = read_json("sprites/%s.json" % name)
		var image_rel := "sprites/" + String(meta["image"])
		var hd := hd_texture(image_rel)
		if hd != null:
			result = SpriteSheet.new(hd, meta, DisplaySettings.HD_SCALE)
		else:
			result = SpriteSheet.new(texture(image_rel), meta)
	_sheet_cache[name] = result
	return result


## Name of the `newsh?` sheet for the `sheet` byte of EnemyView
## (same encoding as `safe_name` in tyrian-import: `#` -> `newshx23`).
## Bytes 10 and 11: spriteSheet10 / spriteSheet11 of tyrian.shp (bonuses,
## coins and datacubes), like playfield.rs:enemy_sheet_name.
static func enemy_sheet_name(sheet_byte: int) -> String:
	if sheet_byte == 10:
		return "main_09_powerups"
	if sheet_byte == 11:
		return "main_10_coins_datacubes"
	var c := String.chr(sheet_byte).to_lower()
	var is_alnum := (sheet_byte >= 48 and sheet_byte <= 57) or (c >= "a" and c <= "z")
	if is_alnum:
		return "newsh" + c
	return "newshx%02x" % sheet_byte
