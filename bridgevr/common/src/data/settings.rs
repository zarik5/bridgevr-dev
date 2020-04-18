use crate::*;
use serde::{Deserialize, Serialize};
use settings_schema::{
    DictionaryDefault, OptionalDefault, SettingsSchema, Switch, SwitchDefault, VectorDefault,
};
use std::{fs, path::*};

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Fov {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy)]
pub struct Pose {
    #[schema(step = 0.001)]
    pub position: [f32; 3],

    pub orientation: [f32; 4],
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
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

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct ConnectionDesc {
    pub client_ip: Option<String>,

    #[schema(min = 1024)]
    pub server_port: u16,

    #[schema(min = 1024)]
    pub client_port: u16,

    pub config: SocketConfig,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy)]
pub enum FrameSize {
    #[schema(min = 0.25, max = 1.5, step = 0.25)]
    Scale(f32),

    Absolute {
        width: u32,
        height: u32,
    },
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy)]
pub enum CompositionFilteringType {
    NearestNeighbour,

    Bilinear,

    #[schema(min = 0.5, max = 4., step = 0.01)]
    Lanczos(f32),
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy)]
pub struct FoveatedRenderingDesc {
    #[schema(min = 0.5, max = 10., step = 0.1)]
    strength: f32,

    #[schema(advanced, min = 0.5, max = 2., step = 0.1)]
    shape_ratio: f32,

