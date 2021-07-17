#version 330 core

// Uniforms
uniform float progress;

// Inputs
layout (location=0) in vec2 vertex_pos;
layout (location=1) in vec2 vertex_tex;

// Outputs
out vec2 frag_pos;
out vec2 frag_tex;

void main() {
	frag_pos = vertex_pos;
	frag_tex = vertex_tex;
	
	vec4 pos = vec4(vertex_pos, 0.0, 1.0);

	pos.y += progress;

	gl_Position = pos;
}
