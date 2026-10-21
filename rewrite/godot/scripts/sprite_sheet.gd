## Imported sprite sheet: atlas texture + image rectangles.
##
## `index` follows the C numbering: 0-based for simple tables
## (`blit_sprite`), 1-based for Sprite2 sheets (`blit_sprite2`).
class_name SpriteSheet
extends RefCounted

var texture: Texture2D
## Ratio of `texture` size / original atlas size: 1, or 4 in
## "HD sprites" mode (Real-ESRGAN atlas, ADR 0100). `regions` and `offsets` always stay
## in original pixels: only the sampling of the texture changes.
var scale := 1
var is_sprite2: bool
## index -> Rect2 in the atlas
var regions := {}
## index -> Vector2: offset of the top-left corner relative to the C drawing point
var offsets := {}


func _init(tex: Texture2D, meta: Dictionary, tex_scale := 1) -> void:
	texture = tex
	scale = tex_scale
	is_sprite2 = meta.get("format", "") == "sprite2"
	for f in meta["frames"]:
		var i := int(f["index"])
		regions[i] = Rect2(f["x"], f["y"], f["w"], f["h"])
		offsets[i] = Vector2(f["offset_x"], f["offset_y"])


func has(index: int) -> bool:
	return regions.has(index)


func width(index: int) -> int:
	return int(regions[index].size.x) if regions.has(index) else 0


## Configures a Sprite2D to display image `index` at the C drawing point (x, y).
func apply(sprite: Sprite2D, index: int, x: float, y: float) -> bool:
	if not regions.has(index):
		sprite.visible = false
		return false
	sprite.texture = texture
	sprite.centered = false
	sprite.region_enabled = true
	sprite.region_rect = _source_rect(regions[index])
	sprite.scale = Vector2.ONE / scale
	sprite.position = Vector2(x, y) + offsets[index]
	sprite.visible = true
	return true


## Draws image `index` on a CanvasItem (to be called from `_draw`).
func draw(canvas: CanvasItem, index: int, x: float, y: float, modulate := Color.WHITE) -> void:
	if not regions.has(index):
		return
	var r: Rect2 = regions[index]
	canvas.draw_texture_rect_region(texture, Rect2(Vector2(x, y) + offsets[index], r.size), _source_rect(r), modulate)


## Original rectangle -> rectangle in `texture` (×scale in HD mode).
func _source_rect(r: Rect2) -> Rect2:
	return r if scale == 1 else Rect2(r.position * scale, r.size * scale)


## Equivalent of `blit_sprite2x2` (sprite.c): 4 images index, +1, +19, +20.
func draw_2x2(canvas: CanvasItem, index: int, x: float, y: float, modulate := Color.WHITE) -> void:
	draw(canvas, index, x, y, modulate)
	draw(canvas, index + 1, x + 12, y, modulate)
	draw(canvas, index + 19, x, y + 14, modulate)
	draw(canvas, index + 20, x + 12, y + 14, modulate)
