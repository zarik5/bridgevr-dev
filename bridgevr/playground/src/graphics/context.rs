use ash::version::InstanceV1_0;
use ash::{
    extensions::{ext::DebugUtils, *},
    version::{DeviceV1_0, EntryV1_0},
    *,
};
use bridgevr_common::{constants::*, *};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::ffi::{CStr, CString};

pub(super) const TRACE_CONTEXT: &str = "Graphics";

fn required_instance_extension_names() -> Vec<CString> {
    let mut extensions = vec![
        khr::Surface::name().to_owned(),
        khr::XlibSurface::name().to_owned(),
    ];
    if cfg!(debug_assertions) {
        extensions.push(DebugUtils::name().to_owned())
    }
    extensions
}

fn required_device_extension_names() -> Vec<CString> {
    vec![khr::Swapchain::name().to_owned()]
}

pub struct GraphicsContext {
    pub(super) entry: Entry, // this must outlive `instance`
    pub(super) instance: Instance,
    pub(super) surface_loader: khr::Surface,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) device: Device,
    pub(super) queue: vk::Queue,
}

impl GraphicsContext {
    pub fn new(
        adapter_index: Option<usize>,
        instance_extensions_names: Option<&[&str]>,
        device_extensions_names: Option<&[&str]>,
    ) -> StrResult<Self> {
        let entry = trace_err!(Entry::new())?; // todo store this?

        // dbg!(entry.enumerate_instance_layer_properties().unwrap());

        let bridgevr_c_string = trace_err!(CString::new(BVR_NAME))?;
        let application_info = vk::ApplicationInfo::builder()
            .application_name(bridgevr_c_string.as_c_str()) // todo: remove?
            .application_version(vk_make_version!(1, 0, 0)) // todo: remove?
            .engine_name(bridgevr_c_string.as_c_str()) // todo: remove?
            .engine_version(vk_make_version!(1, 0, 0)) // todo: remove?
            .api_version(vk_make_version!(1, 0, 0));

        let validation_layer_c_string =
            trace_err!(CString::new("VK_LAYER_LUNARG_standard_validation"))?;
        let enabled_layer_names = &[validation_layer_c_string.as_ptr()];

        let instance_extension_c_strings = if let Some(names) = instance_extensions_names {
            let mut name_c_strings = vec![];
            for &name in names {
                name_c_strings.push(trace_err!(CString::new(name))?);
            }
            name_c_strings
        } else {
            required_instance_extension_names()
        };
        let enabled_extension_names = instance_extension_c_strings
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();
        let mut instance_create_info = vk::InstanceCreateInfo::builder()
            .application_info(&application_info)
            .enabled_extension_names(&enabled_extension_names);
        if cfg!(debug_assertions) {
            instance_create_info = instance_create_info.enabled_layer_names(enabled_layer_names);
        }

        let instance = trace_err!(unsafe { entry.create_instance(&instance_create_info, None) })?;

        let surface_loader = khr::Surface::new(&entry, &instance);

        let mut physical_devices = trace_err!(unsafe { instance.enumerate_physical_devices() })?;
        let indexed_physical_device_names = physical_devices
            .iter()
            .map(|&physical_device| unsafe {
                CStr::from_ptr(
                    instance
                        .get_physical_device_properties(physical_device)
                        .device_name
                        .as_ptr(),
                )
            })
            .enumerate()
            .collect::<Vec<_>>();
        dbg!(indexed_physical_device_names);

        let adapter_index = adapter_index.unwrap_or(0);
        let physical_device = if physical_devices.len() > adapter_index {
            physical_devices.remove(adapter_index)
        } else {
            return trace_str!("adapter_index out of bounds");
        };

        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let queue_family_index = trace_none!(queue_families.iter().position(|qf| {
            qf.queue_flags.contains(vk::QueueFlags::GRAPHICS) // todo: remove graphics
                && qf.queue_flags.contains(vk::QueueFlags::COMPUTE)
        }))?;

        let logical_device_extension_c_strings = if let Some(names) = device_extensions_names {
            let mut name_c_strings = vec![];
            for &name in names {
                name_c_strings.push(trace_err!(CString::new(name))?);
            }
            name_c_strings
        } else {
            required_device_extension_names()
        };
        let enabled_extension_names = logical_device_extension_c_strings
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();

        let queue_priorities = [1.0_f32];
        let queue_create_infos = &[vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index as _)
            .queue_priorities(&queue_priorities)
            .build()];
        let mut logical_device_creation_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(queue_create_infos)
            .enabled_extension_names(&enabled_extension_names);
        if cfg!(debug_assertions) {
            logical_device_creation_info =
                logical_device_creation_info.enabled_layer_names(enabled_layer_names);
        }

        let logical_device = trace_err!(unsafe {
            instance.create_device(physical_device, &logical_device_creation_info, None)
        })?;

        let queue = unsafe { logical_device.get_device_queue(queue_family_index as _, 0) };

        Ok(Self {
            entry,
            instance,
            surface_loader,
            physical_device,
            device: logical_device,
            queue,
        })
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
