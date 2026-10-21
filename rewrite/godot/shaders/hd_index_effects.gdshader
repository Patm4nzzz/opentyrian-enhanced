// HD mode (AI): the C code's operations on palette indices, applied to the
// upscaled images, which no longer have indices. Each color is mapped to the
// nearest index (64^3 table from TyrianVga.nearest_index_lut), the index is
// transformed as in the C code, then looked up in the palette again; the
// difference between the HD color and the color of its index is added back
// (attenuated if the operation darkens): the detail of the upscaled image is kept.
//   mode 1 : sprite.c:blit_sprite2_blend (sprite hue, average brightness
//            with the background) - explosions, invulnerable ship;
//   mode 2 : blit_sprite2_darken (background at half brightness under the silhouette) -
//            shadows;
//   mode 4 : blit_sprite2_filter (`sprite_filter` hue, sprite brightness) -
//            hit or tinted enemies;
//   mode -1 : mode and filter read from the draw color (red = mode/255,
//            green = filter/255), to mix modes in a single node;
//   mode 5 : vga256d.c:JE_barBright on the background, applied `filter` times
//            (green of the draw color in mode -1) - Zinglon ray;
//   mode 6 : play-area rectangle: image filters from backgrnd.c
//            (`smoothie` 1 lava_filter, 2 water_filter, 3 iced_blur_filter),
//            the C code's per-line-band ripple; without the previous frame
//            (which the C code blends in), the neighbor below is read from the current frame;
//   mode 3 : play-area rectangle: backgrnd.c:JE_filterScreen (hue,
//            brightness), then enemy_render.c:JE_starShowVGA (1: flipped
//            image, 2: lamp centered on player 1).
shader_type canvas_item;

uniform sampler2D screen_tex : hint_screen_texture, filter_nearest;
uniform sampler2D index_lut : filter_nearest;
uniform sampler2D palette_tex : filter_nearest;
uniform int mode = 0;
uniform int sprite_filter = 0;
uniform int filter_col = -99;
uniform int filter_int = -99;
uniform int star_show_code = 0;
uniform int light_x = 0;
uniform int light_y = 0;
// Play area in logical pixels (264x184).
uniform vec2 play_size = vec2(264.0, 184.0);
uniform int smoothie = 0;
uniform int water_hue = 0;

varying flat int v_mode;
varying flat int v_filter;

void vertex() {
	v_mode = mode;
	v_filter = sprite_filter;
	if (mode == -1) {
		v_mode = int(round(COLOR.r * 255.0));
		v_filter = int(round(COLOR.g * 255.0));
		COLOR = vec4(1.0);
	}
}

int palette_index(vec3 c) {
	ivec3 q = ivec3(round(clamp(c, 0.0, 1.0) * 63.0));
	ivec2 p = ivec2(q.r + (q.b % 8) * 64, q.g + (q.b / 8) * 64);
	return int(round(texelFetch(index_lut, p, 0).r * 255.0));
}

vec3 pal(int i) {
	return texelFetch(palette_tex, ivec2(i, 0), 0).rgb;
}

float lum(vec3 c) {
	return dot(c, vec3(0.299, 0.587, 0.114));
}

// Index `t` in place of `i` for the HD color `c`.
vec3 remap(vec3 c, int i, int t) {
	vec3 a = pal(i);
	vec3 b = pal(t);
	float k = clamp((lum(b) + 0.02) / (lum(a) + 0.02), 0.0, 1.0);
	return clamp(b + (c - a) * k, 0.0, 1.0);
}

int filter_index(int s) {
	if (filter_col != -99) {
		s = ((filter_col & 0x0f) << 4) | (s & 0x0f);
	}
	if (filter_int != -99) {
		int t = (s & 0x0f) + filter_int;
		// Unsigned integer underflow in the C code: below zero, 0.
		int v = t < 0 ? 0 : min(t, 15);
		s = (s & 0xf0) | v;
	}
	return s;
}

