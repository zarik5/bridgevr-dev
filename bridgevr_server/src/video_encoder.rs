use bridgevr_common::settings::VideoEncoderDesc;

pub struct VideoEncoder {}

impl VideoEncoder {

    pub fn new(settings: VideoEncoderDesc, packet_ready_callback: impl FnMut(&[u8])) -> Self {
        panic!();
    }

    pub fn encode(&mut self, force_idr: bool) {
        
    }
}
