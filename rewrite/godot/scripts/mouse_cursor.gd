## Mouse cursor of the C code (mouse.c:JE_mouseStart / JE_mouseStartFilter): sprite
## of shopSpriteSheet (newsh1.shp, 2×2 drawing) chosen by `mouseCursor`,
## drawn only when the mouse is active (TyrianMouse.inactive false).
##
## The C code draws it in VGAScreen just before JE_showVGA then restores the image
## (JE_mouseReplace); here it is a node of the screen's `overlay` layer
## (ADR 0122), never in the 8-bit framebuffer: OTFR captures and the
## interface regression do not see it.
class_name MouseCursor
extends Sprite2D

const SHEET := "newsh1.shp"
## mouse.c:mousePointerSprites: index, x, y, w, h, fx, fy (hot spot).
const POINTERS := [
	[273, 0, 0, 11, 16, 0, 0],
	[275, 0, 0, 21, 16, 10, 8],
	[277, 0, 0, 21, 16, 10, 7],
	[279, 0, 0, 16, 21, 8, 10],
	[281, 8, 0, 16, 21, 7, 10],
]
## mouse.h: MOUSE_POINTER_NORMAL, UP, DOWN, LEFT, RIGHT.
enum { NORMAL, UP, DOWN, LEFT, RIGHT }

## Owned 8-bit screen (null: node-based screen).
var screen: TyrianScreen
## Private VGA for drawing the pointers (assets loaded by TyrianVga).
static var _vga: RefCounted
## Key of the last texture built (pointer, filter, palette).
var _key := ""


func _init(owner_screen: TyrianScreen = null) -> void:
	screen = owner_screen
	centered = false
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	z_index = 100  # above the `overlay` layer (z_index 1)
	visible = false


func _process(_delta: float) -> void:
	# mouse.c: cursor hidden if mouse inactive, out of bounds, or if the screen
	# asks not to display it. is_instance_valid: the screen may be
	# being freed during a scene change.
	var s_valid := screen != null and is_instance_valid(screen)
	if TyrianMouse.inactive or not TyrianMouse.in_range() or not _wanted(s_valid):
		visible = false
		return
	var kind: int = screen.mouse_pointer if s_valid else NORMAL
	var hue: int = screen.mouse_filter_hue if s_valid else 0
	var pal := _palette_bytes()
	var hd := DisplaySettings.hd_sprites() and TyrianAssets.hd_texture("sprites/newsh1.png") != null
	var key := "%d:%d:%d:%d" % [kind, hue, hash(pal), 1 if hd else 0]
	if key != _key:
		_key = key
		var low := _build_image(kind, hue, pal)
		texture = _build_hd(kind, hue, pal, low) if hd else ImageTexture.create_from_image(low)
		# HD mode: ×4 texture displayed at the original size (same hot spot).
		scale = Vector2.ONE / float(DisplaySettings.HD_SCALE if hd else 1)
		texture_filter = CanvasItem.TEXTURE_FILTER_LINEAR if hd else CanvasItem.TEXTURE_FILTER_NEAREST
	visible = true
	var info: Array = POINTERS[kind]
	# mouse.c: x = mouse_x - sprite.x - sprite.fx, y = mouse_y - sprite.y - sprite.fy.
	position = Vector2(
		TyrianMouse.position.x - int(info[1]) - int(info[5]), TyrianMouse.position.y - int(info[2]) - int(info[6])
	)


## Pointer wanted by its screen: `pointer_shown()` of the 8-bit screen, otherwise
## that of the parent node-based screen that defines one.
func _wanted(s_valid: bool) -> bool:
	if s_valid:
		return screen.pointer_shown()
	var host := get_parent()
	return host == null or not host.has_method("pointer_shown") or host.pointer_shown()


## Current hardware palette: that of the 8-bit screen, otherwise that of the VGA
## shared by the screens (image 2 of the menus).
func _palette_bytes() -> PackedByteArray:
	var s_valid := screen != null and is_instance_valid(screen)
	var v: RefCounted = screen.vga if s_valid else TyrianScreen._shared_vga
	if v == null:
		return PackedByteArray()
	return v.palette_bytes()