int lamp_index(int s, int c, int r) {
	int y = int(play_size.y) - r;
	int x = int(play_size.x) - c;
	if (light_y > y) {
		return (s & 0xf0) | ((s >> 2) & 0x03);
	}
	int d = abs(light_x - x) + light_y;
	if (d < y) {
		return s;
	} else if (d - y <= 5) {
		return (s & 0xf0) | (((s & 0x0f) + 3 * (5 - (d - y))) / 4);
	}
	return (s & 0xf0) | ((s & 0x0f) >> 2);
}

void fragment() {
	if (v_mode == 1 && COLOR.a > 0.0) {
		int t = palette_index(COLOR.rgb);
		int u = palette_index(texture(screen_tex, SCREEN_UV).rgb);
		COLOR.rgb = remap(COLOR.rgb, t, (t & 0xf0) | (((u & 0x0f) + (t & 0x0f)) / 2));
		COLOR.a = 1.0;
	} else if (v_mode == 2 && COLOR.a > 0.0) {
		vec3 under = texture(screen_tex, SCREEN_UV).rgb;
		int u = palette_index(under);
		COLOR.rgb = remap(under, u, (u & 0xf0) | ((u & 0x0f) / 2));
		COLOR.a = 1.0;
	} else if (v_mode == 4 && COLOR.a > 0.0) {
		int t = palette_index(COLOR.rgb);
		COLOR.rgb = remap(COLOR.rgb, t, (v_filter & 0xf0) | (t & 0x0f));
		COLOR.a = 1.0;
	} else if (v_mode == 5 && COLOR.a > 0.0) {
		vec3 under = texture(screen_tex, SCREEN_UV).rgb;
		int u = palette_index(under);
		int n = v_filter;
		COLOR.rgb = remap(under, u, (u & 0xf0) | min((u & 0x0f) + 2 * n, 15));
		COLOR.a = 1.0;
	} else if (v_mode == 6) {
		ivec2 cell = ivec2(floor(UV * play_size));
		// Logical pixel of the 320x200 screen in SCREEN_UV coordinates.
		vec2 px = vec2(1.0 / 320.0, 1.0 / 200.0);
		vec3 c = texture(screen_tex, SCREEN_UV).rgb;
		int i = palette_index(c);
		int t = i;
		// C code traversal: from the end of the 185 lines of game_screen (x + 24)
		// toward the start, in groups of 8 pixels; w decreases by 8 per group.
		int gx = cell.x + 24;
		int w = 320 * 185 - 1 - 8 * ((184 - cell.y) * 40 + (319 - gx) / 8);
		if (smoothie == 1) {
			int waver = abs(((w >> 9) & 0x0f) - 8) - 1;
			int s = palette_index(texture(screen_tex, SCREEN_UV + vec2(float(waver), 0.0) * px).rgb);
			int below = palette_index(texture(screen_tex, SCREEN_UV + vec2(float(waver), 1.0) * px).rgb);
			int above = palette_index(texture(screen_tex, SCREEN_UV + vec2(float(waver), -1.0) * px).rgb);
			t = (((s & 0x0f) * 2 + (below & 0x0f) + (above & 0x0f)) / 4) | 0x70;
		} else if (smoothie == 2) {
			if ((i & 0x30) != 0) {
				int waver = abs(((w >> 10) & 0x07) - 4) - 1;
				int below = palette_index(texture(screen_tex, SCREEN_UV + vec2(float(waver), 1.0) * px).rgb);
				t = (((i & 0x0f) + (below & 0x0f)) / 2) | water_hue;
			}
		} else if (smoothie == 3) {
			t = (i & 0x0f) | 0x80;
		}
		COLOR = vec4(t == i ? c : remap(c, i, t), 1.0);
	} else if (v_mode == 3) {
		vec2 suv = SCREEN_UV;
		ivec2 cell = ivec2(floor(UV * play_size));
		if (star_show_code == 1) {
			// Flipped image: row r is read at 183 - r (play area at the top
			// left of the 320x200 screen).
			suv.y = (1.0 - UV.y) * play_size.y / 200.0;
		}
		vec3 c = texture(screen_tex, suv).rgb;
		int i = palette_index(c);
		int t = filter_index(i);
		if (star_show_code == 2) {
			t = lamp_index(t, cell.x, cell.y);
		}
		COLOR = vec4(t == i ? c : remap(c, i, t), 1.0);
	}
}
