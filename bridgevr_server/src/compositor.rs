#![allow(clippy::type_complexity)]

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
use parking_lot::*;
use std::{
    collections::{hash_map::*, VecDeque},
    ops::RangeFrom,
    sync::{atomic::*, Arc},
    time::Duration,
};

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

// Lock that can behave both as a spinlock and a mutex.
// This is used in place of a mutex to circumvent Rust's ownership system.
pub struct SpinLockableMutex {
    locked: AtomicBool,
    mutex: Mutex<()>,
}

impl SpinLockableMutex {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            mutex: Mutex::new(()),
        }
    }

    fn set_to_locked(&self) {
        self.locked.store(true, Ordering::Relaxed);
    }

    fn mutex_lock(&self) -> SpinLockableMutexGuard {
        self.set_to_locked();

        SpinLockableMutexGuard {
            mutex: self,
            _guard: self.mutex.lock(),
        }
    }

    // Return true if unlocked, false if failed to unlock
    // If locked == true and mutex is not locked, this method will spin lock.
    pub fn wait_for_unlock(&self, timeout: Duration) -> bool {
        while self.locked.load(Ordering::Relaxed) {
            if self.mutex.try_lock_for(timeout).is_none() {
                return false;
            }
        }
        true
    }
}

pub struct SpinLockableMutexGuard<'a> {
    mutex: &'a SpinLockableMutex,
    _guard: MutexGuard<'a, ()>,
}

impl<'a> Drop for SpinLockableMutexGuard<'a> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Relaxed);
    }
}

pub struct FrameSlice {
    pub index: u64,
    pub texture: Arc<Texture>,
    pub pose: Pose,
    pub force_idr: bool,
}

pub struct PresentData {
    pub frame_index: u64,
    pub layers: Vec<([(Arc<Texture>, TextureBounds); 2], Pose)>,
    pub sync_texture: Arc<Texture>,
    pub sync_texture_mutex: Arc<SpinLockableMutex>,
    pub force_idr_slice_idxs: Vec<usize>,
}

// TS is a texture auxiliary storage. For OpenVR this is VRVulkanTextureData_t
pub struct SwapTextureManager<TS = ()> {
    graphics: Arc<GraphicsContext>,
    textures: HashMap<u64, (usize, Arc<Texture>, Arc<Mutex<TS>>)>,
    handle_queue: VecDeque<u64>,
    handle_sets_id_iter: RangeFrom<usize>,
    handle_sets: HashMap<usize, (Vec<u64>, u32)>,
    max_single_textures: usize,
}

impl<TS: Default> SwapTextureManager<TS> {
    pub fn new(graphics: Arc<GraphicsContext>, max_single_textures: usize) -> Self {
        Self {
            graphics,
            textures: HashMap::new(),
            handle_queue: VecDeque::new(),
            handle_sets_id_iter: 0..,
            handle_sets: HashMap::new(),
            max_single_textures,
        }
    }

    pub fn add_single(&mut self, texture: Arc<Texture>) {
        let handle = texture.as_handle();
        self.textures.insert(handle, (0, texture, <_>::default()));
        self.handle_queue.push_back(handle);

        if self.handle_queue.len() > self.max_single_textures {
            if let Some(handle) = self.handle_queue.pop_front() {
                self.textures.remove(&handle);
            }
        }
    }

    pub fn create_set(
        &mut self,
        count: usize,
        resolution: (u32, u32),
        format: Format,
        sample_count: u8,
        pid: u32,
    ) -> StrResult<(usize, Vec<(u64, Arc<Mutex<TS>>)>)> {
        let set_id = trace_none!(self.handle_sets_id_iter.next(), "Overflow")?;

        let mut data = vec![];
        for _ in 0..count {
            let texture = Arc::new(Texture::new(
                self.graphics.clone(),
                resolution,
                format,
                sample_count,
            )?);
            let handle = texture.as_handle();
            let storage = Arc::new(Mutex::new(<_>::default()));
            self.textures
                .insert(handle, (set_id, texture.clone(), storage.clone()));
            self.handle_queue.push_back(handle);
            data.push((handle, storage));
        }

        let handles: Vec<_> = data.iter().map(|(h, _)| *h).collect();
        self.handle_sets.insert(set_id, (handles, pid));

        Ok((set_id, data))
    }

