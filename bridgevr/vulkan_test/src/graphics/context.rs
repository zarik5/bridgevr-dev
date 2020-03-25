use ash::version::InstanceV1_0;
use ash::{
    extensions::*,
    version::{DeviceV1_0, EntryV1_0},
    *,
};
use bridgevr_common::{constants::*, *};
use std::ffi::{c_void, CStr, CString};

pub(super) const TRACE_CONTEXT: &str = "Graphics";

const VALIDATION_LAYER: &str = "VK_LAYER_LUNARG_standard_validation";

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    let message = CStr::from_ptr((*p_callback_data).p_message);
    println!(
        "Vulkan: [{:?}] [{:?}] {:?}",
        message_severity, message_types, message
    );
    vk::FALSE
}

pub struct GraphicsContext {
    _entry: Entry, // this must outlive `instance`
    pub(super) instance: Instance,
    debug_utils_loader: ext::DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub(super) device: Device,
    pub(super) queue_family_index: u32,
    pub(super) queue: vk::Queue,
}

impl GraphicsContext {
    pub fn new(
        adapter_index: Option<usize>,
        instance_extensions_names: &[&str],
        device_extensions_names: &[&str],
    ) -> StrResult<Self> {
        let entry = trace_err!(Entry::new())?;

        // unwrap never fails
        let bridgevr_c_string = CString::new(BVR_NAME).unwrap();
        let application_info = vk::ApplicationInfo::builder()
            .application_name(bridgevr_c_string.as_c_str()) // todo: remove?
            .application_version(vk_make_version!(1, 0, 0)) // todo: remove?
            .engine_name(bridgevr_c_string.as_c_str()) // todo: remove?
            .engine_version(vk_make_version!(1, 0, 0)) // todo: remove?
            .api_version(vk_make_version!(1, 0, 0));

        // unwrap never fails
        let validation_layer_c_string = CString::new(VALIDATION_LAYER).unwrap();
        let enabled_layer_names = &[validation_layer_c_string.as_ptr()];

        let mut instance_extension_c_strings = vec![];
        for &name in instance_extensions_names {
            // unwrap never fails
            instance_extension_c_strings.push(CString::new(name).unwrap());
        }
        if cfg!(debug_assertions) {
            instance_extension_c_strings.push(ext::DebugUtils::name().to_owned());
        }
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

        let debug_utils_loader = ext::DebugUtils::new(&entry, &instance);
        let debug_messenger = if cfg!(debug_assertions) {
            let debug_messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                        | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                )
                .pfn_user_callback(Some(debug_callback));

            trace_err!(unsafe {
                debug_utils_loader.create_debug_utils_messenger(&debug_messenger_create_info, None)
            })?
        } else {
            vk::DebugUtilsMessengerEXT::null()
        };

        let mut physical_devices = trace_err!(unsafe { instance.enumerate_physical_devices() })?;

        let adapter_index = adapter_index.unwrap_or(0);
        let physical_device = if physical_devices.len() > adapter_index {
            physical_devices.remove(adapter_index)
        } else {
            return trace_str!("adapter_index out of bounds");
        };

        let memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let queue_family_index = trace_none!(queue_families
            .iter()
            .position(|qf| { qf.queue_flags.contains(vk::QueueFlags::COMPUTE) }))?
            as _;

        let mut logical_device_extension_c_strings = vec![];
        for &name in device_extensions_names {
            // unwrap never fails
            logical_device_extension_c_strings.push(CString::new(name).unwrap());
        }
        let enabled_extension_names = logical_device_extension_c_strings
            .iter()
            .map(|s| s.as_ptr())
            .collect::<Vec<_>>();

        let queue_priorities = [1.0_f32]; // if multithreading, add queues here
        let queue_create_infos = &[vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
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

        let queue = unsafe { logical_device.get_device_queue(queue_family_index, 0) };

        Ok(Self {
            _entry: entry,
            instance,
            debug_utils_loader,
            debug_messenger,
            physical_device,
            memory_properties,
            device: logical_device,
            queue_family_index,
            queue,
        })
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);

            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}
