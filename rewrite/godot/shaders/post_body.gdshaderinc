// Post-processing of the game image (320x200, or 1280x800 in HD AI mode)
// displayed at the window resolution. Three independent effects:
//
//  - Scaler (graphics_menu.c:scaler, video_scale.c): resampling of the
//    pixel art applied to the 320x200 image before the integer scaling.
//    "None / 2x / 3x / 4x" = nearest neighbor (the factor only changes the
//    window size, video.c:init_scaler); "Scale2x" / "Scale3x" =
//    exact port of scale2x_32/scale3x_32 from src/video_scale.c (AdvMAME
//    EPX algorithm, GPL, already in this repository); "hq2x / hq3x / hq4x" =
//    closest equivalent: the xBRZ freescale filter already used by the
//    "Sharp pixel art" mode (docs/setup-options.md, docs/divergences.md).
//  - CRT (graphics_menu.c:crt, shader.c:frag_crt_src): six modes (Off,
//    Scanlines, Plus, Premium, Aperture, Soft); the mode uniforms
//    reproduce shader.c:shader_render exactly (curvature, scan
//    lines, phosphor halo, chromatic aberration, aperture grille,
//    vignetting, +20% compensation).
// Common body of the two variants (post.gdshader, post_xbrz.gdshader). xBRZ is only
// compiled in the POST_XBRZ variant: the full shader, with xBRZ repeated at each
// sample_image call, costs 2 to 3 s of compilation on the first frame under Mesa
// (llvmpipe), while the default setting (Scale3x, no CRT) does not need it.
#ifdef POST_XBRZ
#include "res://shaders/xbrz_freescale.gdshaderinc"
#endif

// --- Scaler ------------------------------------------------------------------
// 0 None, 1 2x, 2 Scale2x, 3 hq2x, 4 3x, 5 Scale3x, 6 hq3x, 7 4x, 8 hq4x
// (ordre de video_scale.c:scalers[]).
uniform int scaler = 0;
uniform bool xbrz_enabled = false;
// mainint.c:JE_gammaCorrect: each palette component is multiplied
// by r = 1 + gamma / 10 then clamped (Gamma key, in-level only).
uniform float gamma_gain : hint_range(1.0, 2.0) = 1.0;
uniform vec2 source_size = vec2(320.0, 200.0);
// Screen size in pixels (window, updated by effects.gd on each resize).
// Needed for xBRZ freescale outside fragment() (SCREEN_PIXEL_SIZE is only available
// in the fragment shader scope, not in helper functions).
uniform vec2 screen_size = vec2(1280.0, 800.0);
// Game image (SubViewport texture), sampled with nearest filtering.
// Explicit uniform: TEXTURE cannot be used in functions.
uniform sampler2D source_tex : filter_nearest;

// --- CRT (shader.c:shader_render) --------------------------------------------
uniform bool crt_enabled = false;
uniform int crt_mode = 0;
uniform float scanline_intensity = 0.0;
uniform float curvature = 0.0;
uniform float phosphor_glow = 0.0;
uniform float chroma_shift = 0.0;
uniform float aperture_strength = 0.0;
uniform float vignette_strength = 0.0;

// Scaler factor, in the order of video_scale.c:scalers[]:
// 0 None, 1 2x, 2 Scale2x, 3 hq2x, 4 3x, 5 Scale3x, 6 hq3x, 7 4x, 8 hq4x.
int scale_factor() {
	if (scaler == 1 || scaler == 2 || scaler == 3) {
		return 2;
	} else if (scaler == 4 || scaler == 5 || scaler == 6) {
		return 3;
	} else if (scaler == 7 || scaler == 8) {
		return 4;
	}
	return 1;
}

// Size of the image actually presented by the CRT: the scaled texture
// (video.c:shader_render passes src_w/src_h = dimensions of main_window_texture,
// i.e. the scaler factor applied). The scan lines and the halo are
// tuned to this size, not to 320x200.
vec2 crt_source_size() {
	return source_size * float(scale_factor());
}

