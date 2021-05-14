mod context;
mod storage;
// mod texture;

use ash::{version::DeviceV1_0, *};
use bridgevr_common::*;
use safe_transmute::*;
use std::{ffi::CString, mem::size_of, sync::Arc};

pub use context::*;
pub use storage::*;
// pub use texture::*;

const WORKGROUP_SIZE: u32 = 32;

// Note: do not use specialization constant 0 (it could cause problems on nvidia)
// https://www.reddit.com/r/vulkan/comments/5eqgly/has_anybody_got_a_working_example_of_work_group/

// Note: compositor cannot be implemented with a single pipeline and a buffer array because the layers
// can be of different sizes. Also I cannot use a single pipeline and images with different bindings
// because I cannot change dynamically the number of bindings in a shader.
// The only option is a pipeline per layer pair (eye layer pairs should have the same size)

// Use cases:
// * Storage buffers bindings are always fixed
// * Some image bindings are always fixed (mediacodec output surface) and some can change (OpenVR
// layers and OpenXR swapchain surfaces)

// Implementation:
// Buffers + non swapping images bindings are bundled in the same immutable descriptor sets.
// Swapping images bindings are bundled in other descriptor sets that are updated each frame.
// The command buffer needs to be rerecorded every frame because of mutable swapping images bindings
// and push constants

// Future optimizations (to be tested):
// Cache image descriptor sets. The primary command buffer cannot be cached because of the push
// constants but a secondary command buffers can be cached.

// todo: consider the complexity of the ComputeBuffer constructor; new ComputeBuffers need to be
// created when the layout of the layers change.

const SHADER_ENTRY_POINT: &str = "main";

pub struct BindingDesc<T>(u32, Vec<T>);

pub struct SpecializationConstantDesc(u32, Vec<u8>);

pub struct PipelineDesc {
    shader_bytecode: Vec<u8>,
    specialization_constants: Vec<SpecializationConstantDesc>,
    push_constants_size: Option<usize>,
    storage_buffers: Vec<BindingDesc<Buffer>>,
    // images: Vec<BindingDesc<Buffer>>,
    group_count: (u32, u32),
}

pub struct PipelineExecutionData {
    push_constant_buffer: Vec<u8>,
}

struct CompiledPipelineData {
    descriptor_set: vk::DescriptorSet,
    pipeline: vk::Pipeline,
    group_count: (u32, u32),
}

pub struct ComputeBuffer {
    graphics_context: Arc<GraphicsContext>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    descriptor_pools: Vec<vk::DescriptorPool>,
    shader_modules: Vec<vk::ShaderModule>,
    pipeline_layouts: Vec<vk::PipelineLayout>,
    pipelines: Vec<vk::Pipeline>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    compiled_pipelines: Vec<CompiledPipelineData>,
}

impl ComputeBuffer {
    pub fn new(
        graphics_context: Arc<GraphicsContext>,
        pipelines_descs: &[PipelineDesc],
    ) -> StrResult<Self> {
        let dev = &graphics_context.device;

        let mut descriptor_set_layouts = vec![];
        let mut descriptor_pools = vec![];
        let mut shader_modules = vec![];
        let mut pipeline_layouts = vec![];
        let mut pipelines = vec![];
        let mut compiled_pipelines = vec![];

        for pip_desc in pipelines_descs {
            let mut descriptor_set_layout_bindings = vec![];
            for BindingDesc(index, buffers) in pip_desc.storage_buffers {
                descriptor_set_layout_bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(index)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(buffers.len() as _)
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                        .build(),
                );
            }
            // todo add image descriptor set layout bindings
            let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&descriptor_set_layout_bindings);
            let descriptor_set_layout = trace_err!(unsafe {
                dev.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
            })?;
            descriptor_set_layouts.push(descriptor_set_layout);

            let descriptor_pool_sizes = vec![];
            for BindingDesc(_, buffers) in pip_desc.storage_buffers {
                descriptor_pool_sizes.push(
                    vk::DescriptorPoolSize::builder()
                        .ty(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(buffers.len() as _)
                        .build(),
                );
            }
            // todo add image descriptor pool sizes
            let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::builder()
                .max_sets(1)
                .pool_sizes(&descriptor_pool_sizes);
            let descriptor_pool = trace_err!(unsafe {
                dev.create_descriptor_pool(&descriptor_pool_create_info, None)
            })?;
            descriptor_pools.push(descriptor_pool);

            let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&descriptor_set_layouts);
            let descriptor_set =
                trace_err!(unsafe { dev.allocate_descriptor_sets(&descriptor_set_allocate_info) })?
                    [0];

