use super::context::*;
use crate::StrResult;
use gfx_hal::{format::*, image::*, memory, prelude::*, *};
use std::{
    mem::ManuallyDrop,
    sync::{atomic::*, Arc},
    time::Duration,
};

pub use gfx_hal::format::Format;

pub struct Texture {
    graphics: Arc<GraphicsContext>,
    image_handle: ManuallyDrop<ImageImpl>,
    image_memory: ManuallyDrop<MemoryImpl>,
    image_view: ManuallyDrop<ImageViewImpl>,
    resolution: (u32, u32),
    format: Format,
    sample_count: u8,
    sync_acquired: AtomicBool,
}

impl Texture {
    fn create_image_memory_view(
        mut image_handle: &mut ManuallyDrop<ImageImpl>,
        graphics: Arc<GraphicsContext>,
        format: Format,
    ) -> StrResult<(ManuallyDrop<MemoryImpl>, ManuallyDrop<ImageViewImpl>)> {
        let dev = &graphics.device;

        let image_requirements = unsafe { dev.get_image_requirements(&image_handle) };

        let mem_type_id =
            trace_none!(graphics
                .memory_types
                .iter()
                .enumerate()
                .position(|(id, memory_type)| {
                    image_requirements.type_mask & (1 << id) != 0
                        && memory_type
                            .properties
                            .contains(memory::Properties::DEVICE_LOCAL)
                }))?
            .into();

        let image_memory = ManuallyDrop::new(unsafe {
            let mem = trace_err!(dev.allocate_memory(mem_type_id, image_requirements.size))?;
            trace_err!(dev.bind_image_memory(&mem, 0, &mut image_handle))?;
            mem
        });

        let image_view = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_image_view(
                &image_handle,
                ViewKind::D2,
                format,
                Swizzle::NO,
                SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            )
        })?);

        Ok((image_memory, image_view))
    }

    pub fn new(
        graphics: Arc<GraphicsContext>,
        (width, height): (u32, u32),
        format: Format,
        sample_count: u8,
    ) -> StrResult<Self> {
        let dev = &graphics.device;

        let kind = Kind::D2(width, height, /*layers*/ 1, sample_count);
        //todo check usage bits
        let usage = image::Usage::SAMPLED | image::Usage::STORAGE | image::Usage::COLOR_ATTACHMENT;

        let mut image_handle = ManuallyDrop::new(trace_err!(unsafe {
            dev.create_image(
                kind,
                1,
                format,
                Tiling::Optimal,
                usage,
                ViewCapabilities::empty(),
            )
        })?);

        let (image_memory, image_view) =
            Texture::create_image_memory_view(&mut image_handle, graphics.clone(), format)?;

        Ok(Self {
            graphics,
            image_handle,
            image_memory,
            image_view,
            resolution: (width, height),
            format,
            sample_count,
            sync_acquired: AtomicBool::new(false),
        })
    }

    pub fn graphics(&self) -> &Arc<GraphicsContext> {
        &self.graphics
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn from_shared_vulkan_ptrs(
        image_ptr: u64,
        graphics: Arc<GraphicsContext>,
        other_instance_ptr: u64,
        other_physical_device: u64,
        other_device: u64,
        other_queue: u64,
        other_queue_family_index: u32,
        (width, height): (u32, u32),
        format: Format,
        sample_count: u8,
    ) -> StrResult<Self> {
        // todo: error if instance or physical device differs from graphics al

        todo!();
    }

    #[cfg(windows)]
    pub fn from_handle(handle: u64, graphics: Arc<GraphicsContext>) -> StrResult<Self> {
        todo!();
    }

    #[cfg(windows)]
    pub fn from_ptr(ptr: u64, graphics: Arc<GraphicsContext>) -> StrResult<Self> {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn as_ptr(&self) -> u64 {
        todo!();
    }

    #[cfg(windows)]
    pub fn as_ptr(&self) -> u64 {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn as_handle(&self) -> u64 {
        todo!();
    }

    #[cfg(windows)]
    pub fn as_handle(&self) -> u64 {
        todo!();
    }

    pub fn read(&self) -> StrResult<Vec<u8>> {
        todo!();
    }

    pub fn write(&self, data: Vec<u8>) -> StrResult {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn acquire_sync(&self, timeout: Duration) -> StrResult {
        todo!();
    }

    #[cfg(windows)]
    pub fn acquire_sync(&self, timeout: Duration) -> StrResult {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn release_sync(&self) {
        todo!();
    }

    #[cfg(windows)]
    pub fn release_sync(&self) {
        todo!();
    }
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        todo!();
        // self.handle == other.handle
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.image_view);
            ManuallyDrop::drop(&mut self.image_memory);
            ManuallyDrop::drop(&mut self.image_handle);
        }
    }
}

// impl Drop for Texture {
//     fn drop(&mut self) {
//         if self.graphics.device.wait_idle().is_ok() {
//             unsafe {
//                 self.graphics
//                     .device
//                     .destroy_image(ManuallyDrop::into_inner(ptr::read(&mut self.image)));
//                 // todo: use ManuallyDrop::read/take when stabilized
//             }
//         }
//     }
// }
