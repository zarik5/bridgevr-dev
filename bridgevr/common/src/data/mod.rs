#![allow(clippy::large_enum_variant)]

mod settings;
mod constants;

use crate::*;
use bitflags::bitflags;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json as json;
use std::{fs, hash::*, path::*};

pub use settings::*;
pub use constants::*;

#[derive(Serialize, Deserialize, Clone)]
pub struct MotionSample3DofDesc {
    pub default_position: [f32; 3],
    pub orientation: [f32; 4],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub linear_acceleration: [f32; 3],
    pub angular_acceleration: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MotionSample6DofDesc {
    pub pose: Pose,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone)]
pub enum MotionSampleDesc {
    Dof3(MotionSample3DofDesc),
    Dof6(MotionSample6DofDesc),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClientHandshakePacket {
    pub bridgevr_name: String,
    pub version: String,
    pub native_eye_resolution: (u32, u32),
    pub fov: [Fov; 2],
    pub fps: u32,
    pub max_video_encoder_instances: u8,
    pub available_audio_player_sample_rates: Vec<u32>,
    pub preferred_audio_player_sample_rates: u32,
    pub available_microphone_sample_rates: Vec<u32>,
    pub preferred_microphone_sample_rates: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub version: String,
    pub target_eye_resolution: (u32, u32),
}

#[derive(Serialize, Deserialize)]
pub struct ServerHandshakePacket {
    pub config: ServerConfig,
    pub settings: Settings,
}

#[derive(Serialize, Deserialize)]
pub struct VideoPacket<'a> {
    pub nal_index: u64,
    pub sub_nal_index: u8,
    pub sub_nal_count: u8,
    pub hmd_pose: Pose,
    pub sub_nal: &'a [u8],
}

// Since BridgeVR does not attempt at any clock synchronization, sending a timestamp is useless
#[derive(Serialize, Deserialize)]
pub struct AudioPacket<'a> {
    // unfortunately serde does not support slice formats other than u8
    pub samples: &'a [u8],
}

#[derive(Serialize, Deserialize)]
pub struct HapticSample {
    pub duration_seconds: f32,
    pub frequency: f32,
    pub amplitude: f32,
}

#[derive(Serialize, Deserialize)]
pub enum OtherServerPacket {
    Haptic {
        device_type: TrackedDeviceType,
        sample: HapticSample,
    },
    Shutdown,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceMotionDesc {
    pub device_type: TrackedDeviceType,
    pub sample: MotionSampleDesc,
    pub timestamp_ns: u64,
}

bitflags! {
    // Target: XBox controller
    #[derive(Serialize, Deserialize)]
    pub struct GamepadDigitalInput: u16 {
        const A = 0x0001;
        const B = 0x0002;
        const X = 0x0004;
        const Y = 0x0008;
        const DPAD_LEFT = 0x0010;
        const DPAD_RIGHT = 0x0020;
        const DPAD_UP = 0x0040;
        const DPAD_DOWN = 0x0080;
        const JOYSTICK_LEFT_CLICK = 0x0100;
        const JOYSTICK_RIGHT_CLICK = 0x0200;
        const SHOULDER_LEFT = 0x0400;
        const SHOULDER_RIGHT = 0x0800;
        const MENU = 0x1000;
        const VIEW = 0x2000;
        const HOME = 0x4000;
    }
}

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct OculusTouchDigitalInput: u16 {
        const A_CLICK = 0x0001;
        const A_TOUCH = 0x0002;
        const B_CLICK = 0x0004;
        const B_TOUCH = 0x0008;
        const X_CLICK = 0x0010;
        const X_TOUCH = 0x0020;
        const Y_CLICK = 0x0040;
        const Y_TOUCH = 0x0080;
        const THUMBSTICK_LEFT_CLICK = 0x0100;
        const THUMBSTICK_LEFT_TOUCH = 0x0200;
        const THUMBSTICK_RIGHT_CLICK = 0x0400;
        const THUMBSTICK_RIGHT_TOUCH = 0x0800;
        const TRIGGER_LEFT_TOUCH = 0x1000;
        const TRIGGER_RIGHT_TOUCH = 0x2000;
        const MENU = 0x4000;
        const HOME = 0x8000;
    }
}

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct OculusGoDigitalInput: u8 {
        const TOUCHPAD_CLICK = 0x01;
        const TOUCHPAD_TOUCH = 0x02;
        const BACK = 0x04;
        const HOME = 0x08;
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum InputDeviceData {
    Gamepad {
        thumbstick_left_horizontal: f32,
        thumbstick_left_vertical: f32,
        thumbstick_right_horizontal: f32,
        thumbstick_right_vertical: f32,
        trigger_left: f32,
        trigger_right: f32,
        digital_input: GamepadDigitalInput,
    },
    OculusTouchPair {
        thumbstick_left_horizontal: f32,
        thumbstick_left_vertical: f32,
        thumbstick_right_horizontal: f32,
        thumbstick_right_vertical: f32,
        trigger_left: f32,
        trigger_right: f32,
        grip_left: f32,
        grip_right: f32,
        digital_input: OculusTouchDigitalInput,
    },
    OculusGoController {
        trigger: f32,
        touchpad_horizontal: f32,
        touchpad_vertical: f32,
        digital_input: OculusGoDigitalInput,
    },
    OculusHands([Vec<MotionSampleDesc>; 2]),
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientStatistics {}

#[derive(Serialize, Deserialize)]
pub enum OtherClientPacket {
    MotionAndTiming {
        device_motions: Vec<DeviceMotionDesc>,
        virtual_vsync_offset_ns: i32,
    },
    InputDeviceData {
        data: InputDeviceData,
        timestamp_ns: u64,
    },
    Statistics(ClientStatistics),
    Disconnected,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SessionDesc {
    pub bitrate: Option<u32>,
    pub last_client_handshake_packet: Option<ClientHandshakePacket>,

    // managed by GUI
    pub settings_cache: serde_json::Value,
}

pub struct SessionDescLoader {
    session_desc: SessionDesc,
    path: PathBuf,
}

impl SessionDescLoader {
    pub fn load(path: &Path) -> Self {
        let session_desc = if let Ok(file_content) = fs::read_to_string(path) {
            json::from_str(&file_content).unwrap_or_else(|_| {
                warn!("Invalid session file. Using default values.");
                <_>::default()
            })
        } else {
            warn!("Session file not found or inaccessible. Using default values.");
            <_>::default()
        };

        Self {
            session_desc,
            path: PathBuf::from(path),
        }
    }

    pub fn get_mut(&mut self) -> &mut SessionDesc {
        &mut self.session_desc
    }

    pub fn save(&self) -> StrResult {
        const TRACE_CONTEXT: &str = "Session";
        trace_err!(fs::write(
            &self.path,
            trace_err!(json::to_string_pretty(&self.session_desc))?
        ))
    }
}
