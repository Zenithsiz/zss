#version 330 core

// Uniforms
uniform sampler2D cur_tex;
uniform sampler2D next_tex;

// Inputs
in vec2 frag_pos;
in vec2 frag_tex;

// Outputs
out vec4 color;

void main() {
	color = texture(cur_tex, frag_tex);
}
