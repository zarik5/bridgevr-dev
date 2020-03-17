mod buffer;
mod context;
mod texture;

use ash::{version::DeviceV1_0, *};
use bridgevr_common::*;
use safe_transmute::*;
use std::{ffi::CString, mem::size_of, sync::Arc};

pub use buffer::*;
pub use context::*;
pub use texture::*;

const WORKGROUP_SIZE: u32 = 32;

const SHADER_ENTRY_POINT: &str = "main";

pub enum Binding {
    Buffer(Arc<Buffer>),
    // Texture(Texture),
}

pub enum Operation {
    Render {
        layout: Vec<Vec<Binding>>,
        shader: Vec<u8>,
        resolution: (u32, u32),
    },
}

pub struct OperationBuffer {
    graphics_context: Arc<GraphicsContext>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    descriptor_pool: vk::DescriptorPool,
    shader_module: vk::ShaderModule,
    pipeline_layout: vk::PipelineLayout,
    pipelines: Vec<vk::Pipeline>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
}

impl OperationBuffer {
    pub fn new(graphics_context: Arc<GraphicsContext>, operation: Operation) -> StrResult<Self> {
        let dev = &graphics_context.device;
        match operation {
            Operation::Render {
                layout,
                shader,
                resolution: (width, height),
            } => {
                let buffer_type = vk::DescriptorType::STORAGE_BUFFER;

                let descriptor_set_layout_bindings = &[vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_type(buffer_type)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
                    .build()];
                let descriptor_set_layout_create_info =
                    vk::DescriptorSetLayoutCreateInfo::builder()
                        .bindings(descriptor_set_layout_bindings);
                let descriptor_set_layouts = vec![trace_err!(unsafe {
                    dev.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                })?];

                let descriptor_pool_sizes = [vk::DescriptorPoolSize::builder()
                    .ty(buffer_type)
                    .descriptor_count(1)
                    .build()];
                let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::builder()
                    .max_sets(1)
                    .pool_sizes(&descriptor_pool_sizes);
                let descriptor_pool = trace_err!(unsafe {
                    dev.create_descriptor_pool(&descriptor_pool_create_info, None)
                })?;

                let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&descriptor_set_layouts);
                let descriptor_sets = trace_err!(unsafe {
                    dev.allocate_descriptor_sets(&descriptor_set_allocate_info)
                })?;

                match &layout[0][0] {
                    Binding::Buffer(buffer) => {
                        let descriptor_buffer_info = [vk::DescriptorBufferInfo::builder()
                            .buffer(buffer.handle)
                            .offset(0)
                            .range(buffer.size)
                            .build()];
                        let dst_set = descriptor_sets[0];
                        let descriptor_writes = [vk::WriteDescriptorSet::builder()
                            .dst_set(dst_set)
                            .dst_binding(0)
                            .descriptor_type(buffer_type)
                            .buffer_info(&descriptor_buffer_info)
                            .build()];
                        unsafe { dev.update_descriptor_sets(&descriptor_writes, &[]) };
                    }
                }

                let mut shader_data = shader.to_vec();
                if shader_data.len() % size_of::<u32>() != 0 {
                    shader_data.extend_from_slice(&[0; size_of::<u32>()]);
                }
                // unwrap never fails
                let aligned_shader_data =
                    transmute_many::<u32, PermissiveGuard>(&shader_data).unwrap();

                let shader_module_create_info =
                    vk::ShaderModuleCreateInfo::builder().code(aligned_shader_data);
                let shader_module = trace_err!(unsafe {
                    dev.create_shader_module(&shader_module_create_info, None)
                })?;

                let pipeline_layout_create_info =
                    vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts);
                let pipeline_layout = trace_err!(unsafe {
                    dev.create_pipeline_layout(&pipeline_layout_create_info, None)
                })?;

                // unwrap never fails
                let entry_point_c_string = CString::new(SHADER_ENTRY_POINT).unwrap();
                let pipeline_create_infos = [vk::ComputePipelineCreateInfo::builder()
                    .stage(
                        vk::PipelineShaderStageCreateInfo::builder()
                            .stage(vk::ShaderStageFlags::COMPUTE)
                            .module(shader_module)
                            .name(entry_point_c_string.as_c_str())
                            .build(),
                    )
                    .layout(pipeline_layout)
                    .build()];
                let pipelines = trace_err!(unsafe {
                    dev.create_compute_pipelines(
                        vk::PipelineCache::null(),
                        &pipeline_create_infos,
                        None,
                    )
                }
                .map_err(|(_, e)| e))?;

                let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(graphics_context.queue_family_index);
                let command_pool = trace_err!(unsafe {
                    dev.create_command_pool(&command_pool_create_info, None)
                })?;

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);
                let command_buffer = trace_err!(unsafe {
                    dev.allocate_command_buffers(&command_buffer_allocate_info)
                })?[0];

                let begin_info = vk::CommandBufferBeginInfo::default();
                trace_err!(unsafe { dev.begin_command_buffer(command_buffer, &begin_info) })?;

                unsafe {
                    dev.cmd_bind_pipeline(
                        command_buffer,
                        vk::PipelineBindPoint::COMPUTE,
                        pipelines[0],
                    );
                    dev.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::COMPUTE,
                        pipeline_layout,
                        0,
                        &descriptor_sets,
                        &[],
                    );
                    dev.cmd_dispatch(
                        command_buffer,
                        (width as f32 / WORKGROUP_SIZE as f32).ceil() as u32,
                        (height as f32 / WORKGROUP_SIZE as f32).ceil() as u32,
                        1,
                    );
                }

                trace_err!(unsafe { dev.end_command_buffer(command_buffer) })?;

                Ok(Self {
                    graphics_context,
                    descriptor_set_layouts,
                    descriptor_pool,
                    shader_module,
                    pipeline_layout,
                    pipelines,
                    command_pool,
                    command_buffer,
                })
            }
        }
    }

    pub fn execute(&self) -> StrResult {
        let dev = &self.graphics_context.device;

        let fence_create_info = vk::FenceCreateInfo::default();
        let fence = trace_err!(unsafe { dev.create_fence(&fence_create_info, None) })?;

        let command_buffers = [self.command_buffer];
        let submit_info = vk::SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .build();
        trace_err!(unsafe {
            dev.queue_submit(self.graphics_context.queue, &[submit_info], fence)
        })?;

        trace_err!(unsafe { dev.wait_for_fences(&[fence], true, 100_000_000_000) })?;

        unsafe { dev.destroy_fence(fence, None) };

        Ok(())
    }
}

impl Drop for OperationBuffer {
    fn drop(&mut self) {
        let dev = &self.graphics_context.device;
        unsafe {
            dev.destroy_command_pool(self.command_pool, None);

            for &pipeline in &self.pipelines {
                dev.destroy_pipeline(pipeline, None);
            }

            dev.destroy_pipeline_layout(self.pipeline_layout, None);

            dev.destroy_shader_module(self.shader_module, None);

            dev.destroy_descriptor_pool(self.descriptor_pool, None);

            for &layout in &self.descriptor_set_layouts {
                dev.destroy_descriptor_set_layout(layout, None);
            }
        }
    }
}
