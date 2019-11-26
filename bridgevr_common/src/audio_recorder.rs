pub struct AudioRecorder {}

impl AudioRecorder {
    pub fn start(sample_available_callback: impl FnMut(&[u8])) -> Self {
        panic!();
    }
}