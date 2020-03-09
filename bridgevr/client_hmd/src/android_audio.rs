use bridgevr_common::{*, sockets::* };

pub struct AudioRecorder {
}

impl AudioRecorder {
    pub fn start_recording(
        device_idx: Option<u64>,
        loopback: bool,
        mut packet_enqueuer: PacketEnqueuer,
    ) -> StrResult<AudioRecorder> {
        todo!()
    }

    pub fn request_stop(&mut self) {
        todo!()
    }
}

pub struct AudioPlayer {}

impl AudioPlayer {
    pub fn start_playback(
        device_idx: Option<u64>,
        mut packet_dequeuer: PacketDequeuer,
    ) -> StrResult<AudioPlayer> {
        todo!()
    }

    pub fn request_stop(&mut self) {
        todo!()
    }
}
