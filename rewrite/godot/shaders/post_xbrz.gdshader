// Post-processing of the game image: see shaders/post_body.gdshaderinc.
// Variant with xBRZ ("Sharp pixel art" mode and the hq2x / hq3x / hq4x scalers).
shader_type canvas_item;

#define POST_XBRZ
#include "res://shaders/post_body.gdshaderinc"
