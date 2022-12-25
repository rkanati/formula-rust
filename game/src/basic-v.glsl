#version 310 es

layout(location = 0) uniform mat4 world_to_clip;
layout(location = 4) uniform vec3 translation;

layout(location = 0) in vec3 attr_xyz;
layout(location = 1) in vec3 attr_rgb;
layout(location = 2) in vec2 attr_uv;

out vec3 v_rgb;
out vec2 v_uv;

void main() {
    gl_Position = world_to_clip * vec4(attr_xyz + translation, 1.0);
    v_rgb = attr_rgb;
    v_uv  = attr_uv;
}

