#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec4, vec4};
use spirv_std::spirv;

pub struct Camera {
    x: u32,
    y: u32,
}

#[spirv(fragment)]
pub fn main_fs(
    #[spirv(frag_coord)] pos: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] camera: &Camera,
    output: &mut Vec4,
) {
    let r = pos.x / camera.x as f32;
    let g = pos.y / camera.y as f32;
    *output = vec4(r, g, 0.0, 1.0);
}

#[spirv(vertex)]
pub fn main_vs(
    #[spirv(vertex_index)] vert_id: i32,
    #[spirv(position, invariant)] out_pos: &mut Vec4,
) {
    let positions: [Vec4; 3] = [
        vec4(-1.0, 1.0, 0.0, 1.0),
        vec4(3.0, 1.0, 0.0, 1.0),
        vec4(-1.0, -3.0, 0.0, 1.0),
    ];
    *out_pos = positions[(vert_id) as usize];
}
