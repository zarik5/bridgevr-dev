use crate::compositor::*;
use bridgevr_common::{
    data::VideoEncoderDesc,
    ring_channel::*,
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
        let thread_loop = thread_loop::spawn(|| todo!());

        Ok(Self { thread_loop })
    }

    pub fn request_stop(&mut self) {
        self.thread_loop.request_stop()
    }
}
