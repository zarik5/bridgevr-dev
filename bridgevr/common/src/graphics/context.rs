use crate::{constants::BVR_NAME, StrResult};
pub use gfx_hal::format::Format;
use gfx_hal::{adapter::MemoryType, prelude::*, queue::QueueGroup, *};
use log::debug;
use std::sync::Arc;

#[cfg(any(target_os = "linux", target_os = "android"))]
use gfx_backend_vulkan as back;

#[cfg(windows)]
use gfx_backend_dx11 as back;

#[cfg(target_os = "macos")]
use gfx_backend_metal as back;

pub(super) const TRACE_CONTEXT: &str = "Graphics";

type InstanceImpl = <back::Backend as gfx_hal::Backend>::Instance;
type PhysicalDeviceImpl = <back::Backend as gfx_hal::Backend>::PhysicalDevice;
type DeviceImpl = <back::Backend as gfx_hal::Backend>::Device;
pub(super) type MemoryImpl = <back::Backend as gfx_hal::Backend>::Memory;
pub(super) type BufferImpl = <back::Backend as gfx_hal::Backend>::Buffer;
pub(super) type ImageImpl = <back::Backend as gfx_hal::Backend>::Image;
pub(super) type ImageViewImpl = <back::Backend as gfx_hal::Backend>::ImageView;

#[cfg(windows)]
macro_rules! addr_of {
    ($e:expr) => {
        &mut $e as *mut _ as _
    };
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn format_from_native(vulkan_format: u32) -> Format {
    todo!();
}

#[cfg(windows)]
pub fn format_from_native(dxgi_format: u32) -> Format {
    use winapi::shared::dxgiformat::*;
    match dxgi_format {
        DXGI_FORMAT_R8G8B8A8_UNORM => Format::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Format::Rgba8Srgb,
        _ => Format::Rgba8Unorm,
    }
}

#[derive(Clone, Copy)]
pub struct TextureBounds {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

pub enum BufferUsage {
    Mutable,
    Immutable,
}

// Abstraction layer for graphics instance, device and context.
pub struct GraphicsContext {
    instance: InstanceImpl,
    physical_device: PhysicalDeviceImpl,
    pub(super) device: DeviceImpl,
    pub(super) queue_group: QueueGroup<back::Backend>,
    pub(super) memory_types: Vec<MemoryType>,
    pub(super) limits: Limits,
}

impl GraphicsContext {
    pub fn new(adapter_index: Option<usize>) -> StrResult<Self> {
        let instance = trace_err_dbg!(back::Instance::create(BVR_NAME, 1))?;

        let mut adapters = instance.enumerate_adapters();
        let adapter_index = adapter_index.unwrap_or(0);

        debug!("Selecting graphics adapter {} of:", adapter_index);
        for (i, adapter) in adapters.iter().enumerate() {
            debug!("{}: {:?}", i, adapter.info);
        }
        let adapter = adapters.remove(adapter_index);
        let physical_device = adapter.physical_device;
        let memory_types = physical_device.memory_properties().memory_types;
        let limits = physical_device.limits();

        let queue_family = trace_none!(adapter
            .queue_families
            .iter()
            .find(|qf| qf.queue_type().supports_graphics()))?; // todo filter for surface support

        let features = Features::empty(); // todo add features
        let mut gpu =
            trace_err!(unsafe { physical_device.open(&[(&queue_family, &[1.0])], features) })?;

        let device = gpu.device;
        let queue_group = trace_none!(gpu.queue_groups.pop())?;

        Ok(GraphicsContext {
            instance,
            physical_device,
            device,
            queue_group,
            memory_types,
            limits,
        })
    }

    #[cfg(target_os = "linux")]
    pub fn from_vulkan_ptrs(
        instance_ptr: u64,
        physical_device_ptr: u64,
        logical_device_ptr: u64,
        queue_ptr: u64,
        queue_family_index: u32,
    ) -> StrResult<Self> {
        // NB: vkImage/ash::vk::Image is u64 or *mut vkImage_T

        todo!();
    }

    #[cfg(windows)]
    pub fn from_device_ptr(device_ptr: u64) -> StrResult<Self> {
        todo!();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn instance_ptr(&self) -> u64 {
        use ash::{version::InstanceV1_0, vk::Handle};

        self.instance.raw.0.handle().as_raw()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn physical_device_ptr(&self) -> u64 {
        todo!()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn device_ptr(&self) -> u64 {
        use ash::vk::Handle;

        // Backend::Device has a field raw but it is private.
        // todo: mod gfx-hal crate to access private fields
        let raw: &Arc<gfx_backend_vulkan::RawDevice> =
            unsafe { &*(&self.device as *const _ as *const _) };
        raw.0.handle().as_raw()
    }

    #[cfg(windows)]
    pub fn device_ptr(&self) -> u64 {
        use winapi::um::d3d11;
        use wio::com::ComPtr;

        // todo: mod gfx-hal crate to access private fields
        let raw: &ComPtr<d3d11::ID3D11Device> = unsafe { &*(&self.device as *const _ as *const _) };
        raw.as_raw() as _
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn queue_ptr(&self) -> u64 {
        todo!()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn queue_family_index(&self) -> u32 {
        todo!()
    }
}

// impl Drop for Graphics2DAbstractionLayer {
//     fn drop(&mut self) {
//         unsafe {
//             // NB: inverse order of creation
//             // ManuallyDrop::drop(&mut self.graphics_queue_group);
//             // ManuallyDrop::drop(&mut self.device);
//             // ManuallyDrop::drop(&mut self.instance);
//         }
//     }
// }
