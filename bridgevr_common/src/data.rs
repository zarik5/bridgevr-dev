// WARNING: never use usize in in packets because its size is hardware dependent and deserialization
// can fail

use crate::{constants::Version, *};
use bitflags::bitflags;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json as json;
use std::{fs, hash::*, path::*};

#[derive(Serialize, Deserialize, Clone)]
pub enum Switch<T> {
    Enabled(T),
    Disabled,
}

impl<T> Switch<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Enabled(t) => Some(t),
            Self::Disabled => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Default)]
pub struct Fov {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

// Row major 3x4 matrix
#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Pose {
    orientation: [f32; 4],
    position: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MotionDesc {
    pose: Pose,
    linear_velocity: [f32; 3],
    angular_velocity: [f32; 3],
}

// #[derive(Serialize, Deserialize, Clone)]

// pub enum Preset {
//     HighPerformance,
//     Default,
//     HighQuality,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub struct QP {
//     pub i: u32,
//     pub p: u32,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub enum RateControlMode {
//     // better quality but it can cause lag spikes (or increase overall latency if latency control is on)
//     ConstantQP(Option<QP>),
//     VBR {
//         max_bitrate: Option<u32>,
//         initial_qp: Option<Switch<QP>>,
//         min_qp: Option<Switch<QP>>,
//         max_qp: Option<Switch<QP>>,
//         target_quality: Option<u8>,
//     },
//     CBR,
//     // preferred
//     LowDelayCBR,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub struct AQ {
//     pub enable_spatial: bool,
//     pub enable_temporal: bool,
//     pub strength: u32,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub struct RateControlParams {
//     pub rate_control_mode: RateControlMode,
//     pub bitrate_k: Option<u32>,
//     pub vbv_buffer_size: Option<u32>,
//     pub vbv_initial_delay: Option<u32>,
//     pub aq: Option<AQ>,
//     pub zero_latency: Option<bool>,
//     pub enable_non_ref_p: Option<bool>,
//     pub strict_gop_target: Option<bool>,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub struct IntraRefresh {
//     pub period: u32,
//     pub count: u32,
// }

// #[derive(Serialize, Deserialize, Clone)]
// #[serde(tag = "type")]
// pub enum SliceMode {
//     MBs { mb_per_slice: u32 },
//     Bytes { max_bytes_per_slice: u32 },
//     MBRows { num_rows: u32 },
//     Slices { num_slices: u32 },
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub enum ChromaFormat {
//     YUV420,
//     YUV444,
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub enum LumaSize {
//     SizeAuto,
//     Size8x8,
//     Size16x16,
//     Size32x32,
//     Size64x64,
// }

// #[derive(Serialize, Deserialize, Clone)]
// #[serde(tag = "type")]
// pub enum Codec {
//     H264 {
//         preset: Preset,
//         rc_params: RateControlParams,
//         hierarchical_p_frames: Option<bool>,
//         intra_refresh: Option<Switch<IntraRefresh>>,
//         level: Option<u32>,
//         disable_deblocking_filter_idc: Option<bool>,
//         num_temporal_layers: Option<u32>,
//         adaptive_transform_mode: Option<u32>,
//         entropy_coding_mode: Option<u32>,
//         max_num_ref_frames: Option<u32>,
//         slice_mode: Option<SliceMode>,
//         chroma_format: Option<ChromaFormat>,
//         max_temporal_layers: Option<u32>,
//     },
//     HEVC {
//         preset: Preset,
//         rc_params: RateControlParams,
//         level: Option<u32>,
//         tier: Option<u32>,
//         min_luma_size: LumaSize,
//         max_luma_size: LumaSize,
//         disable_deblock_across_slice_boundary: Option<bool>,
//         intra_refresh: Option<Switch<IntraRefresh>>,
//         chroma_format: Option<ChromaFormat>,
//         max_num_ref_frames_in_dpb: Option<u32>,
//         slice_mode: Option<SliceMode>,
//         max_temporal_layers_minus_1: Option<u32>,
//     },
// }

// #[derive(Serialize, Deserialize, Clone)]
// pub struct NvidiaEncoder {
//     pub codec: Codec,
// }

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum FrameSize {
    Scale(f32),
    Absolute { width: u32, height: u32 },
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum LatencyDesc {
    Automatic {
        expected_missed_poses_per_hour: u32,
        expected_missed_frames_per_hour: u32,
        server_history_mean_lifetime_s: u32,
        client_history_mean_lifetime_s: u32,
    },
    Manual {
        ms: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum BitrateDesc {
    Automatic {
        default_mbps: u32,
        expected_lost_frame_per_hour: u32,
        history_seconds: u32,
        packet_loss_bitrate_factor: f32,
    },
    Manual {
        mbps: u32,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectionDesc {
    pub client_ip: Option<String>,
    pub starting_data_port: u16,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoEncoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoDecoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CompositionFilteringType {
    NearestNeighbour,
    Bilinear,
    Lanczos,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct FoveatedRenderingDesc {
    strength: f32,
    vertical_offset: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VideoDesc {
    pub frame_size: FrameSize,
    pub composition_filtering: CompositionFilteringType,
    pub foveated_rendering: Switch<FoveatedRenderingDesc>,
    pub slice_count: u64,
    pub encoder: VideoEncoderDesc,
    pub decoder: VideoDecoderDesc,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct MicrophoneDesc {
    pub client_device_index: Option<u64>,
    pub server_device_index: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioDesc {
    pub loopback_device_index: Switch<Option<u64>>,
    pub microphone: Switch<MicrophoneDesc>,
    pub max_packet_size: u64,
    pub max_latency_ms: u64, // if set too low the audio becomes choppy
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CompositorType {
    // (default) Use DirectModeDriver interface
    // cons:
    // * supperted limited number of color formats
    // * there can be some glitches with head orientation when more than one layer is submitted
    Custom,
    // Use  VirtualDisplay interface.
    // pro: none of Custom mode cons.
    // cons: tiny bit more latency, potential lower image quality
    SteamVR,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OpenvrPropValue {
    Bool(bool),
    Int32(i32),
    Uint64(u64),
    Float(f32),
    String(String),
    Vector3([f32; 3]),
    Matrix34([f32; 12]),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenvrProp {
    pub code: u32,
    pub value: OpenvrPropValue,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenvrDesc {
    pub block_standby: bool,
    pub timeout_seconds: u64,
    pub input_mapping: Vec<(String, Vec<String>)>,
    pub compositor_type: CompositorType,
    pub preferred_render_eye_resolution: Option<(u32, u32)>,
    pub hmd_custom_properties: Vec<OpenvrProp>,
    pub controllers_custom_properties: Vec<OpenvrProp>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OculusGoDesc {
    default_controller_poses: (Pose, Pose),
    openvr_rotation_only_fallback: bool,
    eye_level_height_meters: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HeadsetsDesc {
    oculus_go: OculusGoDesc,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub connection: ConnectionDesc,
    pub latency: LatencyDesc,
    pub bitrate: BitrateDesc,
    pub video: VideoDesc,
    pub audio: AudioDesc,
    pub openvr: OpenvrDesc,
    pub headsets: HeadsetsDesc,
}

pub fn load_settings(path: &str) -> StrResult<Settings> {
    const TRACE_CONTEXT: &str = "Settings";
    trace_err!(json::from_str(&trace_err!(fs::read_to_string(path))?))
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

#[derive(Serialize, Deserialize, Clone)]
pub struct OculusTouchAnalogInput {
    motion: MotionDesc,
    thumbstick_horizontal: f32,
    thumbstick_vertical: f32,
    trigger: f32,
    grip: f32,
}

#[derive(Serialize, Deserialize, Clone)]
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
        motion: MotionDesc,
        trigger: f32,
        touchpad_horizontal: f32,
        touchpad_vertical: f32,
        is_right_hand: bool,
        digital_input: OculusGoDigitalInput,
    },
    OculusHand(Vec<MotionDesc>),
    GenericTracker(MotionDesc),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClientHandshakePacket {
    pub bridgevr_name: String,
    pub version: Version,
    pub native_eye_resolution: (u32, u32),
    pub fov: [Fov; 2],

    // this is used to determine type and count of input devices
    pub input_devices_initial_data: Vec<InputDeviceData>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientStatistics {}

#[derive(Serialize, Deserialize)]
pub struct ServerHandshakePacket {
    pub version: Version,
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
    Input {
        hmd_motion: MotionDesc,
        devices_data: Vec<InputDeviceData>,
        additional_vsync_offset_ns: i32,
    },
    Statistics(ClientStatistics),
    Disconnected,
}

#[derive(Serialize, Deserialize)]
pub struct VideoPacketHeader {
    pub sub_nal_idx: u8,
    pub sub_nal_count: u8,
    pub hmd_pose: Pose,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SessionDesc {
    pub bitrate: Option<u32>,
    pub last_client_handshake_packet: Option<ClientHandshakePacket>,

    // don't care
    pub settings_cache: serde_json::Value,
}

pub struct SessionDescLoader {
    session_desc: SessionDesc,
    path: PathBuf,
}

impl SessionDescLoader {
    pub fn load(path: &str) -> Self {
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

    pub fn save(&self) -> StrResult<()> {
        const TRACE_CONTEXT: &str = "Session";
        trace_err!(fs::write(
            &self.path,
            trace_err!(json::to_string_pretty(&self.session_desc))?
        ))
    }
}

pub fn load_session_desc(path: &str) -> SessionDesc {
    if let Ok(file_content) = fs::read_to_string(path) {
        json::from_str(&file_content).unwrap_or_else(|_| {
            warn!("Invalid session file. Using default values.");
            <_>::default()
        })
    } else {
        warn!("Session file not found or inaccessible. Using default values.");
        <_>::default()
    }
}

pub fn save_session_desc(path: &str, session: &SessionDesc) -> StrResult<()> {
    const TRACE_CONTEXT: &str = "Session";
    trace_err!(fs::write(
        path,
        trace_err!(json::to_string_pretty(session))?
    ))
}