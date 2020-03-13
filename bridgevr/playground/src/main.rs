mod graphics;

use ash::*;
use bridgevr_common::*;
use graphics::*;
use std::sync::Arc;
use winit::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};

const WINDOW_SIZE: (u32, u32) = (500, 500);

const TRACE_CONTEXT: &str = "Main";

fn run() -> StrResult {
    let event_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("hello")
        .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1))
        .build(&event_loop)
        .expect("Failed to create window");

    let mut context = Arc::new(trace_err!(graphics::GraphicsContext::new(
        None, None, None
    ))?);

    let surface = Arc::new(trace_err!(Texture::new_surface(context.clone(), &window))?);

    // let _graphics_operation_buffer = OperationBuffer::new(context.clone());
    let _presentation_operation_buffer =
        trace_err!(OperationBuffer::new_present(context.clone(), surface))?;

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        }
        | Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                },
            ..
        } => {
            println!("Closing...");
            *control_flow = ControlFlow::Exit;
        }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        Event::RedrawRequested(_) => {}
        _ => {}
    });
}

fn main() {
    println!("Starting... //////////////////////////////////////////////////////////////////////");
    run().unwrap();
}
