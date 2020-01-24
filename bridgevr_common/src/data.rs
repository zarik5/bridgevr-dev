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

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq)]
pub struct Fov {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Default)]
pub struct Pose {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MotionDesc {
    pub pose: Pose,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum FfmpegVideoEncoderType {
    #[cfg(target_os = "linux")]
    CUDA,
    // AMD soon? waiting on Vulkan hwcontext support
    // VAAPI is excluded because there are no Rust libraries
    #[cfg(windows)]
    D3D11VA,

    #[cfg(target_os = "macos")]
    VideoToolbox,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum FfmpegVideoDecoderType {
    #[cfg(target_os = "android")]
    MediaCodec,

    #[cfg(windows)]
    D3D11VA,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FfmpegOptionValue {
    String(String),
    Int(i64),
    Double(f64),
    Rational { num: i32, den: i32 },
    Binary(Vec<u8>),
    ImageSize { width: i32, height: i32 },
    VideoRate { num: i32, den: i32 },
    ChannelLayout(i64),
    Dictionary(Vec<(String, String)>),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FfmpegOption(pub String, pub FfmpegOptionValue);

#[derive(Serialize, Deserialize, Clone)]
pub struct FfmpegVideoCodecDesc {
    pub codec_name: String,
    pub context_options: Vec<FfmpegOption>,
    pub priv_data_options: Vec<FfmpegOption>,
    pub codec_open_options: Vec<(String, String)>,
    pub frame_options: Vec<FfmpegOption>,
    pub vendor_specific_context_options: Vec<(String, String)>,
    pub hw_frames_context_options: Vec<FfmpegOption>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum FrameSize {
    Scale(f32),
    Absolute(u32, u32),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum LatencyDesc {
    Automatic {
        default_ms: u32,
        expected_misses_per_hour: u32,
        history_mean_lifetime_s: u32,
    },
    Manual {
        ms: u32,
        history_mean_lifetime_s: u32,
    },
}

// the following settings are explained here:
// https://docs.rs/laminar/0.3.2/laminar/struct.Config.html
#[derive(Serialize, Deserialize, Clone)]
pub struct SocketDesc {
    pub idle_connection_timeout_ms: Option<u64>, // this should be comprehensive of the running start setup time
    pub max_packet_size: Option<u64>,
    pub receive_buffer_max_size: Option<u64>,
    pub rtt_smoothing_factor: Option<f32>, // todo: maybe unused?
    pub rtt_max_value: Option<u16>,        // todo: maybe unused?
    pub socket_event_buffer_size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectionDesc {
    pub server_idle_shutdown_timeout_ms: u64,
    pub client_ip: Option<String>,
    pub server_port: u16,
    pub client_port: u16,
    pub socket_desc: SocketDesc,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoEncoderDesc {
    Ffmpeg(FfmpegVideoEncoderType, FfmpegVideoCodecDesc),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoDecoderDesc {
    Ffmpeg(FfmpegVideoDecoderType, FfmpegVideoCodecDesc),
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
    shape_ratio: f32,
    vertical_offset: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VideoDesc {
    pub frame_size: FrameSize,
    pub halve_frame_rate: bool,
    pub composition_filtering: CompositionFilteringType,
    pub foveated_rendering: Switch<FoveatedRenderingDesc>,
    pub frame_slice_count: u8,
    pub encoder: VideoEncoderDesc,
    pub decoder: VideoDecoderDesc,
    pub buffering_frame_latency: LatencyDesc,
    pub buffering_head_pose_latency: LatencyDesc,
    pub reliable: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum AudioFormat {
    Bit16,
    Bit24,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioDesc {
    pub input_device_index: Option<u64>,
    pub output_device_index: Option<u64>,
    pub preferred_sample_rate: u16,
    pub preferred_format: AudioFormat,
    pub buffering_latency: LatencyDesc,
    pub reliable: bool,
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

#[derive(Serialize, Deserialize, Clone)]
pub enum InputType {
    Boolean,
    NormalizedOneSided,
    NormalizedTwoSided,
    Skeletal,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenvrProp {
    pub code: u32,
    pub value: OpenvrPropValue,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenvrDesc {
    pub timeout_seconds: u64,
    pub block_standby: bool,
    pub input_mapping: [Vec<(String, InputType, Vec<String>)>; 2],
    pub compositor_type: CompositorType,
    pub preferred_render_eye_resolution: Option<(u32, u32)>,
    pub hmd_custom_properties: Vec<OpenvrProp>,
    pub controllers_custom_properties: [Vec<OpenvrProp>; 2],
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
    pub video: VideoDesc,
    pub game_audio: Switch<AudioDesc>,
    pub microphone: Switch<AudioDesc>,
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
        const A = 0x00_01;
        const B = 0x00_02;
        const X = 0x00_04;
        const Y = 0x00_08;
        const DPAD_LEFT = 0x00_10;
        const DPAD_RIGHT = 0x00_20;
        const DPAD_UP = 0x00_40;
        const DPAD_DOWN = 0x00_80;
        const JOYSTICK_LEFT_PRESS = 0x01_00;
        const JOYSTICK_RIGHT_PRESS = 0x02_00;
        const SHOULDER_LEFT = 0x04_00;
        const SHOULDER_RIGHT = 0x08_00;
        const MENU = 0x10_00;
        const VIEW = 0x20_00;
        const HOME = 0x40_00;
    }
}

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct OculusTouchDigitalInput: u32 {
        const A_PRESS = 0x00_00_00_01;
        const A_TOUCH = 0x00_00_00_02;
        const B_PRESS = 0x00_00_00_04;
        const B_TOUCH = 0x00_00_00_08;
        const X_PRESS = 0x00_00_00_10;
        const X_TOUCH = 0x00_00_00_20;
        const Y_PRESS = 0x00_00_00_40;
        const Y_TOUCH = 0x00_00_00_80;
        const THUMBSTICK_LEFT_PRESS = 0x00_00_01_00;
        const THUMBSTICK_LEFT_TOUCH = 0x00_00_02_00;
        const THUMBSTICK_RIGHT_PRESS = 0x00_00_04_00;
        const THUMBSTICK_RIGHT_TOUCH = 0x00_00_08_00;
        const TRIGGER_LEFT_TOUCH = 0x00_00_10_00;
        const TRIGGER_RIGHT_TOUCH = 0x00_00_20_00;
        const GRIP_LEFT_TOUCH = 0x00_00_40_00;
        const GRIP_RIGHT_TOUCH = 0x00_00_80_00;
        const MENU = 0x00_01_00_00;
        const HOME = 0x00_02_00_00;
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
    OculusHands([Vec<MotionDesc>; 2]),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClientHandshakePacket {
    pub bridgevr_name: String,
    pub version: Version,
    pub native_eye_resolution: (u32, u32),
    pub fov: [Fov; 2],
    pub fps: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ServerHandshakePacket {
    pub version: Version,
    pub settings: Settings,
    pub target_eye_resolution: (u32, u32),
}

#[derive(Serialize, Deserialize)]
pub struct HapticData {
    pub hand: u8,
    pub duration_seconds: f32,
    pub frequency: f32,
    pub amplitude: f32,
}

#[derive(Serialize, Deserialize)]
pub struct VideoPacket<'a> {
    pub slice_index: u8,
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
pub struct MotionData {
    pub time_ns: u64,
    pub hmd: MotionDesc,
    pub controllers: [MotionDesc; 2],
}

#[derive(Serialize, Deserialize)]
pub struct ClientInputs {
    pub motion_data: MotionData,
    pub input_device_data: InputDeviceData,
    pub vsync_offset_ns: i32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ClientStatistics {}

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

    pub fn save(&self) -> StrResult {
        const TRACE_CONTEXT: &str = "Session";
        trace_err!(fs::write(
            &self.path,
            trace_err!(json::to_string_pretty(&self.session_desc))?
        ))
    }
}
