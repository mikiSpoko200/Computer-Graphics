#version 450
#extension GL_ARB_explicit_uniform_location : require

layout(location = 0) uniform mat4 perspective_matrix;
layout(location = 1) uniform mat4 view_matrix;
layout(location = 2) uniform float grid_size;

out vec4 f_color;

vec3 from_01_to_ndc(vec3 position) {
    return 2 * position - 1;
}

vec3 instance_position() {
    float scale = 2.0 / grid_size;
    int div = int(grid_size);
    float x = mod(gl_InstanceID / div, div);
    float y = mod(gl_InstanceID / div * div, div);
    float z = mod(gl_InstanceID / div * div * div, div);
    return from_01_to_ndc(vec3(x, y, z) * scale);
}

vec3 instance_color(vec3 instance_ndc_position) {
    instance_ndc_position += 1;
    instance_ndc_position /= 2;
    float r = instance_ndc_position.x * 0.6 + 0.2;
    float g = instance_ndc_position.y * 0.6 + 0.2;
    float b = instance_ndc_position.z * 0.6 + 0.2;
    return vec3(1, 0, 1);
}

vec4 to_clip_space(vec4 position) {
    vec4 view_space_position = view_matrix * position;
    vec4 clip_space_position = perspective_matrix * view_space_position;
    return clip_space_position;
}

void main(void) {
    vec3 position = instance_position();
    f_color = vec4(instance_color(position), 1.0);
    vec4 view_position = vec4(position, 1.0);
    gl_Position = to_clip_space(view_position);
}