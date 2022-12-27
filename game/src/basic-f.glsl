#version 310 es

precision highp float;

layout(binding = 0) uniform sampler2D tex;

in vec3 v_rgb;
in vec2 v_uv;

out vec4 frag;

void main() {
    vec4 texel = texture(tex, v_uv);
    //vec4 texel = vec4(1,1,1,1);
    if(texel.a < 0.5) discard;
    vec3 final = v_rgb * texel.rgb;
    frag = vec4(final * final, 1);
}

