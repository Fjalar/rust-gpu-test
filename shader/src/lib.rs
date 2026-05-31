#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec3, Vec4, vec3, vec4};
use spirv_std::spirv;

pub struct Display {
    x: u32,
    y: u32,
}

pub struct Params {
    t: f32,
}

pub struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
}

#[spirv(fragment)]
pub fn main_fs(
    #[spirv(frag_coord)] pos: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] display: &Display,
    #[spirv(uniform, descriptor_set = 1, binding = 0)] params: &Params,
    output: &mut Vec4,
) {
    let half_w = display.x as f32 / 2.0;
    let half_h = display.y as f32 / 2.0;
    let pixel = vec3(pos.x - half_w, pos.y - half_h, 0.0);
    let origin = vec3(0.0, 0.0, -1.0);
    let ray = pixel - origin;

    let triangles = [
        Triangle {
            a: Vec3::new(0.0, -200.0, 1.0),
            b: Vec3::new(200.0, 200.0, 1.0),
            c: Vec3::new(-200.0, 200.0, 1.0),
        },
        Triangle {
            a: Vec3::new(300.0, 200.0, 1.0),
            b: Vec3::new(500.0, -200.0, 1.0),
            c: Vec3::new(100.0, -200.0, 1.0),
        },
    ];

    for i in 0..triangles.len() {
        let intersection_point = moller_trumbore_intersection(origin, ray, &triangles[i]);
        if intersection_point != Vec3::ZERO {
            *output = vec4(
                (params.t as f32 % 4.0) * 0.25,
                (params.t as f32 % 2.0) * 0.5,
                (params.t as f32 % 1.0) * 1.0,
                1.0,
            );
        }
    }
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

// https://en.wikipedia.org/wiki/M%C3%B6ller%E2%80%93Trumbore_intersection_algorithm#Rust_implementation
fn moller_trumbore_intersection(origin: Vec3, direction: Vec3, triangle: &Triangle) -> Vec3 {
    let e1 = triangle.b - triangle.a;
    let e2 = triangle.c - triangle.a;

    let ray_cross_e2 = direction.cross(e2);
    let det = e1.dot(ray_cross_e2);

    if det > -f32::EPSILON && det < f32::EPSILON {
        return Vec3::ZERO; // This ray is parallel to this triangle.
    }

    let inv_det = 1.0 / det;
    let s = origin - triangle.a;
    let u = inv_det * s.dot(ray_cross_e2);
    if u < 0.0 || u > 1.0 {
        return Vec3::ZERO;
    }

    let s_cross_e1 = s.cross(e1);
    let v = inv_det * direction.dot(s_cross_e1);
    if v < 0.0 || u + v > 1.0 {
        return Vec3::ZERO;
    }
    // At this stage we can compute t to find out where the intersection point is on the line.
    let t = inv_det * e2.dot(s_cross_e1);

    if t > f32::EPSILON {
        // ray intersection
        let intersection_point = origin + direction * t;
        return intersection_point;
    } else {
        // This means that there is a line intersection but not a ray intersection.
        return Vec3::ZERO;
    }
}
