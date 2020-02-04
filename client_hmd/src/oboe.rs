pub struct AudioRecorder {
}

impl AudioRecorder {
    pub fn start_recording(
        device_idx: Option<u64>,
        loopback: bool,
        mut buffer_producer: Producer<SenderData>,
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
        mut buffer_consumer: KeyedConsumer<ReceiverData<()>, u64>,
    ) -> StrResult<AudioPlayer> {
        todo!()
    }

    pub fn request_stop(&mut self) {
        todo!()
    }
}
