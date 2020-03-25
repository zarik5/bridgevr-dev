mod graphics;

use bridgevr_common::*;
use graphics::*;
use safe_transmute::*;
use std::{mem::size_of, sync::Arc};

const TRACE_CONTEXT: &str = "Main";

const IMAGE_SIZE: (u32, u32) = (3200, 2400);

#[derive(Debug)]
struct Hello {}

fn run() -> StrResult {
    println!("Starting...");
    let context = Arc::new(trace_err!(graphics::GraphicsContext::new(None, &[], &[]))?);

    let buffer = Arc::new(Buffer::new(
        context.clone(),
        IMAGE_SIZE.0 as u64 * IMAGE_SIZE.1 as u64 * 4 * size_of::<f32>() as u64,
        BufferType::Storage,
    )?);

    let shader_bytecode = include_bytes!(concat!(env!("OUT_DIR"), "/shader.spv"));

    let operation_buffer = OperationBuffer::new(
        context,
        Operation::Render {
            layout: vec![vec![Binding::Buffer(buffer.clone())]],
            shader: shader_bytecode.to_vec(),
            resolution: IMAGE_SIZE,
        },
    )?;

    operation_buffer.execute()?;

    buffer.download(|data| {
        println!("Converting data...");
        let data = transmute_many::<f32, PermissiveGuard>(data).unwrap();
        let data = data.iter().map(|f| (f * 255.0) as u8).collect::<Vec<_>>();

        let file = std::fs::File::create("./mandelbrot.png").unwrap();

        let mut file_buf = std::io::BufWriter::new(file);

        let mut encoder = png::Encoder::new(&mut file_buf, IMAGE_SIZE.0, IMAGE_SIZE.1);
        encoder.set_color(png::ColorType::RGBA);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();

        println!("Saving data...");
        trace_err!(writer.write_image_data(&data))?;

        Ok(())
    })?;

    println!("Closing...");

    Ok(())
}

fn main() {
    match run() {
        Ok(()) => println!("All right!"),
        Err(e) => println!("{}", e),
    }
}
