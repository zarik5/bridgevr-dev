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
use log::*;
use parking_lot::Mutex;
use std::{collections::hash_map::*, ops::RangeFrom, sync::Arc, time::Duration};

const TRACE_CONTEXT: &str = "Compositor";

const TIMEOUT: Duration = Duration::from_millis(100);

const COPY_EYE_LAYERS_NEAREST_SHADER_STR: &str = ""; // todo
const COPY_EYE_LAYERS_BILINEAR_SHADER_STR: &str = ""; // todo
const COPY_EYE_LAYERS_LANCZOS_SHADER_STR: &str = ""; // todo

fn get_copy_eye_layers_operation_desc(
    input_textures: [Arc<Texture>; 2],
    bounds_uniform_buffer: Arc<UniformBuffer>,
    filter_type: CompositionFilteringType,
    output_texture: Arc<Texture>,
    is_first: bool,
) -> OperationDesc {
    let shader = match filter_type {
        CompositionFilteringType::NearestNeighbour => COPY_EYE_LAYERS_NEAREST_SHADER_STR.to_owned(),
        CompositionFilteringType::Bilinear => COPY_EYE_LAYERS_BILINEAR_SHADER_STR.to_owned(),
        CompositionFilteringType::Lanczos => COPY_EYE_LAYERS_LANCZOS_SHADER_STR.to_owned(),
    };
    OperationDesc::Rendering {
        input_textures: input_textures.to_vec(),
        uniform_buffer: Some(bounds_uniform_buffer),
        shader,
        output_textures: vec![output_texture],
        alpha: !is_first,
    }
}

pub struct FrameSlice {
    pub index: u64,
    pub texture: Arc<Texture>,
    pub pose: Pose,
    pub force_idr: bool,
}

pub type LayerDesc = ([(u64, TextureBounds); 2], Pose);

pub struct PresentData {
    pub frame_index: u64,
    pub layers: Vec<LayerDesc>,
    pub sync_texture_handle: u64,
    pub force_idr_slice_idxs: Vec<usize>,
}

pub struct Graphics {
    graphics_al: Arc<GraphicsAbstractionLayer>,
    swap_textures: HashMap<u64, Arc<Texture>>,
    swap_texture_handle_sets_id_iter: RangeFrom<usize>,
    swap_texture_handle_sets: HashMap<usize, [u64; 3]>,
}

impl Graphics {
    pub fn new() -> StrResult<Self> {
        let graphics_al = Arc::new(GraphicsAbstractionLayer::new(Some(0))?);

        Ok(Self {
            graphics_al,
            swap_textures: HashMap::new(),
            swap_texture_handle_sets_id_iter: 0..,
            swap_texture_handle_sets: HashMap::new(),
        })
    }

