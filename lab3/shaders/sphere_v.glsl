#version 450
#extension GL_ARB_explicit_uniform_location : require

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;

layout(location = 0) uniform mat4 perspective_matrix;
layout(location = 1) uniform mat4 view_matrix;
layout(location = 2) uniform vec3 light_direction;

out vec4 f_color;

const vec3 BALL_COLOR = vec3(1, 0.85, 0.82);

vec4 world_to_clip_space(vec4 position) {
    vec4 view_space_position = view_matrix * position;
    vec4 clip_space_position = perspective_matrix * view_space_position;
    return clip_space_position;
}

void main(void) {
    vec4 world_space_position = vec4(position + vec3(4, 0, 0) , 1.0);
    gl_Position = world_to_clip_space(world_space_position);
    f_color = vec4(BALL_COLOR * dot(normal, normalize(light_direction)), 1.0);
}