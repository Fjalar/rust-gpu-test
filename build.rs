use spirv_builder::SpirvBuilder;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut builder = SpirvBuilder::new("shader", "spirv-unknown-vulkan1.2");
    builder.build_script.defaults = true;
    builder.build_script.env_shader_spv_path = Some(true);
    builder.build()?;
    Ok(())
}
