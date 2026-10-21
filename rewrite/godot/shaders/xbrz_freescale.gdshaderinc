// xBRZ "freescale" (single pass, arbitrary upscaling factor).
// Godot port, with no algorithm change, of libretro/glsl-shaders
// xbrz/shaders/xbrz-freescale.glsl (commit c6a82efb2d811de70fd64a0971c819dfeb93d7d4).
// Only adaptations: macro P() replaced by a function, RetroArch uniforms
// replaced by the parameters of xbrz_sample(), neighbors A..I
// renamed pA..pI (E is Euler's constant in the Godot shader language).
// "Sharp pixel art" mode of the display options (ADR 0124, review point 17).
//
// Original licenses, reproduced verbatim:
/*
   Hyllian's xBR-vertex code and texel mapping

   Copyright (C) 2011/2016 Hyllian - sergiogdb@gmail.com

   Permission is hereby granted, free of charge, to any person obtaining a copy
   of this software and associated documentation files (the "Software"), to deal
   in the Software without restriction, including without limitation the rights
   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
   copies of the Software, and to permit persons to whom the Software is
   furnished to do so, subject to the following conditions:

   The above copyright notice and this permission notice shall be included in
   all copies or substantial portions of the Software.

   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
   THE SOFTWARE.
*/
// This shader also uses code and/or concepts from xBRZ as it appears
// in the Desmume source code. The license for which is as follows:
// ****************************************************************************
// * This file is part of the HqMAME project. It is distributed under         *
// * GNU General Public License: http://www.gnu.org/licenses/gpl-3.0          *
// * Copyright (C) Zenju (zenju AT gmx DOT de) - All Rights Reserved          *
// *                                                                          *
// * Additionally and as a special exception, the author gives permission     *
// * to link the code of this program with the MAME library (or with modified *
// * versions of MAME that use the same license as MAME), and distribute      *
// * linked combinations including the two. You must obey the GNU General     *
// * Public License in all respects for all of the code used other than MAME. *
// * If you modify this file, you may extend this exception to your version   *
// * of the file, but you are not obligated to do so. If you do not wish to   *
// * do so, delete this exception statement from your version.                *
// ****************************************************************************

#define XBRZ_BLEND_NONE 0
#define XBRZ_BLEND_NORMAL 1
#define XBRZ_BLEND_DOMINANT 2
#define XBRZ_LUMINANCE_WEIGHT 1.0
#define XBRZ_EQUAL_COLOR_TOLERANCE (30.0 / 255.0)
#define XBRZ_STEEP_DIRECTION_THRESHOLD 2.2
#define XBRZ_DOMINANT_DIRECTION_THRESHOLD 3.6

float xbrz_dist(vec3 pixA, vec3 pixB) {
	const vec3 w = vec3(0.2627, 0.6780, 0.0593);
	const float scaleB = 0.5 / (1.0 - 0.0593);
	const float scaleR = 0.5 / (1.0 - 0.2627);
	vec3 diff = pixA - pixB;
	float Y = dot(diff.rgb, w);
	float Cb = scaleB * (diff.b - Y);
	float Cr = scaleR * (diff.r - Y);
	return sqrt(((XBRZ_LUMINANCE_WEIGHT * Y) * (XBRZ_LUMINANCE_WEIGHT * Y)) + (Cb * Cb) + (Cr * Cr));
}

bool xbrz_equal(vec3 pixA, vec3 pixB) {
	return xbrz_dist(pixA, pixB) < XBRZ_EQUAL_COLOR_TOLERANCE;
}

float xbrz_left_ratio(vec2 center, vec2 origin, vec2 direction, vec2 scale) {
	vec2 P0 = center - origin;
	vec2 proj = direction * (dot(P0, direction) / dot(direction, direction));
	vec2 distv = P0 - proj;
	vec2 orth = vec2(-direction.y, direction.x);
	float side = sign(dot(P0, orth));
	float v = side * length(distv * scale);
	return smoothstep(-sqrt(2.0) / 2.0, sqrt(2.0) / 2.0, v);
}

