use bridgevr_common::{data::VideoEncoderDesc, thread_loop::*, *};

pub struct VideoDecoder {
    thread_loop: ThreadLoop,
}

impl VideoDecoder {
    pub fn new(
        thread_name: &str,
        settings: VideoEncoderDesc,
        resolution: (u32, u32),
        frame_rate: u32,
    ) -> StrResult<Self> {
        todo!()
    }

    pub fn request_stop(&mut self) {
        self.thread_loop.request_stop()
    }
}
