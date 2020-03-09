// code adapted from VrCubeWorld_Vulkan.c

use android_ndk_sys as ndk;
use bridgevr_common::{data::*, graphics::*, *};
use ovr_mobile_sdk_sys::*;
use parking_lot::Mutex;
use std::{
    ffi::CString,
    ptr::{null_mut, NonNull},
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

const TRACE_CONTEXT: &str = "Oculus VR client";

macro_rules! trace_ovr_err {
    ($res:expr $(, $expect_fmt:expr $(, $args:expr)*)?) => {{
        if $res != ovrSuccess as i32  {
            vrapi_Shutdown();
            trace_err!(Err($res) $(, $expect_fmt $(, $args)*)?)
        } else {
            Ok(())
        }
    }};
}

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
    ovr_java: ovrJava,
    ovr_context: *mut ovrMobile,
    frame_index: u64,
    display_time: Instant,
    frame_width: u32,
    frame_height: u32,
    swap_chain: *mut ovrTextureSwapChain,
}

unsafe impl Send for VrClient {}

impl VrClient {
    pub fn new(
        app: NonNull<ndk::native_app_glue::android_app>,
        graphics: Arc<GraphicsContext>,
    ) -> StrResult<Self> {
        let ovr_java;
        unsafe {
            ndk::ANativeActivity_setWindowFlags(
                app.as_ref().activity,
                ndk::AWINDOW_FLAG_KEEP_SCREEN_ON,
                0,
            );

            let java_vm = (*app.as_ref().activity).vm;
            let mut jni_env: *mut ndk::JNIEnv = null_mut();
            (**java_vm).AttachCurrentThread.unwrap()(java_vm, &mut jni_env, null_mut());
            ovr_java = ovrJava {
                Vm: java_vm as _,
                Env: jni_env as _,
                ActivityObject: (*app.as_ref().activity).clazz,
            };

            //todo: change current thread name to "OVR::Main"?

            let init_parms = ovrInitParms {
                Type: VRAPI_STRUCTURE_TYPE_INIT_PARMS,
                ProductVersion: VRAPI_PRODUCT_VERSION as _,
                MajorVersion: VRAPI_MAJOR_VERSION as _,
                MinorVersion: VRAPI_MINOR_VERSION as _,
                PatchVersion: VRAPI_PATCH_VERSION as _,
                GraphicsAPI: VRAPI_GRAPHICS_API_VULKAN_1,
                Java: ovr_java,
            };
            trace_ovr_err!(vrapi_Initialize(&init_parms))?;

            let mut ovr_vulkan_info = ovrSystemCreateInfoVulkan {
                Instance: null_mut(),       // todo
                PhysicalDevice: null_mut(), // todo
                Device: null_mut(),         // todo
            };
            // this call must be after vrapi_Initialize
            trace_ovr_err!(vrapi_CreateSystemVulkan(&mut ovr_vulkan_info))?;
        }

        Ok(Self {
            ovr_java,
            ovr_context: null_mut(),
            frame_index: 1,
            display_time: Instant::now(), // 0?
            frame_width: 0,               //todo
            frame_height: 0,              // todo
            swap_chain: null_mut(),
        })
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
