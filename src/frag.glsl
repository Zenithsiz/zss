#version 330 core

// Uniforms
uniform sampler2D tex_sampler;
uniform vec2 tex_offset;
uniform float alpha;

// Inputs
in vec2 frag_pos;
in vec2 frag_tex;

// Outputs
out vec4 color;

void main() {
	// Get the texture
	color = texture(tex_sampler, frag_tex + tex_offset);

	// Set alpha mixing
	color.a = alpha;
}
