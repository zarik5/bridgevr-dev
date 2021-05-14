mod graphics;

use bridgevr_common::*;
use graphics::*;
use std::sync::Arc;

fn run() -> StrResult {
    let numbers = vec![1, 2, 3, 4, 5];

    let shader_bytecode = include_bytes!(concat!(env!("OUT_DIR"), "/shader.spv"));

    let context = Arc::new(GraphicsContext::new(None)?);
    let compute_buffer = ComputeBuffer::new(
        context,
        &[PipelineDesc {
            shader_bytecode: shader_bytecode.to_vec(),
            specialization_constants: gfx_hal::spec_const_list!(189),
            push_constants_size: None,
            storage_buffers: vec![],
            // images: Vec<(u32, Vec<Texture>)>,
            group_count: (32, 32),
        }],
    )?;

    Ok(())
}

fn main() {
    run().unwrap();
}
