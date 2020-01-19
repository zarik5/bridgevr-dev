use super::settings::*;
use bridgevr_common::data::*;
use log::*;
use openvr_driver_sys as vr;
use parking_lot::Mutex;
use std::{collections::HashMap, ffi::*, os::raw::*, ptr, sync::Arc};

const HAPTIC_PATH: &str = "/output/haptic";

pub struct ControllerContext {
    pub index: usize, // 0: left, 1: right
    pub id: Mutex<Option<u32>>,
    pub settings: Arc<Mutex<OpenvrSettings>>,
    pub pose: Mutex<vr::DriverPose_t>,
    pub controller_input_to_component_map: Mutex<HashMap<String, vr::VRInputComponentHandle_t>>,
    pub haptic_component: Mutex<vr::VRInputComponentHandle_t>,
}

unsafe extern "C" fn activate(context: *mut c_void, object_id: u32) -> vr::EVRInitError {
    let context = context as *const ControllerContext;

    *(*context).id.lock() = Some(object_id);
    let container = vr::vrTrackedDeviceToPropertyContainer(object_id);

    //todo: set default props

    set_custom_props(
        container,
        &(*context).settings.lock().controllers_custom_properties[(*context).index],
    );

    let mut component_map = (*context).controller_input_to_component_map.lock();
    for (path, input_type, controller_paths) in
        &(*context).settings.lock().input_mapping[(*context).index]
    {
        let path_c_string = CString::new(path.clone()).unwrap();
        let mut component = vr::k_ulInvalidInputComponentHandle;
        let res = match input_type {
            InputType::Boolean => vr::vrDriverInputCreateBooleanComponent(
                container,
                path_c_string.as_ptr(),
                &mut component,
            ),
            InputType::NormalizedOneSided => vr::vrDriverInputCreateScalarComponent(
                container,
                path_c_string.as_ptr(),
                &mut component,
                vr::VRScalarType_Absolute,
                vr::VRScalarUnits_NormalizedOneSided,
            ),
            InputType::NormalizedTwoSided => vr::vrDriverInputCreateScalarComponent(
                container,
                path_c_string.as_ptr(),
                &mut component,
                vr::VRScalarType_Absolute,
                vr::VRScalarUnits_NormalizedTwoSided,
            ),
            _ => todo!(),
        };
        if res == 0 {
            for controller_path in controller_paths {
                component_map.insert(controller_path.to_owned(), component);
            }
        } else {
            warn!("Create {}: {}", path, res);
        }
    }

    let haptic_path_c_string = CString::new(HAPTIC_PATH).unwrap();
    let mut component = vr::k_ulInvalidInputComponentHandle;
    let res = vr::vrDriverInputCreateHapticComponent(
        container,
        haptic_path_c_string.as_ptr(),
        &mut component,
    );
    if res == 0 {
        *(*context).haptic_component.lock() = component;
    } else {
        warn!("Create {}: {}", HAPTIC_PATH, res);
    }

    vr::VRInitError_None
}

unsafe extern "C" fn deactivate(context: *mut c_void) {
    let context = context as *const ControllerContext;

    *(*context).id.lock() = None;
}

extern "C" fn empty_fn(_: *mut c_void) {}

extern "C" fn get_component(_: *mut c_void, _: *const c_char) -> *mut c_void {
    ptr::null_mut()
}

extern "C" fn debug_request(_: *mut c_void, _: *const c_char, _: *mut c_char, _: u32) {
    // format!("debug request: {}", request)
}

unsafe extern "C" fn get_pose(context: *mut c_void) -> vr::DriverPose_t {
    let context = context as *const ControllerContext;

    *(*context).pose.lock()
}

pub fn create_controller_callbacks(
    controller_context: Arc<ControllerContext>,
) -> vr::TrackedDeviceServerDriverCallbacks {
    vr::TrackedDeviceServerDriverCallbacks {
        context: &*controller_context as *const _ as _,
        Activate: Some(activate),
        Deactivate: Some(deactivate),
        EnterStandby: Some(empty_fn),
        GetComponent: Some(get_component),
        DebugRequest: Some(debug_request),
        GetPose: Some(get_pose),
    }
}
