use bridgevr_common::{data::*, graphics::*, *};
use parking_lot::Mutex;
use std::{
    ffi::CString,
    ptr::{null_mut, NonNull},
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

const TRACE_CONTEXT: &str = "OpenXR client";

// macro_rules! trace_ovr_err {
//     ($res:expr $(, $expect_fmt:expr $(, $args:expr)*)?) => {{
//         if $res != ovrSuccess as i32  {
//             vrapi_Shutdown();
//             trace_err!(Err($res) $(, $expect_fmt $(, $args)*)?)
//         } else {
//             Ok(())
//         }
//     }};
// }

// fn get_vulkan_instance_extensions() -> {
//     const STR_SIZE: usize = 4096;
//             let mut instance_extensions_names = [0u8; STR_SIZE];
//             let mut instance_extension_names_size = STR_SIZE as u32;
//             trace_ovr_err!(vrapi_GetInstanceExtensionsVulkan(
//                 &mut instance_extensions_names[0] as *mut _,
//                 &mut instance_extension_names_size,
//             ))?;

//             // todo create vulkan instance with extensions

//             let mut device_extension_names = [0u8; STR_SIZE];
//             let mut device_extension_names_size = STR_SIZE as u32;
//             trace_ovr_err!(vrapi_GetDeviceExtensionsVulkan(
//                 &mut device_extension_names[0] as *mut _,
//                 &mut device_extension_names_size,
//             ))?;

//             // todo create vulkan device with extensions
// }

pub struct VrClient {
    frame_index: u64,
    display_time: Instant,
    frame_width: u32,
    frame_height: u32,
}

unsafe impl Send for VrClient {}

impl VrClient {
    pub fn new(
        // app: NonNull<ndk::native_app_glue::android_app>,
        graphics: Arc<GraphicsContext>,
    ) -> StrResult<Self> {
        todo!()
    }

    pub fn initialize_for_server(&self, cpu_level: i32, gpu_level: i32) {
        todo!();
    }

    pub fn deinitialize_for_server(&self) {
        todo!();
    }

    pub fn submit_idle_frame(&self) {
        todo!();
    }

    pub fn submit_stream_frame(&self) {
        todo!();
    }

    pub fn native_eye_resolution(&self) -> (u32, u32) {
        todo!();
    }

    pub fn fov(&self) -> [Fov; 2] {
        todo!();
    }

    pub fn fps(&self) -> u32 {
        todo!();
    }

    pub fn poll_input(&self) {
        todo!()
    }
}
