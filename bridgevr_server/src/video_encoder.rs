use bridgevr_common::settings::VideoEncoderDesc;

pub fn aligned_resolution(width: u32, height: u32) -> (u32, u32) {
    (
        ((width / 16) as f32).ceil() as u32 * 16,
        ((height / 16) as f32).ceil() as u32 * 16,
    )
}

pub struct VideoEncoder {}

impl VideoEncoder {
    pub fn new(settings: VideoEncoderDesc, packet_ready_callback: impl FnMut(&[u8])) -> Self {
        panic!();
    }

    pub fn encode(&mut self, force_idr: bool) {}
}
