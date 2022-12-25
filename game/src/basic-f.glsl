#version 310 es

precision highp float;

layout(binding = 0) uniform sampler2D tex;

in vec3 v_rgb;
in vec2 v_uv;

out vec4 frag;

void main() {
    vec3 final = (v_rgb * 2.0 * texture(tex, v_uv).rgb);
    frag = vec4(final * final, 1);
}

