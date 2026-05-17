#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec4, vec4};
use spirv_std::spirv;

#[spirv(fragment)]
pub fn main_fs(#[spirv(frag_coord)] pos: Vec4, output: &mut Vec4) {
    let r = pos.x / 800.0;
    let g = pos.y / 600.0;
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
