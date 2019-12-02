// WARNING: never use usize in in packets because its size is hardware dependent and deserialization
// can fail

use crate::settings::*;
use bitflags::bitflags;
use serde::*;
use std::hash::*;

#[derive(Serialize, Deserialize, PartialEq)]
pub struct Pose(pub [f32; 16]);

#[derive(Serialize, Deserialize, Clone)]
pub struct Fov {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
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
        const DPAD_RIFHT = 0x0020;
        const DPAD_UP = 0x0040;
        const DPAD_DOWN = 0x0080;
        const JOYSTICK_LEFT_PRESS = 0x0100;
        const JOYSTICK_RIGHT_PRESS = 0x0200;
        const MENU = 0x0400;
        const VIEW = 0x0800;
        const HOME = 0x1000;
    }
}

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct OculusTouchDigitalInput: u16 {
        const A_PRESS = 0x0001;
        const A_TOUCH = 0x0002;
        const B_PRESS = 0x0004;
        const B_TOUCH = 0x0008;
        const X_PRESS = 0x0010;
        const X_TOUCH = 0x0020;
        const Y_PRESS = 0x0040;
        const Y_TOUCH = 0x0080;
        const THUMBSTICK_LEFT_PRESS = 0x0100;
        const THUMBSTICK_LEFT_TOUCH = 0x0200;
        const THUMBSTICK_RIGHT_PRESS = 0x0400;
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
        const TOUCHPAD_PRESS = 0x01;
        const TOUCHPAD_TOUCH = 0x02;
        const BACK = 0x04;
        const HOME = 0x08;
    }
}

#[derive(Serialize, Deserialize)]
pub struct OculusTouchAnalogInput {
    pose: Pose,
    thumbstick_horizontal: f32,
    thumbstick_vertical: f32,
    trigger: f32,
    grip: f32,
}

#[derive(Serialize, Deserialize)]
pub enum InputDeviceData {
    Gamepad {
        thumbstick_left_horizontal: f32,
        thumbstick_left_vertical: f32,
        thumbstick_right_horizontal: f32,
        thumbstick_right_vertical: f32,
        shoulder_left: f32,
        shoulder_right: f32,
        digital_input: GamepadDigitalInput,
    },
    OculusTouchPair {
        analog_input: [OculusTouchAnalogInput; 2],
        digital_input: OculusTouchDigitalInput,
    },
    OculusGoController {
        pose: Pose,
        trigger: f32,
        touchpad_horizontal: f32,
        touchpad_vertical: f32,
        digital_input: OculusGoDigitalInput,
    },
    OculusHand([Pose; 22]),
    GenericTracker(Pose),
}

#[derive(Serialize, Deserialize)]
pub struct ClientStatistics {}

#[derive(Serialize, Deserialize)]
pub struct ClientHandshakePacket {
    pub client_version: u32,
    pub native_eye_width: u32,
    pub native_eye_height: u32,
    pub fov: [Fov; 2],

    // this is used to determine type and count of input devices
    pub input_devices_initial_data: Vec<InputDeviceData>,
}

#[derive(Serialize, Deserialize)]
pub struct ServerHandshakePacket {
    pub server_version: u32,
    pub settings: Settings,
    pub target_eye_width: u32,
    pub target_eye_height: u32,
}

// Messages are packets without an associated buffer and are not zero-copy
#[derive(Serialize, Deserialize)]
pub enum ServerMessage {
    Shutdown,
}

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    HmdAndControllers {
        // input
        hmd_pose: Pose,
        input_devices_data: Vec<InputDeviceData>,

        // timing
        additional_vsync_offset_ns: i32,
    },
    Statistics(ClientStatistics),
    Shutdown,
}

pub struct VideoPacketHeader {
    pub sub_nal_idx: u8,
    pub sub_nal_count: u8,
}

// audio packets can be subdiveded indefinitely, no metadata required
// pub struct AudioPacketHeader {}
