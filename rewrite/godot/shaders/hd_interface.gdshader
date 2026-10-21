// HD mode (AI) interface: the C code's 320x200 image (the node's texture) is
// displayed at 1280x800. Where `hd_tex` has an opaque pixel (original image
// upscaled by the model: backgrounds, frames, portraits, icons;
// TyrianVga.hd_image), it is used as is; elsewhere (text, digits,
// flat areas) the pixel art is upscaled sharply by xBRZ (same code as the
// "Sharp pixel art" mode). Transparent layer (`split_alpha`): colors and
// transparency are upscaled separately (TyrianVga.hd_xbrz_sources), giving clean edges
// without fringing. Logical positions and sizes are unchanged.
shader_type canvas_item;

#include "res://shaders/xbrz_freescale.gdshaderinc"

uniform sampler2D source_tex : filter_nearest;
uniform sampler2D alpha_tex : filter_nearest;
uniform sampler2D hd_tex : filter_nearest;
uniform bool hd_enabled = false;
uniform bool split_alpha = false;
uniform vec2 source_size = vec2(320.0, 200.0);

void fragment() {
	vec4 hd = hd_enabled ? texture(hd_tex, UV) : vec4(0.0);
	if (hd.a > 0.5) {
		COLOR = vec4(hd.rgb, 1.0);
	} else {
		vec3 c = xbrz_sample(source_tex, UV, source_size, source_size * 4.0);
		float a = split_alpha ? xbrz_sample(alpha_tex, UV, source_size, source_size * 4.0).r : texture(TEXTURE, UV).a;
		COLOR = vec4(c, a);
	}
}
