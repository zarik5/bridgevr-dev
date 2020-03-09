#![allow(clippy::large_enum_variant)]

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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Fov {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Pose {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}

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
pub struct SocketConfig {
    pub idle_connection_timeout_ms: Option<u64>,
    pub max_packet_size: Option<u64>,
    pub max_fragments: Option<u8>,
    pub fragment_size: Option<u16>,
    pub fragment_reassembly_buffer_size: Option<u16>,
    pub receive_buffer_max_size: Option<u64>,
    pub rtt_smoothing_factor: Option<f32>,
    pub rtt_max_value: Option<u16>,
    pub socket_event_buffer_size: Option<u64>,
    pub max_packets_in_flight: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectionDesc {
    pub client_ip: Option<String>,
    pub server_port: u16,
    pub client_port: u16,
    pub config: SocketConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum FrameSize {
    Scale(f32),
    Absolute(u32, u32),
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CompositionFilteringType {
    NearestNeighbour,
    Bilinear,
    Lanczos(f32),
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct FoveatedRenderingDesc {
    strength: f32,
    shape_ratio: f32,
    vertical_offset: f32,
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

// Do not cfg-gate: The server must accept any value to support any type of client
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FfmpegVideoDecoderType {
    MediaCodec,
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
    pub hw_frames_context_options: Vec<FfmpegOption>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoEncoderDesc {
    Ffmpeg {
        hardware_context: FfmpegVideoEncoderType,
        config: FfmpegVideoCodecDesc,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoDecoderDesc {
    Ffmpeg {
        hardware_context: FfmpegVideoDecoderType,
        config: FfmpegVideoCodecDesc,
    },
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

#[derive(Serialize, Deserialize, Clone)]
pub struct VideoDesc {
    pub frame_size: FrameSize,
    pub preferred_framerate: u16,
    pub composition_filtering: CompositionFilteringType,
    pub foveated_rendering: Switch<FoveatedRenderingDesc>,
    pub frame_slice_count: u8,
    pub encoder: VideoEncoderDesc,
    pub decoder: VideoDecoderDesc,
    pub buffering_frame_latency: LatencyDesc,
    pub pose_prediction_update_history_mean_lifetime_s: u32,
    // pub pose_prediction_update_min_ang_vel_rad_per_s: f32, // todo check if needed
    pub non_hmd_devices_pose_prediction_multiplier: f32,
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

#[repr(i32)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackedDeviceType {
    HMD = 0, // HMD = 0 is enforced by OpenVR
    LeftController = 1,
    RightController = 2,
    Tracker1 = 3,
    Tracker2 = 4,
    Tracker3 = 5,
    Tracker4 = 6,
    Tracker5 = 7,
    Tracker6 = 8,
    Tracker7 = 9,
    Tracker8 = 10,
    Tracker9 = 11,
    Tracker10 = 12,
    Tracker11 = 13,
    Tracker12 = 14,
    Tracker13 = 15,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MotionModel3DofDesc {
    fix_threshold_meters_per_seconds_squared: f32,
    drift_threshold_radians_per_seconds: f32,
    drift_speed_meters_per_second: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrackedDeviceDesc {
    pub device_type: TrackedDeviceType,
    pub default_pose: Pose,
    pub pose_offset: Pose,
    pub motion_model_3dof: Switch<MotionModel3DofDesc>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CompositorType {
    Custom,
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
}

#[derive(Serialize, Deserialize, Clone)]
pub enum OpenvrInputType {
    Boolean,
    NormalizedOneSided,
    NormalizedTwoSided,
    Skeletal,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenvrTrackedDeviceDesc {
    pub device_type: TrackedDeviceType,
    pub properties: Vec<(String, OpenvrPropValue)>,
    pub input_mapping: Vec<(String, OpenvrInputType, Vec<String>)>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenvrDesc {
    pub tracked_devices: Vec<OpenvrTrackedDeviceDesc>,
    pub block_standby: bool,
    pub server_idle_timeout_s: u64,
    pub preferred_render_eye_resolution: Option<(u32, u32)>,
    pub compositor_type: CompositorType,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OvrMobileDesc {
    pub cpu_level: i32,
    pub gpu_level: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub connection: ConnectionDesc,
    pub video: VideoDesc,
    pub game_audio: Switch<AudioDesc>,
    pub microphone: Switch<AudioDesc>,
    pub tracked_devices: Vec<TrackedDeviceDesc>,
    pub openvr: OpenvrDesc,
    pub ovr_mobile: OvrMobileDesc,
}

pub fn load_settings(path: &str) -> StrResult<Settings> {
    const TRACE_CONTEXT: &str = "Settings";
    trace_err!(json::from_str(&trace_err!(fs::read_to_string(path))?))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClientHandshakePacket {
    pub bridgevr_name: String,
    pub version: Version,
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
    pub version: Version,
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
        const A = 0x00_01;
        const B = 0x00_02;
        const X = 0x00_04;
        const Y = 0x00_08;
        const DPAD_LEFT = 0x00_10;
        const DPAD_RIGHT = 0x00_20;
        const DPAD_UP = 0x00_40;
        const DPAD_DOWN = 0x00_80;
        const JOYSTICK_LEFT_CLICK = 0x01_00;
        const JOYSTICK_RIGHT_CLICK = 0x02_00;
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
        const A_CLICK = 0x00_00_00_01;
        const A_TOUCH = 0x00_00_00_02;
        const B_CLICK = 0x00_00_00_04;
        const B_TOUCH = 0x00_00_00_08;
        const X_CLICK = 0x00_00_00_10;
        const X_TOUCH = 0x00_00_00_20;
        const Y_CLICK = 0x00_00_00_40;
        const Y_TOUCH = 0x00_00_00_80;
        const THUMBSTICK_LEFT_CLICK = 0x00_00_01_00;
        const THUMBSTICK_LEFT_TOUCH = 0x00_00_02_00;
        const THUMBSTICK_RIGHT_CLICK = 0x00_00_04_00;
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