            for BindingDesc(index, buffers) in pip_desc.storage_buffers {
                let descriptor_buffer_info = [vk::DescriptorBufferInfo::builder()
                    .buffer(buffer.handle)
                    .offset(0)
                    .range(buffer.size)
                    .build()];
                let descriptor_writes = [vk::WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(buffer_type)
                    .buffer_info(&descriptor_buffer_info)
                    .build()];
                unsafe { dev.update_descriptor_sets(&descriptor_writes, &[]) };
            }
            // todo: update descriptor sets with images

            // let mut shader_data = pip_desc.shader_bytecode.clone();
            // if shader_data.len() % size_of::<u32>() != 0 {
            //     shader_data.extend_from_slice(&[0; size_of::<u32>()]);
            // }
            // // unwrap never fails
            // let aligned_shader_data = transmute_many::<u32, PermissiveGuard>(&shader_data).unwrap();

            // let shader_module_create_info =
            //     vk::ShaderModuleCreateInfo::builder().code(aligned_shader_data);
            // let shader_module =
            //     trace_err!(unsafe { dev.create_shader_module(&shader_module_create_info, None) })?;

            // let pipeline_layout_create_info =
            //     vk::PipelineLayoutCreateInfo::builder().set_layouts(&descriptor_set_layouts); fdsfdsf
            // let pipeline_layout = trace_err!(unsafe {
            //     dev.create_pipeline_layout(&pipeline_layout_create_info, None)
            // })?;

            // // unwrap never fails
            // let entry_point_c_string = CString::new(SHADER_ENTRY_POINT).unwrap();
            // let pipeline_create_infos = [vk::ComputePipelineCreateInfo::builder()
            //     .stage(
            //         vk::PipelineShaderStageCreateInfo::builder()
            //             .stage(vk::ShaderStageFlags::COMPUTE)
            //             .module(shader_module)
            //             .name(entry_point_c_string.as_c_str())
            //             .build(),
            //     )
            //     .layout(pipeline_layout)
            //     .build()];
            // let pipelines = trace_err!(unsafe {
            //     dev.create_compute_pipelines(
            //         vk::PipelineCache::null(),
            //         &pipeline_create_infos,
            //         None,
            //     )
            // }
            // .map_err(|(_, e)| e))?;

            compiled_pipelines.push(CompiledPipelineData {});
        }

        let command_pool_create_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(graphics_context.queue_family_index);
        let command_pool =
            trace_err!(unsafe { dev.create_command_pool(&command_pool_create_info, None) })?;

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer =
            trace_err!(unsafe { dev.allocate_command_buffers(&command_buffer_allocate_info) })?[0];

        Ok(Self {
            graphics_context,
            descriptor_set_layouts,
            descriptor_pools,
            shader_modules,
            pipeline_layouts,
            pipelines,
            command_pool,
            command_buffer,
            compiled_pipelines,
        })
    }

    // Note: additional_data must be coherent with pipelines_descs.
    // Any invalid usage can result in a crash.
    pub unsafe fn record_and_execute(
        &self,
        execution_data: Vec<PipelineExecutionData>,
    ) -> StrResult {
        let dev = &self.graphics_context.device;

        // let begin_info = vk::CommandBufferBeginInfo::default();
        // trace_err!(unsafe { dev.begin_command_buffer(command_buffer, &begin_info) })?;

        // unsafe {
        //     dev.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::COMPUTE, pipelines[0]);
        //     dev.cmd_bind_descriptor_sets(
        //         command_buffer,
        //         vk::PipelineBindPoint::COMPUTE,
        //         pipeline_layout,
        //         0,
        //         &descriptor_sets,
        //         &[],
        //     );
        //     dev.cmd_dispatch(
        //         command_buffer,
        //         (width as f32 / WORKGROUP_SIZE as f32).ceil() as u32,
        //         (height as f32 / WORKGROUP_SIZE as f32).ceil() as u32,
        //         1,
        //     );
        // }

        // trace_err!(unsafe { dev.end_command_buffer(command_buffer) })?;

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

impl Drop for ComputeBuffer {
    fn drop(&mut self) {
        let dev = &self.graphics_context.device;
        unsafe {
            dev.destroy_command_pool(self.command_pool, None);
            for &pipeline in &self.pipelines {
                dev.destroy_pipeline(pipeline, None);
            }
            for &pipeline_layout in &self.pipeline_layouts {
                dev.destroy_pipeline_layout(pipeline_layout, None);
            }
            for &shader in &self.shader_modules {
                dev.destroy_shader_module(shader, None);
            }
            for &pool in &self.descriptor_pools {
                dev.destroy_descriptor_pool(pool, None);
            }
            for &layout in &self.descriptor_set_layouts {
                dev.destroy_descriptor_set_layout(layout, None);
            }
        }
    }
}
