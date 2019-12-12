// AADT: Axis-aligned distorted transfer

use crate::{rendering::OperationDesc, data::*};


pub fn ffr_compressed_eye_resolution(
    original_eye_width: u32,
    original_eye_height: u32,
    ffr_desc: FoveatedRenderingDesc,
) -> (u32, u32) {
    //todo
    (original_eye_width, original_eye_height)
}

pub fn ffr_compression_operation_graph() -> Vec<OperationDesc> {
    unimplemented!()
}

pub fn ffr_decompression_operation_graph() -> Vec<OperationDesc>  {
    unimplemented!()
}
