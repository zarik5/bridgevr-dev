use crate::compositor::*;
use bridgevr_common::{
    rendering::*,
    ring_channel::*,
    data::VideoEncoderDesc,
    sockets::*,
    thread_loop::{self, *},
    *,
};

pub fn aligned_resolution(width: u32, height: u32) -> (u32, u32) {
    (
        ((width / 16) as f32).ceil() as u32 * 16,
        ((height / 16) as f32).ceil() as u32 * 16,
    )
}

pub struct VideoEncoder {
    thread_loop: ThreadLoop,
}

impl VideoEncoder {
    pub fn new(
        settings: VideoEncoderDesc,
        frame_consumer: Consumer<FrameSlice>,
        packet_producer: Producer<SenderData>,
    ) -> StrResult<Self> {
        unimplemented!()
    }

    pub fn request_stop(&mut self) {
        self.thread_loop.request_stop()
    }
}
