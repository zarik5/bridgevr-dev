use bridgevr_common::{ffr_utils::*, rendering_utils::*, *};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;

const CONTEXT: &str = "Server Graphics";
macro_rules! trace_err {
    ($res:expr $(, $expect:expr)?) => {
        crate::trace_err!($res, CONTEXT $(, $expect)?)
    };
}
macro_rules! trace_none {
    ($res:expr $(, $none_message:expr)?) => {
        crate::trace_none!($res, CONTEXT $(, $none_message)?)
    };
}

const MAX_SWAP_TEXTURES: usize = 3;

pub struct Graphics {
    graphics_al: Arc<Graphics2DAbstractionLayer>,
    encoder_input_texture: Arc<Texture>,
    ffr_desc: Option<FfrDesc>,
    operation_buffers: HashMap<u64, OperationBuffer>,
    selected_input_texture_handle: u64,
    swap_textures: HashMap<u64, Arc<Texture>>,
    target_eye_width: u32,
    target_eye_height: u32,
}

impl Graphics {
    pub fn new(
        target_eye_width: u32,
        target_eye_height: u32,
        ffr_desc: Option<FfrDesc>,
    ) -> StrResult<Self> {
        let graphics_al = Arc::new(trace_err!(Graphics2DAbstractionLayer::new(Some(0)))?);

        //todo change with ffr
        let encoder_width = target_eye_width * 2;
        let encoder_height = target_eye_height;

        let encoder_input_texture = Arc::new(trace_err!(Texture::new(
            graphics_al.clone(),
            encoder_width,
            encoder_height,
            Format::Bgra8Unorm,
        ))?);

        Ok(Self {
            graphics_al,
            encoder_input_texture,
            ffr_desc,
            operation_buffers: HashMap::new(),
            selected_input_texture_handle: 0,
            swap_textures: HashMap::new(),
            target_eye_width,
            target_eye_height,
        })
    }

    pub fn device_ptr(&self) -> u64 {
        self.graphics_al.device_ptr()
    }

    pub fn encoder_input_texture(&self) -> Arc<Texture> {
        self.encoder_input_texture.clone()
    }

    // pub fn select_input_texture(&mut self, shared_texture_handle: u64) {
    //     let shared_texture_ref = match self.swap_textures.get(&shared_texture_handle) {
    //         Some(texture) => texture,
    //         None => {
    //             if self.swap_textures.len() == MAX_SWAP_TEXTURES {
    //                 self.swap_textures.clear();
    //                 self.operation_buffers.clear();
    //             }
    //             let texture = Arc::new(Texture::from_ptr(
    //                 self.graphics_al.clone(),
    //                 shared_texture_handle,
    //             ));
    //             self.swap_textures.insert(shared_texture_handle, texture);
    //             &self.swap_textures[&shared_texture_handle]
    //         }
    //     };

    //     shared_texture_ref.wait_for_signal();

    //     self.operation_buffers
    //         .entry(shared_texture_handle)
    //         .or_insert({
    //             let commands = if let Some(ffr_desc) = &self.ffr_desc {
    //                 std::unimplemented!();
    //             } else {
    //                 vec![OperationDesc::CopyTexture {
    //                     input: shared_texture_ref.clone(),
    //                     output: self.encoder_input_texture.clone(),
    //                 }]
    //             };
    //             OperationBuffer::new(self.graphics_al.clone(), commands)
    //         });

    //     self.selected_input_texture_handle = shared_texture_handle;
    // }

    pub fn render(&self) {
        if let Some(operation_buffer) = self
            .operation_buffers
            .get(&self.selected_input_texture_handle)
        {
            operation_buffer.execute();
        }
    }
}