// One source texel (coordinates in logical pixels 0..319 / 0..199), edge
// repeated as in the C code (video_scale.c: `x > 0 ? src - 1 : src`).
vec3 texel(vec2 cell) {
	vec2 c = clamp(cell, vec2(0.0), source_size - 1.0);
	return texture(source_tex, (c + 0.5) / source_size).rgb;
}

// video_scale.c:scale2x_32 - E0/E1/E2/E3 in the order top-left,
// top-right, bottom-left, bottom-right.
vec3 scale2x_cell(vec2 cell, vec2 sub) {
	vec3 e = texel(cell);
	vec3 b = texel(cell + vec2(0.0, -1.0));
	vec3 d = texel(cell + vec2(-1.0, 0.0));
	vec3 f = texel(cell + vec2(1.0, 0.0));
	vec3 h = texel(cell + vec2(0.0, 1.0));
	vec3 e0 = e, e1 = e, e2 = e, e3 = e;
	if (b != h && d != f) {
		e0 = d == b ? d : e;
		e1 = b == f ? f : e;
		e2 = d == h ? d : e;
		e3 = h == f ? f : e;
	}
	int ix = int(sub.x);
	int iy = int(sub.y);
	if (iy == 0) {
		return ix == 0 ? e0 : e1;
	}
	return ix == 0 ? e2 : e3;
}

// video_scale.c:scale3x_32 - E0..E8 in row-by-row order.
vec3 scale3x_cell(vec2 cell, vec2 sub) {
	vec3 a = texel(cell + vec2(-1.0, -1.0));
	vec3 b = texel(cell + vec2(0.0, -1.0));
	vec3 c = texel(cell + vec2(1.0, -1.0));
	vec3 d = texel(cell + vec2(-1.0, 0.0));
	vec3 e = texel(cell);
	vec3 f = texel(cell + vec2(1.0, 0.0));
	vec3 g = texel(cell + vec2(-1.0, 1.0));
	vec3 h = texel(cell + vec2(0.0, 1.0));
	vec3 i = texel(cell + vec2(1.0, 1.0));
	vec3 e0 = e, e1 = e, e2 = e, e3 = e, e4 = e, e5 = e, e6 = e, e7 = e, e8 = e;
	if (b != h && d != f) {
		e0 = d == b ? d : e;
		e1 = ((d == b && e != c) || (b == f && e != a)) ? b : e;
		e2 = b == f ? f : e;
		e3 = ((d == b && e != g) || (d == h && e != a)) ? d : e;
		e5 = ((b == f && e != i) || (h == f && e != c)) ? f : e;
		e6 = d == h ? d : e;
		e7 = ((d == h && e != i) || (h == f && e != g)) ? h : e;
		e8 = h == f ? f : e;
	}
	int ix = int(sub.x);
	int iy = int(sub.y);
	if (iy == 0) {
		return ix == 0 ? e0 : (ix == 1 ? e1 : e2);
	} else if (iy == 1) {
		return ix == 0 ? e3 : (ix == 1 ? e4 : e5);
	}
	return ix == 0 ? e6 : (ix == 1 ? e7 : e8);
}

// The C code's scaler applied to the 320x200 image: we recover the source pixel and the
// sub-pixel of the scaled block targeted by this sample (exact equivalent of
// "upscale with the scaler, then stretch with nearest neighbor").
vec3 sample_scaled(vec2 uv) {
	if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
		return vec3(0.0);
	}
	int k = scale_factor();
	// Scale2x (2) and Scale3x (5): exact port of video_scale.c.
	if (scaler == 2 || scaler == 5) {
		float fk = float(k);
		vec2 p = uv * source_size * fk;
		vec2 cell = floor(p / fk);
		vec2 sub = p - cell * fk;
		return k == 2 ? scale2x_cell(cell, sub) : scale3x_cell(cell, sub);
	}
	// hq2x (3), hq3x (6), hq4x (8): closest equivalent, the xBRZ filter
	// freescale, upscaled by the same factor (docs/setup-options.md).
#ifdef POST_XBRZ
	if (scaler == 3 || scaler == 6 || scaler == 8) {
		return xbrz_sample(source_tex, uv, source_size, source_size * float(k));
	}
