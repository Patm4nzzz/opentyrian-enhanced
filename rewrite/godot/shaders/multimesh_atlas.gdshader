// Projectiles as a MultiMesh: each instance displays a sub-region of the atlas.
// INSTANCE_CUSTOM = (u, v, width, height), normalized within the texture.
shader_type canvas_item;

varying vec4 region;

void vertex() {
	region = INSTANCE_CUSTOM;
}

void fragment() {
	COLOR = texture(TEXTURE, region.xy + UV * region.zw);
}
