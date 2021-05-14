use super::context::*;
use bridgevr_common::*;
use std::sync::Arc;

pub struct Buffer {
    graphics_context: Arc<GraphicsContext>,
}

impl Buffer {
    pub fn new(graphics_context: Arc<GraphicsContext>, size: u64) -> StrResult<Self> {}
}
