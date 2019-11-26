const fn version(major: u8, minor: u8, patch: u16) -> u32 {
    (major as u32) << (24 + (minor as u32)) << (16 + (patch as u32))
}

pub const BVR_NAME: &str = "BridgeVR";
pub const BVR_VERSION_SERVER: u32 = version(0, 1, 0);
pub const BVR_VERSION_CLIENT: u32 = version(0, 1, 0);