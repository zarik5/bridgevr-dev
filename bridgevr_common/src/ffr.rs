// AADT: Axis-aligned distorted transfer

use crate::{data::*, rendering::*};
use std::sync::Arc;

pub fn ffr_compressed_eye_resolution(
    original_eye_width: u32,
    original_eye_height: u32,
    ffr_desc: FoveatedRenderingDesc,
) -> (u32, u32) {
    todo!()
}

pub fn ffr_compression_operation_descs(
    source: Arc<Texture>,
    source_eye_width: u32,
    source_eye_height: u32,
    compressed_eye_width: u32,
    compressed_eye_height: u32,
) -> Vec<OperationDesc> {
    todo!()
}

pub fn ffr_decompression_operation_descs() -> Vec<OperationDesc> {
    todo!()
}
