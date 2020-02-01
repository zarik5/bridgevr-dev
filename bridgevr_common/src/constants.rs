use serde::*;
use std::fmt::*;

#[derive(Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct Version(u32);

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}.{}.{}",
            self.0 >> 24,
            (self.0 >> 16) % 0x1_00,
            self.0 % 0x1_00_00
        )
    }
}

const fn version(major: u32, minor: u32, patch: u32) -> Version {
    Version((major << 24) + (minor << 16) + patch)
}

pub const BVR_NAME: &str = "BridgeVR";

pub const BVR_VERSION_SERVER: Version = version(0, 0, 1);
pub const BVR_VERSION_CLIENT: Version = version(0, 0, 1);

pub const BVR_MIN_VERSION_SERVER: Version = version(0, 0, 0);
pub const BVR_MIN_VERSION_CLIENT: Version = version(0, 0, 0);

// IDs used to match PacketSender-PacketReceiver across server and client
// These IDs are used also as priority cues when dealing with bandwidth issues.
pub const SERVER_SHUTDOWN_STREAM_ID: u8 = 0;
pub const fn video_stream_id(slice_id: u8) -> u8 {
    slice_id + 1
}
pub const fn game_audio_stream_id(video_slice_count: u8) -> u8 {
    video_slice_count
}
pub const fn haptic_stream_id(video_slice_count: u8) -> u8 {
    video_slice_count + 1
}

pub const CLIENT_DISCONNECTED_STREAM_ID: u8 = 0;
pub const CLIENT_INPUTS_STREAM_ID: u8 = 1;
pub const MICROPHONE_STREAM_ID: u8 = 2;
pub const CLIENT_STATISTICS_STREAM_ID: u8 = 3;