    #[schema(min = -0.05, max = 0.05, step = 0.001)]
    vertical_offset: f32,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Debug)]
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

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VideoCodecDesc {
    pub codec_name: String,
    pub context_options: Vec<(String, FfmpegOptionValue)>,
    pub priv_data_options: Vec<(String, FfmpegOptionValue)>,
    pub codec_open_options: Vec<(String, String)>,
    pub frame_options: Vec<(String, FfmpegOptionValue)>,
    pub hw_frames_context_options: Vec<(String, FfmpegOptionValue)>,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VideoEncoderDesc {
    pub linux_windows_amd: VideoCodecDesc,
    pub linux_windows_nvidia: VideoCodecDesc,
    pub macos: VideoCodecDesc,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VideoDecoderDesc {
    pub android: VideoCodecDesc,
    pub windows: VideoCodecDesc,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub enum LatencyMode {
    Automatic {
        #[schema(min = 1, gui = "UpDown")]
        expected_misses_per_hour: u32,
    },
    Manual,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct LatencyDesc {
    // todo: when the users set this to 0, show message:
    // "BridgeVR cannot do magic! A value greater than 0 is needed to avoid missing frames"
    #[schema(gui = "UpDown")]
    pub default_ms: u32,

    #[schema(advanced, gui = "UpDown")]
    pub history_mean_lifetime_s: u32,

    pub mode: LatencyMode,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VideoDesc {
    pub frame_size: FrameSize,

    #[schema(advanced)]
    pub fov: Option<[Fov; 2]>,

    #[schema(advanced)]
    pub preferred_framerate: u16,

    #[schema(advanced)]
    pub composition_filtering: CompositionFilteringType,

    pub foveated_rendering: Switch<FoveatedRenderingDesc>,

    #[schema(advanced, min = 1, max = 8, gui = "UpDown")]
    pub frame_slice_count: u8,

    #[schema(advanced)]
    pub encoder: VideoEncoderDesc,

    #[schema(advanced)]
    pub decoder: VideoDecoderDesc,

    pub buffering_frame_latency: LatencyDesc,

    #[schema(advanced)]
    pub pose_prediction_update_history_mean_lifetime_s: u32,

    pub non_hmd_devices_pose_prediction_multiplier: f32,

    #[schema(advanced)]
    pub reliable: bool,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub enum AudioFormat {
    Bit8,
    Bit16,
    Bit24,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct AudioDesc {
    #[schema(advanced)]
    pub input_device_index: Option<u64>,

    #[schema(advanced)]
    pub output_device_index: Option<u64>,

    #[schema(advanced)]
    pub preferred_sample_rate: u16,

    #[schema(advanced)]
    pub preferred_format: AudioFormat,

    pub buffering_latency: LatencyDesc,

    #[schema(advanced)]
    pub reliable: bool,
}

#[repr(i32)]
#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackedDeviceType {
    HMD = 0, // HMD = 0 is enforced by OpenVR
    LeftController,
    RightController,
    Gamepad,
    GenericTracker1,
    GenericTracker2,
    GenericTracker3,
    GenericTracker4,
    GenericTracker5,
    GenericTracker6,
    GenericTracker7,
    GenericTracker8,
    GenericTracker9,
    GenericTracker10,
    GenericTracker11,
    GenericTracker12,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct TrackedDeviceDesc {
    pub device_type: TrackedDeviceType,
    pub default_pose: Pose,
    pub pose_offset: Pose,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Copy)]
pub enum CompositorType {
    Custom,
    SteamVR,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone, Debug)]
pub enum OpenvrPropValue {
    Bool(bool),
    Int32(i32),
    Uint64(u64),
    Float(f32),
    String(String),
    Vector3([f32; 3]),
    Double(f64),
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub enum OpenvrInputType {
    Boolean,
    NormalizedOneSided,
    NormalizedTwoSided,
    Skeletal,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct OpenvrInputValue {
    pub input_type: OpenvrInputType,
    pub source_paths: Vec<String>,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct OpenvrTrackedDeviceDesc {
    pub device_type: TrackedDeviceType,
    pub properties: Vec<(String, OpenvrPropValue)>,
    pub input_mapping: Vec<(String, OpenvrInputValue)>,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct OpenvrDesc {
    pub custom_fov: Option<[Fov; 2]>,
    pub tracked_devices: Vec<OpenvrTrackedDeviceDesc>,
    pub block_standby: bool,
    pub server_idle_timeout_s: u64,
    pub preferred_render_eye_resolution: Option<FrameSize>,
    pub compositor_type: CompositorType,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VrServerDesc {
    pub openvr: OpenvrDesc,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub enum HmdTrackingMode {
    Absolute,
    XYRelativeZAbsolute,
    Relative,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct OvrMobileDesc {
    #[schema(min = 0, max = 3, gui = "Slider")]
    pub cpu_level: i32,

    #[schema(min = 0, max = 3, gui = "Slider")]
    pub gpu_level: i32,

    pub dynamic_clock_throttling: bool,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct OpenxrDesc {
    pub hmd_tracking_mode: HmdTrackingMode,
    pub ovr_mobile: OvrMobileDesc,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct VrClientDesc {
    pub openxr: OpenxrDesc,
}

#[derive(SettingsSchema, Serialize, Deserialize, Clone)]
pub struct Settings {
    #[schema(advanced)]
    pub connection: ConnectionDesc,

    pub video: VideoDesc,

    pub game_audio: Switch<AudioDesc>,

    pub microphone: Switch<AudioDesc>,

    #[schema(advanced)]
    pub tracked_devices: Vec<TrackedDeviceDesc>,

    #[schema(advanced)]
    pub vr_server: VrServerDesc,

    #[schema(advanced)]
    pub vr_client: VrClientDesc,
}

pub fn load_settings(path: &Path) -> StrResult<Settings> {
    const TRACE_CONTEXT: &str = "Settings";
    trace_err!(serde_json::from_str(&trace_err!(fs::read_to_string(path))?))
}

pub fn settings_default() -> SettingsDefault {
    let default_ffmpeg_option_value = FfmpegOptionValueDefault {
        variant: FfmpegOptionValueDefaultVariant::String,
        String: "".into(),
        Int: 0,
        Double: 0.,
        Rational: FfmpegOptionValueRationalDefault { num: 1, den: 1 },
        Binary: VectorDefault {
            element: 0,
            default: vec![],
        },
        ImageSize: FfmpegOptionValueImageSizeDefault {
            width: 100,
            height: 100,
        },
        VideoRate: FfmpegOptionValueVideoRateDefault { num: 1, den: 1 },
        ChannelLayout: 0,
        Dictionary: DictionaryDefault {
            key: "".into(),
            value: "".into(),
            default: vec![],
        },
    };

    let default_pose = Pose {
        position: [0.; 3],
        orientation: [1., 0., 0., 0.],
    };

    SettingsDefault {
        connection: ConnectionDescDefault {
            client_ip: OptionalDefault {
                set: false,
                content: "192.168.X.X".into(),
            },
            server_port: 9944,
            client_port: 9944,
            config: SocketConfigDefault {
                idle_connection_timeout_ms: OptionalDefault {
                    set: true,
                    content: 1000,
                },
                max_packet_size: OptionalDefault {
                    set: false,
                    content: 16384,
                },
                max_fragments: OptionalDefault {
                    set: false,
                    content: 16,
                },
                fragment_size: OptionalDefault {
                    set: false,
                    content: 1024,
                },
                fragment_reassembly_buffer_size: OptionalDefault {
                    set: false,
                    content: 64,
                },
                receive_buffer_max_size: OptionalDefault {
                    set: false,
                    content: 1452,
                },
                rtt_smoothing_factor: OptionalDefault {
                    set: false,
                    content: 0.10,
                },
                rtt_max_value: OptionalDefault {
                    set: false,
                    content: 250,
                },
                socket_event_buffer_size: OptionalDefault {
                    set: false,
                    content: 1024,
                },
                max_packets_in_flight: OptionalDefault {
                    set: false,
                    content: 512,
                },
            },
        },
        video: VideoDescDefault {
            frame_size: FrameSizeDefault {
                variant: FrameSizeDefaultVariant::Scale,
                Scale: 1.,
                Absolute: FrameSizeAbsoluteDefault {
                    width: 1920,
                    height: 1080,
                },
            },
            fov: OptionalDefault {
                set: false,
                content: [
                    FovDefault {
                        left: -45.,
                        top: -45.,
                        right: 45.,
                        bottom: 45.,
                    },
                    FovDefault {
                        left: -45.,
                        top: -45.,
                        right: 45.,
                        bottom: 45.,
                    },
                ],
            },
            preferred_framerate: 72,
            composition_filtering: CompositionFilteringTypeDefault {
                variant: CompositionFilteringTypeDefaultVariant::Bilinear,
                Lanczos: 2.5,
            },
            foveated_rendering: SwitchDefault {
                enabled: false,
                content: FoveatedRenderingDescDefault {
                    strength: 4.,
                    shape_ratio: 1.5,
                    vertical_offset: 0.,
                },
            },
            frame_slice_count: 1,
            encoder: VideoEncoderDescDefault {
                linux_windows_amd: VideoCodecDescDefault {
                    codec_name: "".into(),
                    context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    priv_data_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    codec_open_options: DictionaryDefault {
                        key: "".into(),
                        value: "".into(),
                        default: vec![],
                    },
                    frame_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    hw_frames_context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                },
                linux_windows_nvidia: VideoCodecDescDefault {
                    codec_name: "".into(),
                    context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    priv_data_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    codec_open_options: DictionaryDefault {
                        key: "".into(),
                        value: "".into(),
                        default: vec![],
                    },
                    frame_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    hw_frames_context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                },
                macos: VideoCodecDescDefault {
                    codec_name: "".into(),
                    context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    priv_data_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    codec_open_options: DictionaryDefault {
                        key: "".into(),
                        value: "".into(),
                        default: vec![],
                    },
                    frame_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    hw_frames_context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                },
            },
            decoder: VideoDecoderDescDefault {
                android: VideoCodecDescDefault {
                    codec_name: "".into(),
                    context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    priv_data_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    codec_open_options: DictionaryDefault {
                        key: "".into(),
                        value: "".into(),
                        default: vec![],
                    },
                    frame_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    hw_frames_context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                },
                windows: VideoCodecDescDefault {
                    codec_name: "".into(),
                    context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    priv_data_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    codec_open_options: DictionaryDefault {
                        key: "".into(),
                        value: "".into(),
                        default: vec![],
                    },
                    frame_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value.clone(),
                        default: vec![],
                    },
                    hw_frames_context_options: DictionaryDefault {
                        key: "".into(),
                        value: default_ffmpeg_option_value,
                        default: vec![],
                    },
                },
            },
            buffering_frame_latency: LatencyDescDefault {
                default_ms: 30,
                history_mean_lifetime_s: 5,
                mode: LatencyModeDefault {
                    variant: LatencyModeDefaultVariant::Automatic,
                    Automatic: LatencyModeAutomaticDefault {
                        expected_misses_per_hour: 60,
                    },
                },
            },
            pose_prediction_update_history_mean_lifetime_s: 60,
            non_hmd_devices_pose_prediction_multiplier: 1.,
            reliable: false,
        },
        game_audio: SwitchDefault {
            enabled: true,
            content: AudioDescDefault {
                input_device_index: OptionalDefault {
                    set: false,
                    content: 0,
                },
                output_device_index: OptionalDefault {
                    set: false,
                    content: 0,
                },
                preferred_sample_rate: 44100,
                preferred_format: AudioFormatDefault {
                    variant: AudioFormatDefaultVariant::Bit16,
                },
                buffering_latency: LatencyDescDefault {
                    default_ms: 30,
                    history_mean_lifetime_s: 120,
                    mode: LatencyModeDefault {
                        variant: LatencyModeDefaultVariant::Automatic,
                        Automatic: LatencyModeAutomaticDefault {
                            expected_misses_per_hour: 30,
                        },
                    },
                },
                reliable: false,
            },
        },
        microphone: SwitchDefault {
            enabled: false,
            content: AudioDescDefault {
                input_device_index: OptionalDefault {
                    set: false,
                    content: 0,
                },
                output_device_index: OptionalDefault {
                    set: false,
                    content: 0,
                },
                preferred_sample_rate: 44100,
                preferred_format: AudioFormatDefault {
                    variant: AudioFormatDefaultVariant::Bit8,
                },
                buffering_latency: LatencyDescDefault {
                    default_ms: 40,
                    history_mean_lifetime_s: 120,
                    mode: LatencyModeDefault {
                        variant: LatencyModeDefaultVariant::Automatic,
                        Automatic: LatencyModeAutomaticDefault {
                            expected_misses_per_hour: 120,
                        },
                    },
                },
                reliable: false,
            },
        },
        tracked_devices: VectorDefault {
            element: TrackedDeviceDescDefault {
                device_type: TrackedDeviceTypeDefault {
                    variant: TrackedDeviceTypeDefaultVariant::GenericTracker1,
                },
                default_pose: PoseDefault {
                    position: [0.; 3],
                    orientation: [1., 0., 0., 0.],
                },
                pose_offset: PoseDefault {
                    position: [0.; 3],
                    orientation: [1., 0., 0., 0.],
                },
            },
            default: vec![
                TrackedDeviceDesc {
                    device_type: TrackedDeviceType::HMD,
                    default_pose: Pose {
                        position: [0., 1.7, 0.],
                        orientation: [1., 0., 0., 0.],
                    },
                    pose_offset: default_pose,
                },
                TrackedDeviceDesc {
                    device_type: TrackedDeviceType::LeftController,
                    default_pose: Pose {
                        position: [-0.25, 0.8, 0.],
                        orientation: [1., 0., 0., 0.],
                    },
                    pose_offset: default_pose,
                },
                TrackedDeviceDesc {
                    device_type: TrackedDeviceType::RightController,
                    default_pose: Pose {
                        position: [0.25, 0.8, 0.],
                        orientation: [1., 0., 0., 0.],
                    },
                    pose_offset: default_pose,
                },
            ],
        },

        vr_server: VrServerDescDefault {
            openvr: OpenvrDescDefault {
                custom_fov: OptionalDefault {
                    set: false,
                    content: [
                        FovDefault {
                            left: -45.,
                            top: 45.,
                            right: 45.,
                            bottom: -45.,
                        },
                        FovDefault {
                            left: -45.,
                            top: 45.,
                            right: 45.,
                            bottom: -45.,
                        },
                    ],
                },
                tracked_devices: VectorDefault {
                    element: OpenvrTrackedDeviceDescDefault {
                        device_type: TrackedDeviceTypeDefault {
                            variant: TrackedDeviceTypeDefaultVariant::GenericTracker1,
                        },
                        properties: DictionaryDefault {
                            key: "".into(),
                            value: OpenvrPropValueDefault {
                                variant: OpenvrPropValueDefaultVariant::Bool,
                                Bool: false,
                                Int32: 0,
                                Uint64: 0,
                                Float: 0.,
                                String: "".into(),
                                Vector3: [0.; 3],
                                Double: 0.,
                            },
                            default: vec![],
                        },
                        input_mapping: DictionaryDefault {
                            key: "".into(),
                            value: OpenvrInputValueDefault {
                                input_type: OpenvrInputTypeDefault {
                                    variant: OpenvrInputTypeDefaultVariant::Boolean,
                                },
                                source_paths: VectorDefault {
                                    element: "".into(),
                                    default: vec![],
                                },
                            },
                            default: vec![],
                        },
                    },
                    default: vec![
                        OpenvrTrackedDeviceDesc {
                            device_type: TrackedDeviceType::HMD,
                            properties: vec![],
                            input_mapping: vec![],
                        },
                        OpenvrTrackedDeviceDesc {
                            device_type: TrackedDeviceType::LeftController,
                            properties: vec![],
                            input_mapping: vec![],
                        },
                        OpenvrTrackedDeviceDesc {
                            device_type: TrackedDeviceType::RightController,
                            properties: vec![],
                            input_mapping: vec![],
                        },
                    ],
                },
                block_standby: false,
                server_idle_timeout_s: 60,
                preferred_render_eye_resolution: OptionalDefault {
                    set: false,
                    content: FrameSizeDefault {
                        variant: FrameSizeDefaultVariant::Scale,
                        Scale: 1.,
                        Absolute: FrameSizeAbsoluteDefault {
                            width: 1920,
                            height: 1080,
                        },
                    },
                },
                compositor_type: CompositorTypeDefault {
                    variant: CompositorTypeDefaultVariant::Custom,
                },
            },
        },
        vr_client: VrClientDescDefault {
            openxr: OpenxrDescDefault {
                hmd_tracking_mode: HmdTrackingModeDefault {
                    variant: HmdTrackingModeDefaultVariant::Absolute,
                },
                ovr_mobile: OvrMobileDescDefault {
                    cpu_level: 2,
                    gpu_level: 2,
                    dynamic_clock_throttling: true,
                },
            },
        },
    }
}
