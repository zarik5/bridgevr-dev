use crate::video_encoder::aligned_resolution;
use bridgevr_common::{
    ffr::*,
    packets::*,
    rendering::*,
    ring_buffer::*,
    thread_loop::{self, ThreadLoop},
    *,
};
use std::{collections::HashMap, ops::RangeFrom, sync::*, time::Duration};

const TRACE_CONTEXT: &str = "Server Graphics";

const TIMEOUT: Duration = Duration::from_millis(100);

pub struct SlicesDesc {
    single_width: u32,
    single_height: u32,
    horizontal_count: usize,
    vertical_count: usize,
}

// Find the best arrangement of horizontal and vertical cuts so that the slices are as close as
// possible to squares. Maximizing the area/perimeter ratio, I minimize the probability that objects
// in the scene enter or exit the slice, so it uses less bandwidth.
pub fn slices_desc_from_count(count: usize, frame_width: u32, frame_height: u32) -> SlicesDesc {
    let mut min_ratio_score = std::f32::MAX; // distance from 1
    let mut best_slices_desc = SlicesDesc {
        single_width: frame_width,
        single_height: frame_height,
        horizontal_count: 1,
        vertical_count: 1,
    };
    for i in 1..=count {
        if count % i == 0 {
            let width = frame_width as f32 / (count / i) as f32;
            let height = frame_height as f32 / i as f32;
            let ratio = width / height;
            let score = (1f32 - ratio).abs();
            if score < min_ratio_score {
                min_ratio_score = score;
                best_slices_desc = SlicesDesc {
                    single_width: width.ceil() as u32,
                    single_height: height.ceil() as u32,
                    horizontal_count: count / i,
                    vertical_count: i,
                }
            } else {
                break;
            }
        }
    }

    best_slices_desc
}

pub struct Slice {
    index: usize,
    texture: Arc<Texture>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

// The compositor sends back the present data after it took a lock on shared_texture_handle.
// When the present data is returned, OpenVR can return from Present().
// When the compositor unlocks shared_texture_handle, OpenVR can return from PostPresent() or
// WaitForPresent().
pub struct PresentData {
    layers: Vec<([u64; 2], Pose)>,
    shared_texture_handle: Mutex<u64>,
}

pub enum CompositorType {
    Custom {
        swap_texture_handle_sets_id_iter: RangeFrom<usize>,
        swap_texture_handle_sets: HashMap<usize, [u64; 3]>,
    },
    Runtime,
}

// This is able to create and destroy textures even when the client is not connected, so SteamVR
// does not hang or throw errors.
pub struct Compositor {
    graphics_al: Arc<GraphicsAL2D>,
    swap_textures: HashMap<u64, Arc<Mutex<Texture>>>,
    compositor_type: CompositorType,
    present_consumer: Arc<Mutex<Consumer<PresentData>>>,
    rendering_loop: ThreadLoop,
}

impl Compositor {
    pub fn empty_rendering_loop(present_consumer: Arc<Mutex<Consumer<PresentData>>>) -> ThreadLoop {
        thread_loop::spawn(move || {
            present_consumer
                .lock()
                .unwrap()
                .consume(TIMEOUT, |_| Ok(()))
                .ok();
        })
    }

    pub fn new(
        compositor_type: settings::CompositorType,
        present_consumer: Consumer<PresentData>,
    ) -> StrResult<Self> {
        let graphics_al = Arc::new(GraphicsAL2D::new(Some(0))?);

        let compositor_type = match compositor_type {
            settings::CompositorType::Custom => CompositorType::Custom {
                swap_texture_handle_sets_id_iter: 0..,
                swap_texture_handle_sets: HashMap::new(),
            },
            settings::CompositorType::Runtime => CompositorType::Runtime,
        };

        let present_consumer = Arc::new(Mutex::new(present_consumer));
        let rendering_loop = Self::empty_rendering_loop(present_consumer.clone());

        Ok(Self {
            graphics_al,
            swap_textures: HashMap::new(),
            compositor_type,
            present_consumer,
            rendering_loop,
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
        if let CompositorType::Custom {
            swap_texture_handle_sets_id_iter,
            swap_texture_handle_sets,
        } = &mut self.compositor_type
        {
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

            let id = trace_none!(swap_texture_handle_sets_id_iter.next(), "Overflow")?;
            swap_texture_handle_sets.insert(id, handles);
            Ok((id, handles))
        } else {
            Err("Invalid operation".into())
        }
    }

    pub fn destroy_swap_texture_set(&mut self, id: usize) -> StrResult<()> {
        if let CompositorType::Custom {
            swap_texture_handle_sets,
            ..
        } = &mut self.compositor_type
        {
            if let Some(handles) = swap_texture_handle_sets.remove(&id) {
                for handle in handles.iter() {
                    self.swap_textures.remove(handle);
                }
            }
            Ok(())
        } else {
            Err("Invalid operation".into())
        }
    }

    pub fn initialize_for_client(
        &mut self,
        target_eye_width: u32,
        target_eye_height: u32,
        ffr_desc: Option<FfrDesc>,
        slice_producers: Vec<Producer<Texture>>,
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

        self.rendering_loop = thread_loop::spawn(|| {});

        Ok(())
    }

    // Calling this method is not mandatory but it makes the client reconnection faster
    pub fn deinitialize_for_client(&mut self) {
        self.rendering_loop = Self::empty_rendering_loop(self.present_consumer.clone());
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
