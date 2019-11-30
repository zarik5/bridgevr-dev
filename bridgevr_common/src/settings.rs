// WARNING: never use usize (see packets.rs)

use crate::StrResult;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json as json;
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub enum Switch<T> {
    Enabled(T),
    Disabled,
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

#[derive(Serialize, Deserialize, Clone)]
pub enum FrameSize {
    Scale(f32),
    Absolute { width: u32, height: u32 },
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
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
pub enum Clients {
    Count(u64),
    WithIp(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Connections {
    pub clients: Clients,
    pub starting_data_port: u16,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoEncoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum AudioEncoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum VideoDecoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum AudioDecoderDesc {
    Gstreamer(String),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FoveatedRendering {
    strength: f32,
    vertical_offset: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Slices {
    Count(u64),
    Size { max_pixels: u64 },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Video {
    pub frame_size: FrameSize,
    pub encoder: VideoEncoderDesc,
    pub decoder: VideoDecoderDesc,
    pub foveated_rendering: Switch<FoveatedRendering>,
    pub slices: Slices,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Audio {
    pub bitrate_video_audio_balance: f32,
    pub encoder: AudioEncoderDesc,
    pub decoder: AudioDecoderDesc,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Microphone {
    pub encoder: AudioEncoderDesc,
    pub decoder: AudioDecoderDesc,
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
pub struct OpenvrProps {
    pub preferred_render_eye_width: Option<u32>,
    pub preferred_render_eye_height: Option<u32>,
    pub hmd_custom_properties: Option<Vec<OpenvrProp>>,
    pub controllers_custom_properties: Option<Vec<OpenvrProp>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub connections: Connections,
    pub latency: LatencyDesc,
    pub bitrate: BitrateDesc,
    pub video: Video,
    pub audio: Switch<Audio>,
    pub microphone: Switch<Microphone>,
    pub openvr: OpenvrProps,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Session {
    bitrate: u32,
    client_presentation_latency_ns: u32,
    server_present_latency_ns: u32,

    // don't care
    settings_cache: serde_json::Value,
}

pub fn load_settings(path: &str) -> StrResult<Settings> {
    // If settings.json is invalid or unavailable I cannot use a default value because for the video
    // codec I need to know the which hardware the client has.
    // serde_json::from_value(value: Value)
    const TRACE_CONTEXT: &str = "Settings";
    trace_err!(json::from_str(&trace_err!(fs::read_to_string(path))?))
}

pub fn load_session(path: &str) -> Session {
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

pub fn save_session(path: &str, session: &Session) -> StrResult<()> {
    const TRACE_CONTEXT: &str = "Session";
    trace_err!(fs::write(
        path,
        trace_err!(json::to_string_pretty(session))?
    ))
}