#endif
	// None (0), 2x (1), 3x (4), 4x (7): nearest neighbor.
	return texture(source_tex, uv).rgb;
}

vec2 barrel_distort(vec2 coord, float amt) {
	vec2 cc = coord - 0.5;
	float dist = dot(cc, cc);
	return coord + cc * dist * amt;
}

vec3 sample_image(vec2 uv) {
	if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
		return vec3(0.0);
	}
#ifdef POST_XBRZ
	if (xbrz_enabled) {
		return xbrz_sample(source_tex, uv, source_size, screen_size);
	}
#endif
	return sample_scaled(uv);
}

void fragment() {
	vec3 col;
	bool out_of_bounds = false;
	if (!crt_enabled) {
		col = sample_image(UV);
	} else if (crt_mode == 1) {
		// "Scanlines": video.c:550-565, the path the C code takes when it has no
		// OpenGL context (Direct3D renderer by default on Windows, software
		// on Linux without GL): multiplicative brightening x(1 + 51/255), then
		// every second screen line (odd y from the top of the image) is veiled with
		// black at 51/255 (+8% on average). Before: GL formula from shader.c (sine
		// per source line, intensity 0.5): image 14% darker, pale yellow
		// turning dark yellow. Divergence #60 in docs/divergences.md.
		col = min(sample_image(UV) * (1.0 + 51.0 / 255.0), vec3(1.0));
		float row = floor(UV.y / max(abs(dFdy(UV.y)), 1e-6));
		if (mod(row, 2.0) >= 1.0) {
			col *= 1.0 - 51.0 / 255.0;
		}
	} else {
		// shader.c:frag_crt_src - same computations, in the same order.
		// `return` is forbidden in fragment() (Godot 4): out-of-bounds is tracked
		// with a boolean and the following computations are short-circuited.
		vec2 curved_uv = barrel_distort(UV, curvature * 0.15);
		out_of_bounds = (curved_uv.x < 0.0 || curved_uv.x > 1.0 || curved_uv.y < 0.0 || curved_uv.y > 1.0);
		if (!out_of_bounds) {
			float cs = chroma_shift;
			if (cs > 0.0) {
				col.r = sample_image(curved_uv + vec2(cs, 0.0)).r;
				col.g = sample_image(curved_uv).g;
				col.b = sample_image(curved_uv - vec2(cs, 0.0)).b;
			} else {
				col = sample_image(curved_uv);
			}
			if (phosphor_glow > 0.0) {
				vec2 px = 1.5 / crt_source_size();
				vec3 bloom = sample_image(curved_uv + vec2(px.x, 0.0));
				bloom += sample_image(curved_uv + vec2(-px.x, 0.0));
				bloom += sample_image(curved_uv + vec2(0.0, px.y));
				bloom += sample_image(curved_uv + vec2(0.0, -px.y));
				bloom *= 0.25;
				col += bloom * phosphor_glow;
			}
			float scanline = sin(curved_uv.y * crt_source_size().y * 3.14159) * 0.5 + 0.5;
			scanline = pow(scanline, 2.0);
			col *= mix(1.0, scanline, scanline_intensity);
			if (aperture_strength > 0.0) {
				float c3 = mod(FRAGCOORD.x, 3.0);
				vec3 mask = vec3(0.35);
				if (c3 < 1.0) {
					mask.r = 1.0;
				} else if (c3 < 2.0) {
					mask.g = 1.0;
				} else {
					mask.b = 1.0;
				}
				col *= mix(vec3(1.0), mask, aperture_strength);
			}
			vec2 vig_uv = curved_uv * (1.0 - curved_uv.yx);
			float vignette = pow(vig_uv.x * vig_uv.y * 15.0, 0.25);
			col *= mix(1.0, vignette, vignette_strength);
			col = clamp(col * 1.2, 0.0, 1.0);
		}
	}
	if (!out_of_bounds) {
		col = min(col * gamma_gain, vec3(1.0));
		COLOR = vec4(col, 1.0);
	} else {
		COLOR = vec4(0.0, 0.0, 0.0, 1.0);
	}
}
