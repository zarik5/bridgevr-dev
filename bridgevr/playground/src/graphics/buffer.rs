use super::context::*;
use ash::{
    version::{DeviceV1_0, InstanceV1_0},
    *,
};
use bridgevr_common::*;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum BufferType {
    UniformImmutable,
    UniformMutable,
    Storage,
}

fn get_buffer_usage_flags(buffer_type: BufferType) -> vk::BufferUsageFlags {
    match buffer_type {
        BufferType::UniformImmutable | BufferType::UniformMutable => {
            vk::BufferUsageFlags::UNIFORM_BUFFER
        }
        BufferType::Storage => vk::BufferUsageFlags::STORAGE_BUFFER,
    }
}

fn get_memory_property_flags(_: BufferType) -> vk::MemoryPropertyFlags {
    vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE
    // todo: distinguish cases
}

pub struct Buffer {
    graphics_context: Arc<GraphicsContext>,
    pub(super) size: u64,
    pub(super) ty: BufferType,
    pub(super) handle: vk::Buffer,
    memory: vk::DeviceMemory,
}

impl Buffer {
    pub fn new(
        graphics_context: Arc<GraphicsContext>,
        size: u64,
        ty: BufferType,
    ) -> StrResult<Self> {
        let dev = &graphics_context.device;

        let buffer_create_info = vk::BufferCreateInfo::builder()
            .size(size)
            .usage(get_buffer_usage_flags(ty))
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = trace_err!(unsafe { dev.create_buffer(&buffer_create_info, None) })?;

        let memory_requirements = unsafe { dev.get_buffer_memory_requirements(buffer) };
        let memory_property_flags = get_memory_property_flags(ty);

        let memory_type_index = trace_none!(graphics_context
            .memory_properties
            .memory_types
            .iter()
            .take(graphics_context.memory_properties.memory_type_count as _)
            .enumerate()
            .find(|&(i, m)| {
                memory_requirements.memory_type_bits & (1 << i as u32) != 0
                    && m.property_flags.contains(memory_property_flags)
            }))?
        .0 as _;
        let memory_allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(memory_requirements.size)
            .memory_type_index(memory_type_index);
        let memory = trace_err!(unsafe { dev.allocate_memory(&memory_allocate_info, None) })?;

        trace_err!(unsafe { dev.bind_buffer_memory(buffer, memory, 0) })?;

        Ok(Self {
            graphics_context,
            size,
            ty,
            handle: buffer,
            memory,
        })
    }

    pub fn download(&self, callback: impl FnOnce(&[u8]) -> StrResult) -> StrResult {
        let dev = &self.graphics_context.device;
        let memory_ptr = trace_err!(unsafe {
            dev.map_memory(self.memory, 0, self.size, vk::MemoryMapFlags::empty())
        })?;

        callback(unsafe { std::slice::from_raw_parts(memory_ptr as _, self.size as _) })?;

        unsafe { dev.unmap_memory(self.memory) };

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let dev = &self.graphics_context.device;
        unsafe {
            dev.free_memory(self.memory, None);
            dev.destroy_buffer(self.handle, None);
        }
    }
}
