use std::fmt::*;
use serde::*;

#[derive(PartialEq, PartialOrd, Serialize, Deserialize)]
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

// this is used with UDP for pose data and TCP for shutdown signal
// at least other two ports are used (out p1: video, out p2: audio, in p1: microphone)
pub const MESSAGE_PORT: u16 = 9943;