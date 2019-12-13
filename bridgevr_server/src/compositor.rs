use crate::video_encoder::aligned_resolution;
use bridgevr_common::{
    data::*,
    ffr::*,
    frame_slices::*,
    rendering::*,
    ring_channel::*,
    thread_loop::{self, ThreadLoop},
    *,
};
use parking_lot::Mutex;
use std::{collections::HashMap, ops::RangeFrom, sync::Arc, time::Duration};

const TRACE_CONTEXT: &str = "Server Graphics";

const TIMEOUT: Duration = Duration::from_millis(100);

pub struct FrameSlice {
    pub index: usize,
    pub texture: Arc<Texture>,
    pub pose: Pose,
    pub force_idr_slice_idxs: Vec<usize>,
}

#[derive(Clone, Copy)]
pub struct Bounds {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

pub type LayerDesc = ([(u64, Bounds); 2], Pose);

pub struct PresentData {
    pub frame_index: u64,
    pub layers: Vec<LayerDesc>,
    pub sync_texture_handle: u64,
    pub force_idr_slice_idxs: Vec<usize>,
}

// This is able to create and destroy textures even when the client is not connected, so SteamVR
// does not hang or throw errors.
pub struct Compositor {
    graphics_al: Arc<GraphicsAL2D>,
    swap_textures: HashMap<u64, Arc<Mutex<Texture>>>,
    swap_texture_handle_sets_id_iter: RangeFrom<usize>,
    swap_texture_handle_sets: HashMap<usize, [u64; 3]>,
    rendering_loop: Option<ThreadLoop>,
}

impl Compositor {
    fn empty_rendering_loop(present_consumer: Arc<Mutex<Consumer<PresentData>>>) -> ThreadLoop {
        thread_loop::spawn(move || {
            present_consumer.lock().consume(TIMEOUT, |_| Ok(())).ok();
        })
    }

    pub fn new() -> StrResult<Self> {
        let graphics_al = Arc::new(GraphicsAL2D::new(Some(0))?);

        Ok(Self {
            graphics_al,
            swap_textures: HashMap::new(),
            swap_texture_handle_sets_id_iter: 0..,
            swap_texture_handle_sets: HashMap::new(),
            rendering_loop: None,
        })
    }

    // todo: on openvr module use two hashmaps
    pub fn create_swap_texture_set(
        &mut self,
        width: u32,
        height: u32,
        format: u32,
        sample_count: u8,
    ) -> StrResult<(usize, [u64; 3])> {
        let format = format_from_native(format);

        let mut handles = [0; 3];
        for handle in &mut handles {
            let texture = Texture::new(
                self.graphics_al.clone(),
                width,
                height,
                format,
                Some(sample_count),
            )?;
            *handle = texture.as_handle();
            self.swap_textures
                .insert(*handle, Arc::new(Mutex::new(texture)));
        }

        let id = trace_none!(self.swap_texture_handle_sets_id_iter.next(), "Overflow")?;
        self.swap_texture_handle_sets.insert(id, handles);
        Ok((id, handles))
    }

    pub fn destroy_swap_texture_set(&mut self, id: usize) -> StrResult<()> {
        if let Some(handles) = self.swap_texture_handle_sets.remove(&id) {
            for handle in handles.iter() {
                self.swap_textures.remove(handle);
            }
        }
        Ok(())
    }

    pub fn initialize_for_client(
        &mut self,
        target_eye_width: u32,
        target_eye_height: u32,
        ffr_desc: Option<data::FoveatedRenderingDesc>,
        mut present_consumer: Consumer<PresentData>,
        sync_handle_mutex: Arc<Mutex<()>>,
        slice_producers: Vec<Producer<FrameSlice>>,
    ) -> StrResult<()> {
        let composition_texture = Arc::new(trace_err!(Texture::new(
            self.graphics_al.clone(),
            target_eye_width,
            target_eye_height,
            Format::Rgba8Unorm,
            None
        ))?);

        let (compressed_eye_width, compressed_eye_height) = match ffr_desc {
            Some(ffr_desc) => {
                ffr_compressed_eye_resolution(target_eye_width, target_eye_height, ffr_desc)
            }
            None => (target_eye_width, target_eye_height),
        };
        let slices_desc = slices_desc_from_count(
            slice_producers.len(),
            compressed_eye_width * 2,
            compressed_eye_height,
        );
        let (aligned_slice_width, aligned_slice_height) =
            aligned_resolution(slices_desc.single_width, slices_desc.single_height);

        // let mut slices = vec![];
        for prod in slice_producers {}
        //         let slices = slice_producers.iter().map(|prod| {
        // let encoder_input_texture = Arc::new(trace_err!(Texture::new(
        //             self.graphics_al.clone(),
        //             aligned_slice_width,
        //             aligned_slice_height,
        //             Format::Rgba8Unorm,
        //             None
        //         ))?);
        //         }).collect();
        let mut render = move || -> UnitResult {
            {
                let _guard = sync_handle_mutex.lock();
                let mut layers = vec![];
                present_consumer
                    .consume(TIMEOUT, |present_data| {
                        layers = present_data.layers.to_vec();

                        Ok(())
                    })
                    .map_err(|_| ())?;
            }

            Ok(())
        };

        self.rendering_loop = Some(thread_loop::spawn(move || {
            render().ok();
        }));

        Ok(())
    }

    // Calling this method is not mandatory but it makes the reinitialization faster
    pub fn request_deinitialize_for_client(&mut self) {
        if let Some(thread) = &mut self.rendering_loop {
            thread.request_stop()
        }
    }

    pub fn device_ptr(&self) -> u64 {
        self.graphics_al.device_ptr()
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

    // Return as soon the texture has been locked and then
    // pub fn present(&self, shared_texture_handle: u64) {
    //     if let Some(operation_buffer) = self
    //         .operation_buffers
    //         .get(&self.selected_input_texture_handle)
    //     {
    //         operation_buffer.execute();
    //     }
    // }

    // // Wait until the current layers have been used
    // pub fn post_present(&self) {
    //     if let Some(client) = self.client {
    //         for [h1, h2] in client.selected_textures_handles {}
    //         if let Some(layers) = self
    //             .swap_textures
    //             .get(&client.selected_input_texture_handle)
    //         {
    //             layers.lock().ok();
    //         }
    //     }
    // }
}
