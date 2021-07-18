#version 330 core

// Uniforms
uniform sampler2D tex;
uniform vec2 tex_offset;

// Inputs
in vec2 frag_pos;
in vec2 frag_tex;

// Outputs
out vec4 color;

void main() {
	color = texture(tex, frag_tex + tex_offset);
}
