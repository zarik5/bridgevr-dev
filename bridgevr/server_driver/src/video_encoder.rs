use crate::compositor::*;
use bridgevr_common::{
    data::VideoEncoderDesc,
    sockets::*,
    thread_loop::{self, *},
    *,
};
use log::debug;
use std::{sync::mpsc::*, time::Duration};

const TRACE_CONTEXT: &str = "Video encoder";

const TIMEOUT: Duration = Duration::from_millis(100);

pub fn aligned_resolution((width, height): (u32, u32)) -> (u32, u32) {
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
        thread_name: &str,
        settings: VideoEncoderDesc,
        resolution: (u32, u32),
        frame_rate: u32,
        slice_receiver: Receiver<FrameSlice>,
        slice_encoded_notif_sender: Sender<()>,
        packet_enqueuer: PacketEnqueuer,
    ) -> StrResult<Self> {
        // let encode_callback = match settings {
        //     VideoEncoderDesc::Nvidia(nv_codec) => {
        //         let encoder =
        //             NvidiaEncoder::new(graphics_device_ptr, resolution, frame_rate, nv_codec)?;

        //         move |texture, force_idr| encoder.encode(force_idr, texture)
        //     }
        //     VideoEncoderDesc::Gstreamer(pipeline_str) => todo!(),
        // };

        // let thread_loop = thread_loop::spawn(thread_name, move || {
        //     let mut maybe_video_packet = None;
        //     frame_consumer
        //         .consume(TIMEOUT, |frame_slice| {
        //             maybe_video_packet =
        //                 encode_callback(frame_slice.texture.clone(), frame_slice.force_idr)
        //                     .map_err(|e| debug!("{}", e))
        //                     .ok();
        //             Ok(())
        //         })
        //         .map_err(|e| debug!("{:?}", e))
        //         .ok();

        //     if let Some(video_packet) = maybe_video_packet {
        //         packet_producer
        //             .fill(TIMEOUT, |sender_data| {
        //                 sender_data.packet = video_packet;
        //                 // todo fill other fields
        //                 Ok(())
        //             })
        //             .map_err(|e| debug!("{:?}", e))
        //             .ok();
        //     }
        // })?;

        // Ok(Self { thread_loop })
        todo!()
    }

    pub fn request_stop(&mut self) {
        self.thread_loop.request_stop()
    }
}
