// HD mode (AI), ADR 0100: background layer 2 blended with what it covers,
// like backgrnd.c:blit_background_row_blend, which works on palette indices:
// hue (high nibble) from layer 2, brightness (low nibble) halfway between
// the two. Upscaled images no longer have indices: each
// pixel (layer 2 and the image already drawn below it) is mapped to the
// nearest index of the game palette (64^3 tables from TyrianVga.nearest_index_lut:
// the whole palette for the background, only the hues of the layer 2
// tiles for layer 2, otherwise the upscaled ice blue lands on other hues),
// blended by index, then looked up in the palette again. Without blending (`blend_on` false:
// background2notTransparent, background2over 3), the layer stays opaque.
shader_type canvas_item;

uniform sampler2D screen_tex : hint_screen_texture, filter_nearest;
uniform sampler2D index_lut : filter_nearest;
uniform sampler2D under_lut : filter_nearest;
uniform sampler2D palette_tex : filter_nearest;
uniform bool blend_on = true;

int palette_index(sampler2D lut, vec3 c) {
	ivec3 q = ivec3(round(clamp(c, 0.0, 1.0) * 63.0));
	ivec2 p = ivec2(q.r + (q.b % 8) * 64, q.g + (q.b / 8) * 64);
	return int(round(texelFetch(lut, p, 0).r * 255.0));
}

void fragment() {
	if (blend_on && COLOR.a > 0.0) {
		int t = palette_index(index_lut, COLOR.rgb);
		int u = palette_index(under_lut, texture(screen_tex, SCREEN_UV).rgb);
		int r = (t & 0xf0) | (((u & 0x0f) + (t & 0x0f)) / 2);
		COLOR.rgb = texelFetch(palette_tex, ivec2(r, 0), 0).rgb;
	}
}
