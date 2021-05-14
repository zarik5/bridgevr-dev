mod context;
mod storage;

use bridgevr_common::*;
use gfx_hal::{prelude::*, pso};
use safe_transmute::*;
use std::sync::Arc;

pub use context::*;
pub use storage::*;

const SHADER_ENTRY_POINT: &str = "main";

pub struct PipelineDesc {
    pub shader_bytecode: Vec<u8>,
    pub specialization_constants: pso::Specialization<'static>,
    pub push_constants_size: Option<usize>,
    pub storage_buffers: Vec<(u32, Vec<Buffer>)>,
    // pub images: Vec<(u32, Vec<Texture>)>,
    pub group_count: (u32, u32),
}

pub struct PipelineExecutionData {
    push_constant_buffer: Vec<u8>,
}

pub struct ComputeBuffer {}

impl ComputeBuffer {
    pub fn new(
        graphics_context: Arc<GraphicsContext>,
        pipelines_descs: &[PipelineDesc],
    ) -> StrResult<Self> {
        let dev = &graphics_context.device;

        for mut pip_desc in pipelines_descs {
            let descriptor_type = pso::DescriptorType::Buffer {
                ty: pso::BufferDescriptorType::Storage { read_only: false },
                format: pso::BufferDescriptorFormat::Structured {
                    dynamic_offset: false,
                },
            };

            // todo: check if shaderc already produces 4 byte aligned bytecode
            // pip_desc
            //     .shader_bytecode
            //     .extend_from_slice(&[0; size_of::<u32>() - 1]);

            let shader_bytecode_aligned = trace_err!(transmute_many::<u32, PedanticGuard>(
                &pip_desc.shader_bytecode
            ))?;
            let shader = trace_err!(unsafe { dev.create_shader_module(shader_bytecode_aligned) })?;

            let descriptor_set_layout = trace_err!(unsafe {
                dev.create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: descriptor_type,
                        count: 1,
                        stage_flags: pso::ShaderStageFlags::COMPUTE,
                        immutable_samplers: false,
                    }],
                    &[],
                )
            })?;

            let pipeline_layout =
                trace_err!(unsafe { dev.create_pipeline_layout(&[descriptor_set_layout], &[]) })?;
            // let mut specialization_map_entries = vec![];
            // let mut specialization_data = vec![];
            // let mut offset = 0;
            // for (idx, data) in pip_desc.specialization_constants.iter().enumerate() {
            //     let next_offset = offset + data.len();
            //     specialization_map_entries.push(pso::SpecializationConstant {
            //         id: idx as _,
            //         range: (offset as _)..(next_offset as _),
            //     });
            //     specialization_data.extend(data);
            //     offset = next_offset;
            // }
            let shader_entry_point = pso::EntryPoint {
                entry: SHADER_ENTRY_POINT,
                module: &shader,
                // specialization: pso::Specialization {
                //     constants: specialization_map_entries.into(),
                //     data: specialization_data.into(),
                // },
                specialization: pip_desc.specialization_constants.clone(),
            };
            let pipeline = trace_err!(unsafe {
                dev.create_compute_pipeline(
                    &pso::ComputePipelineDesc::new(shader_entry_point, &pipeline_layout),
                    None,
                )
            })?;

            let descriptor_pool = trace_err!(unsafe {
                dev.create_descriptor_pool(
                    1,
                    &[pso::DescriptorRangeDesc {
                        ty: descriptor_type,
                        count: 1,
                    }],
                    pso::DescriptorPoolCreateFlags::empty()
                )
            })?;
        }
        todo!()
    }
}