## 320×200 RGBA8 texture where only the pointer is opaque. The pointer is
## drawn twice on the private VGA (backgrounds 0 then 1): the pixels written by
## the sprite have the same index in both (opaque), the others differ. The
## filter (title_screen.c:JE_mouseStartFilter) replaces the high nibble of the
## index with its own (blit_sprite2x2_filter_clip).
func _build_image(kind: int, hue: int, pal: PackedByteArray) -> Image:
	var info: Array = POINTERS[kind]
	var img := Image.create(320, 200, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	if pal.is_empty() or pal.size() < 768:
		return img
	var v := _dev_vga()
	var marks := []
	for bg in 2:
		v.clear(0, bg)
		v.blit_sprite2(0, 0, 0, SHEET, int(info[0]), 4 if hue == 0 else 5, hue, true)
		marks.append(v.indices(0))
	# blit_sprite2x2: 24×28 block (four 12×14 sprites).
	for y in 28:
		for x in 24:
			var i := y * 320 + x
			if marks[0][i] != marks[1][i]:
				continue
			var c: int = marks[0][i]
			img.set_pixel(x, y, Color8(pal[c * 3], pal[c * 3 + 1], pal[c * 3 + 2]))
	return img


func _build(kind: int, hue: int, pal: PackedByteArray) -> ImageTexture:
	return ImageTexture.create_from_image(_build_image(kind, hue, pal))


## Intentional divergence (the maintainer's decision, 2026-10-10): in HD mode, the pointer
## is drawn with the enlarged newsh1 art (same mechanism as the other
## 2×2 sprites: TyrianAssets.sheet -> SpriteSheet at HD_SCALE).
## Original 24×28 image ×HD_SCALE. The tint filter (high nibble of the index)
## does not apply to a color image: each HD pixel is multiplied by the
## ratio filtered color / unfiltered color of the corresponding original
## pixel (same tint effect, sharp outlines). Without a filter, the HD art
## is displayed as is (colors of palette 5 of the sheet).
func _build_hd(kind: int, hue: int, pal: PackedByteArray, low: Image) -> ImageTexture:
	var k := DisplaySettings.HD_SCALE
	var sheet := _hd_sheet()
	var info: Array = POINTERS[kind]
	var img := Image.create(24 * k, 28 * k, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	if sheet == null:
		return ImageTexture.create_from_image(img)
	var src: Image = sheet.texture.get_image()
	var base := int(info[0])
	var parts := [[base, 0, 0], [base + 1, 12, 0], [base + 19, 0, 14], [base + 20, 12, 14]]
	for part in parts:
		var idx: int = part[0]
		if not sheet.has(idx):
			continue
		var r: Rect2 = sheet.regions[idx]
		var dst: Vector2 = (Vector2(part[1], part[2]) + sheet.offsets[idx]) * k
		img.blend_rect(src, Rect2i(r.position * k, r.size * k), Vector2i(dst))
	if hue != 0 and not pal.is_empty():
		var plain := _build_image(kind, 0, pal)
		for y in 28 * k:
			for x in 24 * k:
				var c := img.get_pixel(x, y)
				if c.a == 0.0:
					continue
				var a := plain.get_pixel(x / k, y / k)
				var b := low.get_pixel(x / k, y / k)
				if a.a == 0.0 or b.a == 0.0:
					continue
				img.set_pixel(
					x,
					y,
					Color(
						clampf(c.r * b.r / maxf(a.r, 0.02), 0.0, 1.0),
						clampf(c.g * b.g / maxf(a.g, 0.02), 0.0, 1.0),
						clampf(c.b * b.b / maxf(a.b, 0.02), 0.0, 1.0),
						c.a
					)
				)
	return ImageTexture.create_from_image(img)


## HD newsh1 sheet rebuilt at each pointer change: the cache
## of TyrianAssets.sheet does not follow the art mode toggle.
static func _hd_sheet() -> SpriteSheet:
	var tex := TyrianAssets.hd_texture("sprites/newsh1.png")
	if tex == null:
		return null
	return SpriteSheet.new(tex, TyrianAssets.read_json("sprites/newsh1.json"), DisplaySettings.HD_SCALE)


static func _dev_vga() -> RefCounted:
	if _vga == null:
		_vga = ClassDB.instantiate("TyrianVga")
	return _vga
