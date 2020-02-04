use bridgevr_common::{rendering::*, *, data::*};
use std::sync::Arc;

enum Backend {
    OpenXR,
    OculusVR,
}

pub struct VrClient {
    backend: Backend,
}

impl VrClient {
    pub fn new(graphics: Arc<GraphicsContext>) -> StrResult<Self> {
        todo!();
    }

    pub fn initialize_for_server(&self) {
        todo!();
    }

    pub fn deinitialize_for_server(&self) {
        todo!();
    }

    pub fn submit_idle_frame(&self) {
        todo!();
    }

    pub fn submit_stream_frame(&self) {
        todo!();
    }

    pub fn native_eye_resolution(&self) -> (u32, u32) {
        todo!();
    }

    pub fn fov(&self) -> [Fov; 2] {
        todo!();
    }

    pub fn fps(&self) -> u32 {
        todo!();
    }

    pub fn poll_input(&self) {
        todo!()
    }
}
