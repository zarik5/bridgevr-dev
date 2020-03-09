use super::context::*;
use crate::StrResult;
use gfx_hal::{buffer::Usage, memory, prelude::*};
use log::error;
use std::{any::TypeId, iter, mem::ManuallyDrop, mem::*, ptr, sync::Arc};

pub struct UniformBuffer {
    graphics: Arc<GraphicsContext>,
    buffer_handle: ManuallyDrop<BufferImpl>,
    buffer_memory: ManuallyDrop<MemoryImpl>,
    struct_type: TypeId,
}

impl UniformBuffer {
    pub fn new<T: 'static>(graphics: Arc<GraphicsContext>) -> StrResult<Self> {
        let dev = &graphics.device;

        let non_coherent_alignment = graphics.limits.non_coherent_atom_size;
        let buffer_length = ((size_of::<T>() + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;
        let mut buffer_handle = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_buffer(buffer_length as u64, Usage::UNIFORM)
        })?);

        let buffer_requirements = unsafe { dev.get_buffer_requirements(&buffer_handle) };

        let mem_type_id =
            trace_none!(graphics
                .memory_types
                .iter()
                .enumerate()
                .position(|(id, mem_type)| {
                    buffer_requirements.type_mask & (1 << id) != 0
                        && mem_type
                            .properties
                            .contains(memory::Properties::CPU_VISIBLE)
                }))?
            .into();

        let buffer_memory = ManuallyDrop::new(unsafe {
            let mem = trace_err!(dev.allocate_memory(mem_type_id, buffer_requirements.size))?;
            trace_err!(dev.bind_buffer_memory(&mem, 0, &mut buffer_handle))?;
            mem
        });

        Ok(UniformBuffer {
            graphics,
            buffer_handle,
            buffer_memory,
            struct_type: TypeId::of::<T>(),
        })
    }

    pub fn write<T: 'static>(&self, data: &T) -> StrResult {
        // Cannot check this at compilation time: UniformBuffer cannot have type parameters because
        // I want multiple UniformBuffer with different struct types in the same operation desc vec.
        debug_assert_eq!(TypeId::of::<T>(), self.struct_type);

        let data_size = size_of::<T>();
        unsafe {
            let mapping = trace_err!(self
                .graphics
                .device
                .map_memory(&self.buffer_memory, 0..data_size as _))?;

            ptr::copy_nonoverlapping(data as *const _ as *const u8, mapping, data_size);

            // do not early return if flush fails because the buffer memory must be unmapped
            self.graphics
                .device
                .flush_mapped_memory_ranges(iter::once((&*self.buffer_memory, 0..data_size as _)))
                .map_err(|e| error!("[Graphics] Buffer map flush: {}", e))
                .ok();
            self.graphics.device.unmap_memory(&self.buffer_memory);
        }
        Ok(())
    }
}

impl Drop for UniformBuffer {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.buffer_memory);
            ManuallyDrop::drop(&mut self.buffer_handle);
        }
    }
}
