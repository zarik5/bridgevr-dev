use super::settings::*;
use crate::shutdown_signal::ShutdownSignal;
use bridgevr_common::data::*;
use log::*;
use openvr_driver_sys as vr;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ffi::*,
    os::raw::*,
    ptr,
    sync::{mpsc::*, Arc},
};

const HAPTIC_PATH: &str = "/output/haptic";

pub struct TrackedDeviceContext {
    pub device_type: TrackedDeviceType,
    pub object_id: Mutex<Option<u32>>,
    pub settings: Arc<Mutex<OpenvrSettings>>,
    pub pose: Mutex<vr::DriverPose_t>,
    pub input_to_component_map: Mutex<HashMap<String, vr::VRInputComponentHandle_t>>,
    pub haptic_component: Mutex<vr::VRInputComponentHandle_t>,
    pub shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
}

pub extern "C" fn activate(context: *mut c_void, object_id: u32) -> vr::EVRInitError {
    let context = unsafe { &*(context as *const TrackedDeviceContext) };

    *context.object_id.lock() = Some(object_id);
    let container = unsafe { vr::vrTrackedDeviceToPropertyContainer(object_id) };

    match context.device_type {
        TrackedDeviceType::HMD => {
            //todo
        }
        TrackedDeviceType::LeftController => {
            //todo
        }
        TrackedDeviceType::RightController => {
            //todo
        }
        _ => {
            //todo
        }
    }

    if let Some(tracked_device_desc) = context
        .settings
        .lock()
        .tracked_devices
        .iter()
        .find(|td| td.device_type == context.device_type)
    {
        set_custom_props(container, &tracked_device_desc.properties);

        let mut component_map_ref = context.input_to_component_map.lock();
        for (openvr_path, input_type, client_paths) in &tracked_device_desc.input_mapping {
            // unwrap never fails
            let openvr_path_c_string = CString::new(openvr_path.clone()).unwrap();
            let mut component = vr::k_ulInvalidInputComponentHandle;
            let res = unsafe {
                match input_type {
                    OpenvrInputType::Boolean => vr::vrDriverInputCreateBooleanComponent(
                        container,
                        openvr_path_c_string.as_ptr(),
                        &mut component,
                    ),
                    OpenvrInputType::NormalizedOneSided => vr::vrDriverInputCreateScalarComponent(
                        container,
                        openvr_path_c_string.as_ptr(),
                        &mut component,
                        vr::VRScalarType_Absolute,
                        vr::VRScalarUnits_NormalizedOneSided,
                    ),
                    OpenvrInputType::NormalizedTwoSided => vr::vrDriverInputCreateScalarComponent(
                        container,
                        openvr_path_c_string.as_ptr(),
                        &mut component,
                        vr::VRScalarType_Absolute,
                        vr::VRScalarUnits_NormalizedTwoSided,
                    ),
                    OpenvrInputType::Skeletal => todo!(),
                }
            };
            if res == 0 {
                for path in client_paths {
                    component_map_ref.insert(path.to_owned(), component);
                }
            } else {
                warn!("Create {}: {}", openvr_path, res);
            }
        }

        // unwrap never fails
        let haptic_path_c_string = CString::new(HAPTIC_PATH).unwrap();
        let mut component = vr::k_ulInvalidInputComponentHandle;
        let res = unsafe {
            vr::vrDriverInputCreateHapticComponent(
                container,
                haptic_path_c_string.as_ptr(),
                &mut component,
            )
        };
        if res == 0 {
            *context.haptic_component.lock() = component;
        } else {
            warn!("Create {}: {}", HAPTIC_PATH, res);
        }
    }

    vr::VRInitError_None
}

pub extern "C" fn deactivate(context: *mut c_void) {
    let context = unsafe { &*(context as *const TrackedDeviceContext) };

    *context.object_id.lock() = None;

    context
        .shutdown_signal_sender
        .lock()
        .send(ShutdownSignal::BackendShutdown)
        .ok();
}

pub extern "C" fn empty_fn(_: *mut c_void) {}

extern "C" fn get_component(_: *mut c_void, _: *const c_char) -> *mut c_void {
    ptr::null_mut()
}

pub extern "C" fn debug_request(_: *mut c_void, _: *const c_char, _: *mut c_char, _: u32) {
    // format!("debug request: {}", request)
}

pub extern "C" fn get_pose(context: *mut c_void) -> vr::DriverPose_t {
    let context = unsafe { &*(context as *const TrackedDeviceContext) };

    *context.pose.lock()
}

pub fn create_tracked_device_callbacks(
    tracked_device_context: Arc<TrackedDeviceContext>,
) -> vr::TrackedDeviceServerDriverCallbacks {
    vr::TrackedDeviceServerDriverCallbacks {
        context: &*tracked_device_context as *const _ as _,
        Activate: Some(activate),
        Deactivate: Some(deactivate),
        EnterStandby: Some(empty_fn),
        GetComponent: Some(get_component),
        DebugRequest: Some(debug_request),
        GetPose: Some(get_pose),
    }
}
