use bridgevr_common::{graphics::*, *};
use std::sync::Arc;

pub struct Compositor {}

impl Compositor {
    pub fn new(graphics: Arc<GraphicsContext>) -> StrResult<Self> {
        todo!();
    }

    pub fn initialize_for_server(&self) {
        todo!();
    }

    pub fn deinitialize_for_server(&self) {
        todo!();
    }

    pub fn render_idle_frame(&self) {
        todo!();
    }

    pub fn render_stream_frame(&self) {
        todo!();
    }
}
