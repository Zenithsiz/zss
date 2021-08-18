#version 330 core

// Uniforms
uniform mat4 mat;

// Inputs
in vec2 vertex_pos;
in vec2 vertex_tex;

// Outputs
out vec2 frag_pos;
out vec2 frag_tex;

void main() {
	frag_pos = vertex_pos;
	frag_tex = vertex_tex;

	vec4 pos = mat * vec4(vertex_pos, 0.0, 1.0);

	gl_Position = pos;
}
