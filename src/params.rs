#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Params {
    pub(crate) t: f32,
}