    pub fn destroy_set(&mut self, id: usize) {
        if let Some((handles, _)) = self.handle_sets.remove(&id) {
            for handle in handles {
                self.textures.remove(&handle);
            }
        }
    }

    pub fn destroy_set_with_handle(&mut self, handle: u64) {
        if let Some(&(set_id, _, _)) = self.textures.get(&handle) {
            self.destroy_set(set_id);
        }
    }

    pub fn destroy_sets_with_pid(&mut self, pid: u32) {
        let mut sets_to_remove = vec![];
        for (set_id, (_, p)) in &self.handle_sets {
            if *p == pid {
                sets_to_remove.push(*set_id);
            }
        }
        for set_id in sets_to_remove {
            self.destroy_set(set_id);
        }
    }

    pub fn get(&mut self, handle: u64) -> Option<Arc<Texture>> {
        self.textures
            .get(&handle)
            .map(|(_, texture, _)| texture.clone())
    }
}

pub struct CompositorDesc {
    pub target_eye_resolution: (u32, u32),
    pub filter_type: CompositionFilteringType,
    pub ffr_desc: Option<data::FoveatedRenderingDesc>,
}

pub struct Compositor {
    encoder_resolution: (u32, u32),
    rendering_loop: ThreadLoop,
}

impl Compositor {
    pub fn new(
        graphics: Arc<GraphicsContext>,
        settings: CompositorDesc,
        mut present_consumer: Consumer<PresentData>,
        mut slice_producers: Vec<Producer<FrameSlice>>,
    ) -> StrResult<Self> {
        let CompositorDesc {
            target_eye_resolution,
            filter_type,
            ffr_desc,
        } = settings;

        let composition_texture = Arc::new(Texture::new(
            graphics.clone(),
            target_eye_resolution,
            Format::Rgba8Unorm,
            1,
        )?);

        let mut rendering_operation_descs = vec![];

        let compressed_eye_resolution;
        let compressed_texture;
        match ffr_desc {
            Some(ffr_desc) => {
                compressed_eye_resolution =
                    ffr_compressed_eye_resolution(target_eye_resolution, ffr_desc);
                compressed_texture = Arc::new(Texture::new(
                    graphics.clone(),
                    compressed_eye_resolution,
                    Format::Rgba8Unorm,
                    1,
                )?);

                let ffr_operation_descs = ffr_compression_operation_descs(
                    composition_texture.clone(),
                    target_eye_resolution,
                    compressed_eye_resolution,
                );

                rendering_operation_descs.extend(ffr_operation_descs);
            }
            None => {
                compressed_eye_resolution = target_eye_resolution;
                compressed_texture = composition_texture.clone();
            }
        }

        let (compressed_eye_width, compressed_eye_height) = compressed_eye_resolution;
        let compressed_frame_resolution = (compressed_eye_width * 2, compressed_eye_height);

        let slices_desc =
            slices_desc_from_count(slice_producers.len(), compressed_frame_resolution);
        let encoder_resolution = aligned_resolution(slices_desc.single_resolution);

        for (idx, prod) in slice_producers.iter_mut().enumerate() {
            let slice_texture = Arc::new(Texture::new(
                graphics.clone(),
                encoder_resolution,
                Format::Rgba8Unorm,
                1,
            )?);

            prod.add(FrameSlice {
                index: 0,
                texture: slice_texture.clone(),
                pose: <_>::default(),
                force_idr: false,
            });

            let start = get_slice_start(idx, &slices_desc);
            let bounds = slice_bounds_to_texture_bounds(
                compressed_frame_resolution,
                start,
                encoder_resolution,
            );
            let copy_operation = OperationDesc::CopyTexture {
                input: compressed_texture.clone(),
                bounds,
                output: slice_texture.clone(),
            };

            rendering_operation_descs.push(copy_operation);
        }

        let rendering_operation_buffer =
            OperationBuffer::new(graphics, &rendering_operation_descs)?;

        let mut layers_buffers_history: Vec<(
            Vec<[Arc<Texture>; 2]>,
            (OperationBuffer, Vec<Arc<UniformBuffer>>),
        )> = vec![];
        let mut render = move || -> UnitResult {
            let mut frame_index = 0;
            let mut force_idr_slice_idxs = vec![];
            let pose;
            {
                let mut maybe_sync_texture_mutex = None;
                let mut maybe_sync_texture = None;
                let mut layers = vec![];
                present_consumer
                    .consume(TIMEOUT, |present_data| {
                        layers = present_data.layers.clone();
                        frame_index = present_data.frame_index;
                        force_idr_slice_idxs = present_data.force_idr_slice_idxs.clone();

                        present_data.sync_texture_mutex.set_to_locked();
                        maybe_sync_texture_mutex = Some(present_data.sync_texture_mutex.clone());

                        // return the present_data to openvr regardless the sync texture could be
                        // taken
                        present_data
                            .sync_texture
                            .acquire_sync(TIMEOUT)
                            .map_err(|e| debug!("{}", e))
                            .ok();
                        maybe_sync_texture = Some(present_data.sync_texture.clone());

                        Ok(())
                    })
                    .map_err(|e| debug!("{:?}", e))?;

                let sync_texture = maybe_sync_texture.ok_or(())?;
                let sync_texture_mutex = maybe_sync_texture_mutex.ok_or(())?;
                let _sync_texture_guard = sync_texture_mutex.mutex_lock();

                let graphics = sync_texture.graphics();

                let current_layers_textures: Vec<_> = layers
                    .iter()
                    .map(|([(lt, _), (rt, _)], _)| [lt.clone(), rt.clone()])
                    .collect();

                let maybe_layers_buffers = layers_buffers_history
                    .iter()
                    .find(|(l, _)| *l == current_layers_textures)
                    .map(|(_, bufs)| bufs);

                let (operation_buffer, uniform_buffers) = if let Some(bufs) = maybe_layers_buffers {
                    bufs
                } else {
                    if layers_buffers_history.len() >= 3 {
                        layers_buffers_history.clear();
                    }

                    let mut operation_descs = vec![];
                    let mut uniform_buffers = vec![];
                    for (idx, (eye_layers, _)) in layers.iter().enumerate() {
                        let [(left_texture, _), (right_texture, _)] = eye_layers;

                        let bounds_uniform_buffer = Arc::new(
                            UniformBuffer::new::<[TextureBounds; 2]>(graphics.clone())
                                .map_err(|e| error!("{}", e))?,
                        );

                        operation_descs.push(get_copy_eye_layers_operation_desc(
                            [left_texture.clone(), right_texture.clone()],
                            bounds_uniform_buffer.clone(),
                            filter_type,
                            composition_texture.clone(),
                            idx == 0,
                        ));
                        uniform_buffers.push(bounds_uniform_buffer)
                    }

                    let operation_buffer = OperationBuffer::new(graphics.clone(), &operation_descs)
                        .map_err(|e| error!("{}", e))?;

                    layers_buffers_history
                        .push((current_layers_textures, (operation_buffer, uniform_buffers)));
                    // unwrap is safe because I just added an element.
                    let (_, bufs) = layers_buffers_history.last().unwrap();
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

                sync_texture.release_sync();
                // here sync_texture_mutex lock is released
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

        Ok(Self {
            rendering_loop,
            encoder_resolution,
        })
    }

    pub fn encoder_resolution(&self) -> (u32, u32) {
        self.encoder_resolution
    }

    pub fn request_stop(&mut self) {
        self.rendering_loop.request_stop()
    }
}
