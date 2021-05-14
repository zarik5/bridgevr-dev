use bridgevr_common::{data::BVR_NAME, *};
use gfx_hal::{prelude::*, Features};

#[cfg(windows)]
use gfx_backend_dx11 as back;
#[cfg(target_os = "macos")]
use gfx_backend_metal as back;
#[cfg(any(target_os = "linux", target_os = "android"))]
use gfx_backend_vulkan as back;

type InstanceImpl = <back::Backend as gfx_hal::Backend>::Instance;
type PhysicalDeviceImpl = <back::Backend as gfx_hal::Backend>::PhysicalDevice;
type DeviceImpl = <back::Backend as gfx_hal::Backend>::Device;
// pub(super) type MemoryImpl = <back::Backend as gfx_hal::Backend>::Memory;
// pub(super) type BufferImpl = <back::Backend as gfx_hal::Backend>::Buffer;
// pub(super) type ImageImpl = <back::Backend as gfx_hal::Backend>::Image;
// pub(super) type ImageViewImpl = <back::Backend as gfx_hal::Backend>::ImageView;

pub(super) const TRACE_CONTEXT: &str = "Graphics";

pub struct GraphicsContext {
    instance: InstanceImpl,
    physical_device: PhysicalDeviceImpl,
    pub(super) device: DeviceImpl,
}

impl GraphicsContext {
    pub fn new(adapter_index: Option<usize>) -> StrResult<Self> {
        let instance = trace_err_dbg!(InstanceImpl::create(BVR_NAME, 1))?;

        let adapter_index = adapter_index.unwrap_or(0);
        let adapter = instance.enumerate_adapters().remove(adapter_index);
        let physical_device = adapter.physical_device;

        let memory_types = physical_device.memory_properties();

        let queue_family = trace_none!(adapter
            .queue_families
            .iter()
            .find(|qf| qf.queue_type().supports_compute()))?;

        let features = Features::empty(); // todo add features
        let mut gpu =
            trace_err!(unsafe { physical_device.open(&[(&queue_family, &[1.0])], features) })?;
        let device = gpu.device;

        let queue_group = trace_none!(gpu.queue_groups.pop())?;

        Ok(Self {
            instance,
            physical_device,
            device,
        })
    }
}
