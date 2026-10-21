## HD (AI) mode interface: displays a 320×200 `TyrianVga` image (or a
## transparent play-area layer) at 1280×800, original images upscaled
## by the model (backgrounds, frames, portraits, icons: TyrianVga.hd_image) and
## texts sharply upscaled by xBRZ (shaders/hd_interface.gdshader). Drawing
## into the VGA does not change: pixel parity and the OTFR captures remain
## those of Original mode.
##
##   var hd := HdInterface.new(sprite, vga)   # vga null: xBRZ only (layer)
##   hd.update(surf, img)                     # after each texture update
class_name HdInterface
extends RefCounted

const SHADER := preload("res://shaders/hd_interface.gdshader")

var _sprite: Sprite2D
var _vga: RefCounted
var _material: ShaderMaterial
var _hd_texture: ImageTexture
var _rgb_texture: ImageTexture
var _alpha_texture: ImageTexture
## Composed zone and excluded zone (original pixels), see TyrianVga.hd_image.
var keep := Rect2i(0, 0, 320, 200)
var skip := Rect2i()


func _init(sprite: Sprite2D, vga: RefCounted) -> void:
	_sprite = sprite
	_vga = vga
	_material = ShaderMaterial.new()
	_material.shader = SHADER
	if vga != null and active():
		prepare(vga)


## HD mode active and extension able to render it.
static func active() -> bool:
	return DisplaySettings.hd_sprites() and ClassDB.class_has_method("TyrianVga", "hd_image")


## Provenance tracking of a VGA, to enable before its drawing.
static func prepare(vga: RefCounted) -> void:
	ClassDB.class_call_static("TyrianVga", "hd_set_dirs", TyrianAssets.IMPORTED, DisplaySettings.UPSCALED)
	vga.hd_enable_trace()


## To call after each update of the sprite texture. `img`: image
## placed in the texture, if it has transparent pixels (layers).
func update(surf: int, img: Image = null) -> void:
	if not active():
		if _sprite.material == _material:
			_sprite.material = null
		return
	_sprite.material = _material
	var size := Vector2(_sprite.texture.get_size()) if _sprite.texture != null else Vector2(320, 200)
	_material.set_shader_parameter("source_size", size)
	if _vga != null:
		prepare(_vga)
		var hd_img: Image = _vga.hd_image(surf, keep, skip)
		if hd_img != null:
			if _hd_texture == null:
				_hd_texture = ImageTexture.create_from_image(hd_img)
				_material.set_shader_parameter("hd_tex", _hd_texture)
			else:
				_hd_texture.update(hd_img)
	_material.set_shader_parameter("hd_enabled", _hd_texture != null)
	if img != null:
		var src: Array = ClassDB.class_call_static("TyrianVga", "hd_xbrz_sources", img)
		_rgb_texture = _set_texture(_rgb_texture, src[0], "source_tex")
		_alpha_texture = _set_texture(_alpha_texture, src[1], "alpha_tex")
		_material.set_shader_parameter("split_alpha", true)
	else:
		_material.set_shader_parameter("source_tex", _sprite.texture)
		_material.set_shader_parameter("split_alpha", false)


func _set_texture(tex: ImageTexture, img: Image, param: String) -> ImageTexture:
	if tex == null or tex.get_size() != Vector2(img.get_size()):
		tex = ImageTexture.create_from_image(img)
		_material.set_shader_parameter(param, tex)
	else:
		tex.update(img)
	return tex
