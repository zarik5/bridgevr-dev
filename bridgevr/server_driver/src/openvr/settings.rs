use bridgevr_common::data::*;
use log::*;
use openvr_driver_sys as vr;
use std::{ffi::*, time::*};

pub const TRACE_CONTEXT: &str = "OpenVR";

const DEFAULT_EYE_RESOLUTION: (u32, u32) = (640, 720);

const DEFAULT_FOV: [Fov; 2] = [Fov {
    left: 45_f32,
    top: 45_f32,
    right: 45_f32,
    bottom: 45_f32,
}; 2];

const DEFAULT_BLOCK_STANDBY: bool = false;

// todo: use ::from_secs_f32 if it will be a const fn
const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_nanos((1e9 / 60_f32) as u64);

pub struct OpenvrSettings {
    pub tracked_devices: Vec<OpenvrTrackedDeviceDesc>,
    pub block_standby: bool,
    pub target_eye_resolution: (u32, u32),
    pub fov: [Fov; 2],
    pub frame_interval: Duration,
}

pub fn create_openvr_settings(
    settings: Option<&Settings>,
    session_desc: &SessionDesc,
) -> OpenvrSettings {
    let block_standby;
    let tracked_devices;
    if let Some(settings) = settings {
        block_standby = settings.openvr.block_standby;
        tracked_devices = settings.openvr.tracked_devices.clone();
    } else {
        block_standby = DEFAULT_BLOCK_STANDBY;
        tracked_devices = vec![];
    };

    let fov;
    let frame_interval;
    if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
        fov = client_handshake_packet.fov;
        frame_interval = Duration::from_secs_f32(1_f32 / client_handshake_packet.fps as f32);
    } else {
        fov = DEFAULT_FOV;
        frame_interval = DEFAULT_FRAME_INTERVAL;
    };

    let target_eye_resolution = if let Some(Settings {
        openvr:
            OpenvrDesc {
                preferred_render_eye_resolution: Some(eye_res),
                ..
            },
        ..
    }) = settings
    {
        *eye_res
    } else if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
        client_handshake_packet.native_eye_resolution
    } else {
        DEFAULT_EYE_RESOLUTION
    };

    OpenvrSettings {
        tracked_devices,
        block_standby,
        target_eye_resolution,
        fov,
        frame_interval,
    }
}

pub fn set_custom_props(
    container: vr::PropertyContainerHandle_t,
    props: &[(String, OpenvrPropValue)],
) {
    for (prop_name, value) in props {
        match vr::tracked_device_property_name_to_u32(prop_name) {
            Ok(code) => {
                let res = unsafe {
                    match value {
                        OpenvrPropValue::Bool(value) => {
                            vr::vrSetBoolProperty(container, code as _, *value)
                        }
                        OpenvrPropValue::Int32(value) => {
                            vr::vrSetInt32Property(container, code as _, *value)
                        }
                        OpenvrPropValue::Uint64(value) => {
                            vr::vrSetUint64Property(container, code as _, *value)
                        }
                        OpenvrPropValue::Float(value) => {
                            vr::vrSetFloatProperty(container, code as _, *value)
                        }
                        OpenvrPropValue::String(value) => {
                            // unwrap never fails
                            let c_string = CString::new(value.clone()).unwrap();
                            vr::vrSetStringProperty(container, code as _, c_string.as_ptr())
                        }
                        OpenvrPropValue::Vector3(value) => vr::vrSetVec3Property(
                            container,
                            code as _,
                            &vr::HmdVector3_t { v: *value },
                        ),
                    }
                };

                if res > 0 {
                    warn!(
                        "Failed to set openvr property {} with code={}",
                        prop_name, res
                    );
                }
            }
            Err(e) => warn!("{}", e),
        }
    }
}