    pub fn create_swap_texture_set(
        &mut self,
        width: u32,
        height: u32,
        format: Format,
        sample_count: u8,
    ) -> StrResult<(usize, [u64; 3])> {
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
            self.swap_textures.insert(*handle, Arc::new(texture));
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

    pub fn device_ptr(&self) -> u64 {
        self.graphics_al.device_ptr()
    }
}

pub struct CompositorSettings {
    pub target_eye_width: u32,
    pub target_eye_height: u32,
    pub filter_type: CompositionFilteringType,
    pub ffr_desc: Option<data::FoveatedRenderingDesc>,
}

pub struct Compositor {
    rendering_loop: ThreadLoop,
}

impl Compositor {
    pub fn new(
        graphics: Arc<Mutex<Graphics>>,
        settings: CompositorSettings,
        mut present_consumer: Consumer<PresentData>,
        sync_handle_mutex: Arc<Mutex<()>>,
        mut slice_producers: Vec<Producer<FrameSlice>>,
    ) -> StrResult<Self> {
        let CompositorSettings {
            target_eye_width,
            target_eye_height,
            filter_type,
            ffr_desc,
        } = settings;

        let graphics_al = graphics.lock().graphics_al.clone();

        let composition_texture = Arc::new(Texture::new(
            graphics_al.clone(),
            target_eye_width,
            target_eye_height,
            Format::Rgba8Unorm,
            None,
        )?);

        let mut rendering_operation_descs = vec![];

        let compressed_eye_width;
        let compressed_eye_height;
        let compressed_texture;
        match ffr_desc {
            Some(ffr_desc) => {
                let (width, height) =
                    ffr_compressed_eye_resolution(target_eye_width, target_eye_height, ffr_desc);
                compressed_eye_width = width;
                compressed_eye_height = height;
                compressed_texture = Arc::new(Texture::new(
                    graphics_al.clone(),
                    compressed_eye_width,
                    compressed_eye_height,
                    Format::Rgba8Unorm,
                    None,
                )?);

                let ffr_operation_descs = ffr_compression_operation_descs(
                    composition_texture.clone(),
                    target_eye_width,
                    target_eye_height,
                    compressed_eye_width,
                    compressed_eye_height,
                );

                rendering_operation_descs.extend(ffr_operation_descs);
            }
            None => {
                compressed_eye_width = target_eye_width;
                compressed_eye_height = target_eye_height;
                compressed_texture = composition_texture.clone();
            }
        }

        let compressed_frame_width = compressed_eye_width * 2;
        let compressed_frame_height = compressed_eye_height;

        let slices_desc = slices_desc_from_count(
            slice_producers.len(),
            compressed_frame_width,
            compressed_frame_height,
        );
        let (aligned_slice_width, aligned_slice_height) =
            aligned_resolution(slices_desc.single_width, slices_desc.single_height);

        for (idx, prod) in slice_producers.iter_mut().enumerate() {
            let slice_texture = Arc::new(Texture::new(
                graphics_al.clone(),
                aligned_slice_width,
                aligned_slice_height,
                Format::Rgba8Unorm,
                None,
            )?);

            prod.add(FrameSlice {
                index: 0,
                texture: slice_texture.clone(),
                pose: <_>::default(),
                force_idr: false,
            });

            let (start_x, start_y) = get_slice_start(idx, &slices_desc);
            let bounds = slice_bounds_to_texture_bounds(
                compressed_frame_width,
                compressed_frame_height,
                start_x,
                start_y,
                aligned_slice_width,
                aligned_slice_height,
            );
            let copy_operation = OperationDesc::CopyTexture {
                input: compressed_texture.clone(),
                bounds,
                output: slice_texture.clone(),
            };

            rendering_operation_descs.push(copy_operation);
        }

        let rendering_operation_buffer =
            OperationBuffer::new(graphics_al.clone(), &rendering_operation_descs)?;

        type Buffers = (OperationBuffer, Vec<Arc<UniformBuffer>>);
        let mut layers_buffers_vec: Vec<(Vec<[u64; 2]>, Buffers)> = vec![];
        let mut render = move || -> UnitResult {
            let mut frame_index = 0;
            let mut force_idr_slice_idxs = vec![];
            let pose;
            {
                let _guard = sync_handle_mutex.lock();
                let mut maybe_sync_texture_guard = None;
                let mut layers = vec![];
                present_consumer
                    .consume(TIMEOUT, |present_data| {
                        layers = present_data.layers.clone();
                        frame_index = present_data.frame_index;
                        force_idr_slice_idxs = present_data.force_idr_slice_idxs.clone();

                        maybe_sync_texture_guard =
                            lock_texture_from_handle(present_data.sync_texture_handle, TIMEOUT)
                                .map_err(|e| debug!("{}", e))
                                .ok();
                        // return the present_data to openvr regardless the sync texture could be
                        // taken
                        Ok(())
                    })
                    .map_err(|e| debug!("{:?}", e))?;

                let sync_texture_guard = maybe_sync_texture_guard.ok_or(())?;

                let layer_handles: Vec<_> = layers
                    .iter()
                    .map(|([(h1, _), (h2, _)], _)| [*h1, *h2])
                    .collect();

                let maybe_layers_buffers = layers_buffers_vec
                    .iter()
                    .find(|(l, _)| *l == layer_handles)
                    .map(|(_, bufs)| bufs);

                let (operation_buffer, uniform_buffers) = if let Some(bufs) = maybe_layers_buffers {
                    bufs
                } else {
                    if layers_buffers_vec.len() >= 3 {
                        layers_buffers_vec.clear();
                    }

                    let mut operation_descs = vec![];
                    let mut uniform_buffers = vec![];
                    for (idx, (layer_pair, _)) in layers.iter().enumerate() {
                        let swap_textures = &mut graphics.lock().swap_textures;
                        let mut texture_pair = vec![];
                        for (handle, _) in layer_pair.iter() {
                            let texture = match swap_textures.entry(*handle) {
                                Entry::Occupied(entry) => entry.into_mut(),
                                Entry::Vacant(entry) => {
                                    let texture = Arc::new(
                                        Texture::from_handle(*handle, graphics_al.clone())
                                            .map_err(|e| error!("{}", e))?,
                                    );
                                    entry.insert(texture)
                                }
                            };
                            texture_pair.push(texture.clone())
                        }
                        // texture_pair has always two elements so remove does not panic
                        let left_tex = texture_pair.remove(0);
                        let right_tex = texture_pair.remove(0);

                        let bounds_uniform_buffer = Arc::new(
                            UniformBuffer::new::<[TextureBounds; 2]>(graphics_al.clone())
                                .map_err(|e| error!("{}", e))?,
                        );

                        operation_descs.push(get_copy_eye_layers_operation_desc(
                            [left_tex, right_tex],
                            bounds_uniform_buffer.clone(),
                            filter_type,
                            composition_texture.clone(),
                            idx == 0,
                        ));
                        uniform_buffers.push(bounds_uniform_buffer)
                    }

                    let operation_buffer =
                        OperationBuffer::new(graphics_al.clone(), &operation_descs)
                            .map_err(|e| error!("{}", e))?;

                    layers_buffers_vec.push((layer_handles, (operation_buffer, uniform_buffers)));
                    // unwrap is safe because I just added an element.
                    let (_, bufs) = layers_buffers_vec.last().unwrap();
                    bufs
                };

                for (([(_, left_bounds), (_, right_bounds)], _), uniform_buffer) in
                    layers.iter().zip(uniform_buffers)
                {
                    uniform_buffer
                        .write(&[*left_bounds, *right_bounds])
                        .map_err(|e| error!("{}", e))?;
                }
                operation_buffer.execute();

                // Improvement: use pose to do reprojection
                pose = layers[0].1;

                unlock_texture(sync_texture_guard);
                // here sync_handle_mutex lock is released
            }
            rendering_operation_buffer.execute();

            for (idx, prod) in slice_producers.iter_mut().enumerate() {
                prod.fill(TIMEOUT, |slice| {
                    slice.index = frame_index;
                    slice.pose = pose;
                    slice.force_idr = force_idr_slice_idxs.contains(&idx);
                    Ok(())
                })
                .map_err(|e| debug!("{:?}", e))
                .ok(); // do not early return if error: let the other slices get submitted
            }

            Ok(())
        };

        let rendering_loop = thread_loop::spawn("Compositor loop", move || {
            render().ok();
        })?;

        Ok(Self { rendering_loop })
    }

    pub fn request_stop(&mut self) {
        self.rendering_loop.request_stop()
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
    //                 self.graphics.clone(),
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
    //                 todo!();
    //             } else {
    //                 vec![OperationDesc::CopyTexture {
    //                     input: shared_texture_ref.clone(),
    //                     output: self.encoder_input_texture.clone(),
    //                 }]
    //             };
    //             OperationBuffer::new(self.graphics.clone(), commands)
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