vec3 xbrz_tap(sampler2D tex, vec2 coord, vec2 texel, float x, float y) {
	return texture(tex, coord + texel * vec2(x, y)).rgb;
}

// tex: source image sampled with nearest filtering; uv: image coordinate;
// source_size: size of the source image in pixels;
// output_size: output size in pixels (sets the edge anti-aliasing).
vec3 xbrz_sample(sampler2D tex, vec2 uv, vec2 source_size, vec2 output_size) {
	vec2 texel = 1.0 / source_size;
	vec2 scale = output_size * texel;
	vec2 pos = fract(uv * source_size) - vec2(0.5, 0.5);
	vec2 coord = uv - pos * texel;

	vec3 pA = xbrz_tap(tex, coord, texel, -1., -1.);
	vec3 pB = xbrz_tap(tex, coord, texel, 0., -1.);
	vec3 pC = xbrz_tap(tex, coord, texel, 1., -1.);
	vec3 pD = xbrz_tap(tex, coord, texel, -1., 0.);
	vec3 pE = xbrz_tap(tex, coord, texel, 0., 0.);
	vec3 pF = xbrz_tap(tex, coord, texel, 1., 0.);
	vec3 pG = xbrz_tap(tex, coord, texel, -1., 1.);
	vec3 pH = xbrz_tap(tex, coord, texel, 0., 1.);
	vec3 pI = xbrz_tap(tex, coord, texel, 1., 1.);

	ivec4 blendResult = ivec4(XBRZ_BLEND_NONE);

	if (!((pE == pF && pH == pI) || (pE == pH && pF == pI))) {
		float dist_H_F = xbrz_dist(pG, pE) + xbrz_dist(pE, pC) + xbrz_dist(xbrz_tap(tex, coord, texel, 0., 2.), pI) + xbrz_dist(pI, xbrz_tap(tex, coord, texel, 2., 0.)) + (4.0 * xbrz_dist(pH, pF));
		float dist_E_I = xbrz_dist(pD, pH) + xbrz_dist(pH, xbrz_tap(tex, coord, texel, 1., 2.)) + xbrz_dist(pB, pF) + xbrz_dist(pF, xbrz_tap(tex, coord, texel, 2., 1.)) + (4.0 * xbrz_dist(pE, pI));
		bool dominantGradient = (XBRZ_DOMINANT_DIRECTION_THRESHOLD * dist_H_F) < dist_E_I;
		blendResult.z = ((dist_H_F < dist_E_I) && pE != pF && pE != pH) ? ((dominantGradient) ? XBRZ_BLEND_DOMINANT : XBRZ_BLEND_NORMAL) : XBRZ_BLEND_NONE;
	}
	if (!((pD == pE && pG == pH) || (pD == pG && pE == pH))) {
		float dist_G_E = xbrz_dist(xbrz_tap(tex, coord, texel, -2., 1.), pD) + xbrz_dist(pD, pB) + xbrz_dist(xbrz_tap(tex, coord, texel, -1., 2.), pH) + xbrz_dist(pH, pF) + (4.0 * xbrz_dist(pG, pE));
		float dist_D_H = xbrz_dist(xbrz_tap(tex, coord, texel, -2., 0.), pG) + xbrz_dist(pG, xbrz_tap(tex, coord, texel, 0., 2.)) + xbrz_dist(pA, pE) + xbrz_dist(pE, pI) + (4.0 * xbrz_dist(pD, pH));
		bool dominantGradient = (XBRZ_DOMINANT_DIRECTION_THRESHOLD * dist_D_H) < dist_G_E;
		blendResult.w = ((dist_G_E > dist_D_H) && pE != pD && pE != pH) ? ((dominantGradient) ? XBRZ_BLEND_DOMINANT : XBRZ_BLEND_NORMAL) : XBRZ_BLEND_NONE;
	}
	if (!((pB == pC && pE == pF) || (pB == pE && pC == pF))) {
		float dist_E_C = xbrz_dist(pD, pB) + xbrz_dist(pB, xbrz_tap(tex, coord, texel, 1., -2.)) + xbrz_dist(pH, pF) + xbrz_dist(pF, xbrz_tap(tex, coord, texel, 2., -1.)) + (4.0 * xbrz_dist(pE, pC));
		float dist_B_F = xbrz_dist(pA, pE) + xbrz_dist(pE, pI) + xbrz_dist(xbrz_tap(tex, coord, texel, 0., -2.), pC) + xbrz_dist(pC, xbrz_tap(tex, coord, texel, 2., 0.)) + (4.0 * xbrz_dist(pB, pF));
		bool dominantGradient = (XBRZ_DOMINANT_DIRECTION_THRESHOLD * dist_B_F) < dist_E_C;
		blendResult.y = ((dist_E_C > dist_B_F) && pE != pB && pE != pF) ? ((dominantGradient) ? XBRZ_BLEND_DOMINANT : XBRZ_BLEND_NORMAL) : XBRZ_BLEND_NONE;
	}
	if (!((pA == pB && pD == pE) || (pA == pD && pB == pE))) {
		float dist_D_B = xbrz_dist(xbrz_tap(tex, coord, texel, -2., 0.), pA) + xbrz_dist(pA, xbrz_tap(tex, coord, texel, 0., -2.)) + xbrz_dist(pG, pE) + xbrz_dist(pE, pC) + (4.0 * xbrz_dist(pD, pB));
		float dist_A_E = xbrz_dist(xbrz_tap(tex, coord, texel, -2., -1.), pD) + xbrz_dist(pD, pH) + xbrz_dist(xbrz_tap(tex, coord, texel, -1., -2.), pB) + xbrz_dist(pB, pF) + (4.0 * xbrz_dist(pA, pE));
		bool dominantGradient = (XBRZ_DOMINANT_DIRECTION_THRESHOLD * dist_D_B) < dist_A_E;
		blendResult.x = ((dist_D_B < dist_A_E) && pE != pD && pE != pB) ? ((dominantGradient) ? XBRZ_BLEND_DOMINANT : XBRZ_BLEND_NORMAL) : XBRZ_BLEND_NONE;
	}

	vec3 res = pE;

	if (blendResult.z != XBRZ_BLEND_NONE) {
		float dist_F_G = xbrz_dist(pF, pG);
		float dist_H_C = xbrz_dist(pH, pC);
		bool doLineBlend = (blendResult.z == XBRZ_BLEND_DOMINANT ||
				!((blendResult.y != XBRZ_BLEND_NONE && !xbrz_equal(pE, pG)) || (blendResult.w != XBRZ_BLEND_NONE && !xbrz_equal(pE, pC)) ||
				(xbrz_equal(pG, pH) && xbrz_equal(pH, pI) && xbrz_equal(pI, pF) && xbrz_equal(pF, pC) && !xbrz_equal(pE, pI))));
		vec2 origin = vec2(0.0, 1.0 / sqrt(2.0));
		vec2 direction = vec2(1.0, -1.0);
		if (doLineBlend) {
			bool haveShallowLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_F_G <= dist_H_C) && pE != pG && pD != pG;
			bool haveSteepLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_H_C <= dist_F_G) && pE != pC && pB != pC;
			origin = haveShallowLine ? vec2(0.0, 0.25) : vec2(0.0, 0.5);
			direction.x += haveShallowLine ? 1.0 : 0.0;
			direction.y -= haveSteepLine ? 1.0 : 0.0;
		}
		vec3 blendPix = mix(pH, pF, step(xbrz_dist(pE, pF), xbrz_dist(pE, pH)));
		res = mix(res, blendPix, xbrz_left_ratio(pos, origin, direction, scale));
	}
	if (blendResult.w != XBRZ_BLEND_NONE) {
		float dist_H_A = xbrz_dist(pH, pA);
		float dist_D_I = xbrz_dist(pD, pI);
		bool doLineBlend = (blendResult.w == XBRZ_BLEND_DOMINANT ||
				!((blendResult.z != XBRZ_BLEND_NONE && !xbrz_equal(pE, pA)) || (blendResult.x != XBRZ_BLEND_NONE && !xbrz_equal(pE, pI)) ||
				(xbrz_equal(pA, pD) && xbrz_equal(pD, pG) && xbrz_equal(pG, pH) && xbrz_equal(pH, pI) && !xbrz_equal(pE, pG))));
		vec2 origin = vec2(-1.0 / sqrt(2.0), 0.0);
		vec2 direction = vec2(1.0, 1.0);
		if (doLineBlend) {
			bool haveShallowLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_H_A <= dist_D_I) && pE != pA && pB != pA;
			bool haveSteepLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_D_I <= dist_H_A) && pE != pI && pF != pI;
			origin = haveShallowLine ? vec2(-0.25, 0.0) : vec2(-0.5, 0.0);
			direction.y += haveShallowLine ? 1.0 : 0.0;
			direction.x += haveSteepLine ? 1.0 : 0.0;
		}
		vec3 blendPix = mix(pH, pD, step(xbrz_dist(pE, pD), xbrz_dist(pE, pH)));
		res = mix(res, blendPix, xbrz_left_ratio(pos, origin, direction, scale));
	}
	if (blendResult.y != XBRZ_BLEND_NONE) {
		float dist_B_I = xbrz_dist(pB, pI);
		float dist_F_A = xbrz_dist(pF, pA);
		bool doLineBlend = (blendResult.y == XBRZ_BLEND_DOMINANT ||
				!((blendResult.x != XBRZ_BLEND_NONE && !xbrz_equal(pE, pI)) || (blendResult.z != XBRZ_BLEND_NONE && !xbrz_equal(pE, pA)) ||
				(xbrz_equal(pI, pF) && xbrz_equal(pF, pC) && xbrz_equal(pC, pB) && xbrz_equal(pB, pA) && !xbrz_equal(pE, pC))));
		vec2 origin = vec2(1.0 / sqrt(2.0), 0.0);
		vec2 direction = vec2(-1.0, -1.0);
		if (doLineBlend) {
			bool haveShallowLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_B_I <= dist_F_A) && pE != pI && pH != pI;
			bool haveSteepLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_F_A <= dist_B_I) && pE != pA && pD != pA;
			origin = haveShallowLine ? vec2(0.25, 0.0) : vec2(0.5, 0.0);
			direction.y -= haveShallowLine ? 1.0 : 0.0;
			direction.x -= haveSteepLine ? 1.0 : 0.0;
		}
		vec3 blendPix = mix(pF, pB, step(xbrz_dist(pE, pB), xbrz_dist(pE, pF)));
		res = mix(res, blendPix, xbrz_left_ratio(pos, origin, direction, scale));
	}
	if (blendResult.x != XBRZ_BLEND_NONE) {
		float dist_D_C = xbrz_dist(pD, pC);
		float dist_B_G = xbrz_dist(pB, pG);
		bool doLineBlend = (blendResult.x == XBRZ_BLEND_DOMINANT ||
				!((blendResult.w != XBRZ_BLEND_NONE && !xbrz_equal(pE, pC)) || (blendResult.y != XBRZ_BLEND_NONE && !xbrz_equal(pE, pG)) ||
				(xbrz_equal(pC, pB) && xbrz_equal(pB, pA) && xbrz_equal(pA, pD) && xbrz_equal(pD, pG) && !xbrz_equal(pE, pA))));
		vec2 origin = vec2(0.0, -1.0 / sqrt(2.0));
		vec2 direction = vec2(-1.0, 1.0);
		if (doLineBlend) {
			bool haveShallowLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_D_C <= dist_B_G) && pE != pC && pF != pC;
			bool haveSteepLine = (XBRZ_STEEP_DIRECTION_THRESHOLD * dist_B_G <= dist_D_C) && pE != pG && pH != pG;
			origin = haveShallowLine ? vec2(0.0, -0.25) : vec2(0.0, -0.5);
			direction.x -= haveShallowLine ? 1.0 : 0.0;
			direction.y += haveSteepLine ? 1.0 : 0.0;
		}
		vec3 blendPix = mix(pD, pB, step(xbrz_dist(pE, pB), xbrz_dist(pE, pD)));
		res = mix(res, blendPix, xbrz_left_ratio(pos, origin, direction, scale));
	}
	return res;
}
