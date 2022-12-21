#version 310 es

precision highp float;

layout(binding = 0) uniform sampler2D tex;

//in vec3 v_bary;
in vec3 v_rgb;
in vec2 v_uv;

out vec4 frag;

void main() {
    //float peri = min(min(v_bary.x, v_bary.y), v_bary.z);
    //float t = step(0.01, peri);
    //frag = mix(vec4(1.0, 0.7, 1.0, 1), vec4(0, 0, 0, 1), t);
    frag = vec4(v_rgb * texture(tex, v_uv).rgb, 1);
}

