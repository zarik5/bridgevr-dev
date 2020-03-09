mod context;
mod texture;
mod uniform_buffer;

use crate::StrResult;
use gfx_hal::{command::CommandBuffer, command::*, image::*, pool::*, prelude::*, pso::*, *};
use std::{mem::ManuallyDrop, sync::Arc};

pub use context::*;
pub use texture::*;
pub use uniform_buffer::*;

// Dependency graph is inferred by the order of the operations and variant fields.
#[derive(Clone)]
pub enum OperationDesc {
    Rendering {
        input_textures: Vec<Arc<Texture>>,
        uniform_buffer: Option<Arc<UniformBuffer>>,
        shader: String,
        output_textures: Vec<Arc<Texture>>,
        alpha: bool,
    },
    CopyTexture {
        input: Arc<Texture>,
        bounds: TextureBounds,
        output: Arc<Texture>,
    },
}

pub struct OperationBuffer {
    graphics: Arc<GraphicsContext>,
    // command_pool: ManuallyDrop<CommandPool<back::Backend, Graphics>>,
    // command_buffer: ManuallyDrop<CommandBuffer<back::Backend, Graphics, MultiShot>>,
    // render_passes: Vec<<back::Backend as gfx_hal::Backend>::RenderPass>,
    // fences: Vec<<back::Backend as gfx_hal::Backend>::Fence>,
    // semaphores: Vec<<back::Backend as gfx_hal::Backend>::Semaphore>,
}

impl OperationBuffer {
    pub fn new(
        graphics: Arc<GraphicsContext>,
        operation_descs: &[OperationDesc],
    ) -> StrResult<OperationBuffer> {
        let dev = &graphics.device;

        let mut command_pool = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_command_pool(
                graphics.queue_group.family,
                CommandPoolCreateFlags::RESET_INDIVIDUAL,
            )
        })?);

        let mut command_buffer = unsafe { command_pool.allocate_one(command::Level::Primary) };
        unsafe { command_buffer.begin_primary(CommandBufferFlags::EMPTY) };

        for op_desc in operation_descs {
            match op_desc {
                OperationDesc::Rendering {
                    input_textures,
                    uniform_buffer,
                    shader,
                    output_textures,
                    alpha,
                } => {}
                OperationDesc::CopyTexture {
                    input,
                    output,
                    bounds,
                } => {}
            }
        }

        let bindings = [
            DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::SampledImage,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            },
            DescriptorSetLayoutBinding {
                binding: 1,
                ty: pso::DescriptorType::Sampler,
                count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            },
        ];
        let set_layout = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_descriptor_set_layout(&bindings, &[])
        })?);

        let mut desc_pool = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_descriptor_pool(
                1, // sets
                &[
                    DescriptorRangeDesc {
                        ty: DescriptorType::SampledImage,
                        count: 1,
                    },
                    DescriptorRangeDesc {
                        ty: DescriptorType::Sampler,
                        count: 1,
                    },
                ],
                DescriptorPoolCreateFlags::empty(),
            )
        })?);
        let desc_set = trace_err!(unsafe { desc_pool.allocate_set(&set_layout) })?;

        let sampler = trace_err!(unsafe {
            dev.create_sampler(&SamplerDesc::new(Filter::Linear, WrapMode::Clamp))
        })?;

        // let mut command_buffer =
        //     ManuallyDrop::new(command_pool.acquire_command_buffer::<MultiShot>());

        // let fences = vec![];
        // let semaphores = vec![];
        // let render_passes = vec![];

        //todo: use Arc::ptr_eq for checking if textures are the same object

        // unsafe {
        //     command_buffer.begin(/*pending resubmits*/ false)
        // };

        // command_buffer.begin_render_pass_inline (
        // )

        // let fence = graphics
        //     .device
        //     .create_fence(/*signaled*/ true)
        //     .unwrap();
        // let semaphore = graphics.device.create_semaphore().unwrap();

        // let render_pass = {
        //     let color_attachment = Attachment {
        //         format: Some(Format::Rgba8Unorm), // todo: infer from texture
        //         samples: 1,
        //         ops: AttachmentOps {
        //             load: AttachmentLoadOp::DontCare,
        //             store: AttachmentStoreOp::Store,
        //         },
        //         stencil_ops: AttachmentOps::DONT_CARE,
        //         layouts: Layout::Undefined..Layout::Present,
        //     };
        //     let subpass = SubpassDesc {
        //         colors: &[(0, Layout::ColorAttachmentOptimal)],
        //         depth_stencil: None,
        //         inputs: &[],
        //         resolves: &[],
        //         preserves: &[],
        //     };

        //     unsafe {
        //         ok_or_panic!(graphics.device.create_render_pass(
        //             &[color_attachment],
        //             &[subpass],
        //             &[/*subpass dependency*/],
        //         ))
        //     }
        // };

        //let image: <back::Device as gfx_hal::Device>::Backend::Image;
        // ok_or_panic!(device.create_image_view(
        //     &image,
        //     ViewKind::D2,
        //     Format::Rgba8Unorm, // todo: infer from texture
        //     Swizzle::NO,
        //     SubresourceRange {
        //         aspects: Aspects::COLOR,
        //         levels: ..1,
        //         layers: ..1,
        //     },
        // ), "Image view");

        // let framebuffer = ok_or_panic!(graphics.device.create_framebuffer(
        //     &render_pass,
        //     vec![image_view],
        //     Extent {
        //         width: extent.width as u32,
        //         height: extent.height as u32,
        //         depth: 1,
        //     },
        // ), "Framebuffer");

        todo!();

        // Self {
        //     graphics,
        //     command_pool,
        //     command_buffer,
        //     fences,
        //     semaphores,
        //     render_passes,
        // }
    }

    pub fn execute(&self) {
        todo!();
    }
}

// impl Drop for OperationBuffer {
//     fn drop(&mut self) {
//         if self.graphics.device.wait_idle().is_ok() {
//             unsafe {
//                 // for f in self.fences.drain(..) {
//                 //     self.graphics.device.destroy_fence(f);
//                 // }

//                 // let command_buffer = ManuallyDrop::into_inner(ptr::read(&mut self.command_buffer));

//                 // self.graphics.device.destroy_command_pool(
//                 //     ManuallyDrop::into_inner(ptr::read(&mut self.command_pool)).into_raw(),
//                 // );
//             }
//         }
//     }
// }
