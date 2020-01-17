#![allow(clippy::type_complexity)]

use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, rendering::*, ring_channel::*};
use log::*;
use openvr_driver as vr;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{mpsc::*, Arc},
    thread,
    time::*,
};

const TIMEOUT: Duration = Duration::from_millis(500);

const SWAP_TEXTURE_SET_SIZE: usize = 3;

// On VirtualDislay interface the same texture is used for left and right eye.
const VIRTUAL_DISPLAY_TEXTURE_BOUNDS: [TextureBounds; 2] = [
    // left
    TextureBounds {
        u_min: 0_f32,
        v_min: 0_f32,
        u_max: 0.5_f32,
        v_max: 1_f32,
    },
    //right
    TextureBounds {
        u_min: 0.5_f32,
        v_min: 0_f32,
        u_max: 1_f32,
        v_max: 1_f32,
    },
];

const HAPTIC_PATH: &str = "/output/haptic";

fn pose_from_openvr_matrix(matrix: &vr::HmdMatrix34_t) -> Pose {
    use nalgebra::{Matrix3, UnitQuaternion};

    let m = matrix.m;
    let na_matrix = Matrix3::new(
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
    );
    let na_quat = UnitQuaternion::from_matrix(&na_matrix);
    let orientation = [na_quat[3], na_quat[0], na_quat[1], na_quat[2]];
    let position = [m[0][3], m[1][3], m[2][3]];

    Pose {
        position,
        orientation,
    }
}

pub struct OpenvrSettings {
    pub target_eye_resolution: (u32, u32),
    pub fov: [Fov; 2],
    pub block_standby: bool,
    pub frame_interval: Duration,
    pub hmd_custom_properties: Vec<OpenvrProp>,
    pub controllers_custom_properties: [Vec<OpenvrProp>; 2],
    pub input_mapping: [Vec<(String, InputType, Vec<String>)>; 2],
}

// The "contexts" are the structs given to the openvr callbacks and are internally mutable.
// Using internal mutability enables the callbacks to use the contexts concurrently.

#[derive(Default)]
pub struct AuxiliaryTextureData(#[cfg(target_os = "linux")] vr::VRVulkanTextureData_t);

unsafe impl Send for AuxiliaryTextureData {}
unsafe impl Sync for AuxiliaryTextureData {}

pub struct HmdContext {
    pub id: Mutex<Option<u32>>,
    pub settings: Arc<Mutex<OpenvrSettings>>,
    pub graphics: Arc<GraphicsContext>,
    pub swap_texture_manager: Mutex<SwapTextureManager<AuxiliaryTextureData>>,
    pub present_producer: Mutex<Option<Producer<PresentData>>>,
    pub current_layers: Mutex<Vec<([(Arc<Texture>, TextureBounds); 2], Pose)>>,
    pub current_sync_texture_mutex: Mutex<Option<Arc<SpinLockableMutex>>>,
    pub pose: Mutex<vr::DriverPose_t>,
    pub latest_vsync: Mutex<(Instant, u64)>,
    pub shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
}

pub fn create_display_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::DisplayComponentCallbacks<HmdContext> {
    vr::DisplayComponentCallbacks {
        context: hmd_context,
        get_window_bounds: |context, x, y, width, height| {
            let (eye_width, eye_height) = context.settings.lock().target_eye_resolution;
            *x = 0;
            *y = 0;
            *width = eye_width * 2;
            *height = eye_height;
        },
        is_display_on_desktop: |_| false,
        is_display_real_display: |_| false,
        get_recommended_render_target_size: |context, width, height| {
            let (eye_width, eye_height) = context.settings.lock().target_eye_resolution;
            *width = eye_width * 2;
            *height = eye_height;
        },
        get_eye_output_viewport: |context, eye, x, y, width, height| {
            let (eye_width, eye_height) = context.settings.lock().target_eye_resolution;
            *x = eye_width * (eye as u32);
            *y = 0;
            *width = eye_width;
            *height = eye_height;
        },
        get_projection_raw: |context, eye, left, right, top, bottom| {
            let settings = context.settings.lock();
            let eye = eye as usize;
            *left = settings.fov[eye].left;
            *right = settings.fov[eye].right;
            *top = settings.fov[eye].top;
            *bottom = settings.fov[eye].bottom;
        },
        compute_distortion: |_, _, u, v| vr::DistortionCoordinates_t {
            rfRed: [u, v],
            rfGreen: [u, v],
            rfBlue: [u, v],
        },
    }
}

fn update_vsync(context: &Arc<HmdContext>) {
    let (vsync_time, vsync_index) = &mut *context.latest_vsync.lock();
    let new_vsync_time = *vsync_time + context.settings.lock().frame_interval;
    if new_vsync_time < Instant::now() {
        *vsync_time = new_vsync_time;
    }
    *vsync_index += 1;
}

fn get_texture_handle(vr_handle: vr::SharedTextureHandle_t) -> u64 {
    #[cfg(target_os = "linux")]
    unsafe {
        (*(vr_handle as *mut vr::VRVulkanTextureData_t)).m_nImage
    }
    #[cfg(not(target_os = "linux"))]
    vr_handle
}

pub fn create_virtual_display_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::VirtualDisplayCallbacks<HmdContext> {
    vr::VirtualDisplayCallbacks {
        context: hmd_context,
        present: |context, present_info| {
            let handle = get_texture_handle(present_info.backbufferTextureHandle);

            let maybe_texture = context.swap_texture_manager.lock().get(handle).or_else(|| {
                #[cfg(target_os = "linux")]
                let maybe_texture = {
                    let data = unsafe {
                        &*(present_info.backbufferTextureHandle as *mut vr::VRVulkanTextureData_t)
                    };

                    let format = format_from_native(data.m_nFormat);
                    Texture::from_shared_vulkan_ptrs(
                        data.m_nImage,
                        context.graphics.clone(),
                        data.m_pInstance as _,
                        data.m_pPhysicalDevice as _,
                        data.m_pDevice as _,
                        data.m_pQueue as _,
                        data.m_nQueueFamilyIndex,
                        (data.m_nWidth, data.m_nHeight),
                        format,
                        data.m_nSampleCount as _,
                    )
                    .map(Arc::new)
                };
                #[cfg(not(target_os = "linux"))]
                let maybe_texture =
                    Texture::from_handle(handle, context.graphics.clone()).map(Arc::new);

                if let Ok(texture) = maybe_texture.as_ref().map_err(|e| debug!("{}", e)) {
                    context
                        .swap_texture_manager
                        .lock()
                        .add_single(texture.clone());
                }

                maybe_texture.ok()
            });

            // this function returns a number of frame timings <= frame_count.
            // frame_count is choosen == 2 > 1 to compensate for missed frames.
            // todo: check if this function always return the latest n frame timings.
            let frame_timings = unsafe { vr::server_driver_host_get_frame_timings(2) };
            let maybe_frame_timing = frame_timings
                .iter()
                .rev()
                .find(|ft| ft.m_nFrameIndex == present_info.nFrameId as u32);

            if let (Some(texture), Some(frame_timing), Some(present_producer)) = (
                &maybe_texture,
                &maybe_frame_timing,
                &mut *context.present_producer.lock(),
            ) {
                let pose =
                    pose_from_openvr_matrix(&frame_timing.m_HmdPose.mDeviceToAbsoluteTracking);

                let res = present_producer
                    .fill(TIMEOUT, |present_data| {
                        let [left_bounds, right_bounds] = VIRTUAL_DISPLAY_TEXTURE_BOUNDS;
                        present_data.frame_index = present_info.nFrameId;
                        present_data.layers = vec![(
                            [
                                (texture.clone(), left_bounds),
                                (texture.clone(), right_bounds),
                            ],
                            pose,
                        )];
                        present_data.sync_texture = texture.clone();
                        // NB: this lock is for writing in the contaner for the mutex
                        *context.current_sync_texture_mutex.lock() =
                            Some(present_data.sync_texture_mutex.clone());

                        // todo force_idr

                        Ok(())
                    })
                    .map_err(|e| debug!("{:?}", e));
                if res.is_ok() {
                    present_producer
                        .wait_for_one(TIMEOUT)
                        .map_err(|e| debug!("{:?}", e))
                        .ok();
                }
            } else if maybe_frame_timing.is_none() {
                debug!("frame timing not found");
            }
        },
        wait_for_present: |context| {
            if let Some(sync_texture_mutex) = context.current_sync_texture_mutex.lock().take() {
                if !sync_texture_mutex.wait_for_unlock(TIMEOUT) {
                    debug!("Sync texture has not been unlocked");
                }
            };

            update_vsync(&context);
        },
        get_time_since_last_vsync: |context, seconds_since_last_vsync, frame_counter| {
            let (vsync_time, vsync_index) = &*context.latest_vsync.lock();
            *seconds_since_last_vsync = (Instant::now() - *vsync_time).as_secs_f32();
            *frame_counter = *vsync_index;

            true
        },
    }
}

pub fn create_driver_direct_mode_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::DriverDirectModeComponentCallbacks<HmdContext> {
    vr::DriverDirectModeComponentCallbacks {
        context: hmd_context,
        create_swap_texture_set: |context, pid, swap_texture_set_desc, shared_texture_handles| {
            let format = format_from_native(swap_texture_set_desc.nFormat);
            let maybe_swap_texture_set = context
                .swap_texture_manager
                .lock()
                .create_set(
                    SWAP_TEXTURE_SET_SIZE,
                    (swap_texture_set_desc.nWidth, swap_texture_set_desc.nHeight),
                    format,
                    swap_texture_set_desc.nSampleCount as _,
                    pid,
                )
                .map_err(|e| error!("{}", e));

            if let Ok((_, data)) = maybe_swap_texture_set {
                #[cfg(target_os = "linux")]
                let shared_texture_handles_vec: Vec<_> = {
                    let instance_ptr = context.graphics.instance_ptr();
                    let physical_device_ptr = context.graphics.physical_device_ptr();
                    let device_ptr = context.graphics.device_ptr();
                    let queue_ptr = context.graphics.queue_ptr();
                    let queue_family_index = context.graphics.queue_family_index();

                    data.iter()
                        .map(|(handle, storage)| {
                            let AuxiliaryTextureData(vulkan_data) = &mut *storage.lock();

                            vulkan_data.m_nImage = *handle;
                            vulkan_data.m_pInstance = instance_ptr as _;
                            vulkan_data.m_pPhysicalDevice = physical_device_ptr as _;
                            vulkan_data.m_pDevice = device_ptr as _;
                            vulkan_data.m_pQueue = queue_ptr as _;
                            vulkan_data.m_nQueueFamilyIndex = queue_family_index;
                            vulkan_data as *mut _ as u64
                        })
                        .collect()
                };
                #[cfg(not(target_os = "linux"))]
                let shared_texture_handles_vec: Vec<_> =
                    data.iter().map(|(handle, _)| *handle).collect();

                shared_texture_handles.copy_from_slice(&shared_texture_handles_vec);
            }
        },
        destroy_swap_texture_set: |context, shared_texture_handle| {
            context
                .swap_texture_manager
                .lock()
                .destroy_set_with_handle(get_texture_handle(shared_texture_handle));
        },
        destroy_all_swap_texture_sets: |context, pid| {
            context
                .swap_texture_manager
                .lock()
                .destroy_sets_with_pid(pid);
        },
        get_next_swap_texture_set_index: |_, _shared_texture_handles, indices| {
            // shared_texture_handles can be ignored because there is always only one texture per
            // set used at any given time, so there are no race conditions.
            for idx in indices {
                *idx = (*idx + 1) % 3;
            }
        },
        submit_layer: |context, per_eye, pose| {
            let eyes_layer_data: Vec<_> = per_eye
                .iter()
                .map(|eye_layer| {
                    let b = eye_layer.bounds;
                    let bounds = TextureBounds {
                        u_min: b.uMin,
                        v_min: b.vMin,
                        u_max: b.uMax,
                        v_max: b.vMax,
                    };
                    let texture = context.swap_texture_manager.lock().get(eye_layer.hTexture);
                    (texture, bounds)
                })
                .collect();
            let pose = pose_from_openvr_matrix(pose);

            if let ((Some(left_texture), left_bounds), (Some(right_texture), right_bounds)) =
                (eyes_layer_data[0].clone(), eyes_layer_data[1].clone())
            {
                context.current_layers.lock().push((
                    [(left_texture, left_bounds), (right_texture, right_bounds)],
                    pose,
                ));
            }
        },
        present: |context, sync_texture| {
            let sync_handle = get_texture_handle(sync_texture);
            if let (Some(present_producer), Some(sync_texture)) = (
                &mut *context.present_producer.lock(),
                &mut context.swap_texture_manager.lock().get(sync_handle),
            ) {
                let res = present_producer
                    .fill(TIMEOUT, |present_data| {
                        present_data.frame_index = context.latest_vsync.lock().1;
                        present_data.layers = context.current_layers.lock().drain(..).collect();
                        present_data.sync_texture = sync_texture.clone();
                        // NB: this lock is for writing in the contaner for the mutex
                        *context.current_sync_texture_mutex.lock() =
                            Some(present_data.sync_texture_mutex.clone());

                        // todo force_idr

                        Ok(())
                    })
                    .map_err(|e| debug!("{:?}", e));
                if res.is_ok() {
                    present_producer
                        .wait_for_one(TIMEOUT)
                        .map_err(|e| debug!("{:?}", e))
                        .ok();
                }
            }
        },
        post_present: |context| {
            if let Some(sync_texture_mutex) = context.current_sync_texture_mutex.lock().take() {
                if !sync_texture_mutex.wait_for_unlock(TIMEOUT) {
                    debug!("Sync texture has not been unlocked");
                }
            };

            update_vsync(&context);

            let (vsync_time, _) = &*context.latest_vsync.lock();
            thread::sleep((*vsync_time + context.settings.lock().frame_interval) - Instant::now());
        },
        // todo: do something here?
        get_frame_timing: |_, _frame_timing| (),
    }
}

fn set_custom_props(container: vr::PropertyContainerHandle_t, props: &[OpenvrProp]) {
    for prop in props {
        let res = unsafe {
            match &prop.value {
                OpenvrPropValue::Bool(value) => {
                    vr::properties_set_bool(container, prop.code as _, *value)
                }
                OpenvrPropValue::Int32(value) => {
                    vr::properties_set_i32(container, prop.code as _, *value)
                }
                OpenvrPropValue::Uint64(value) => {
                    vr::properties_set_u64(container, prop.code as _, *value)
                }
                OpenvrPropValue::Float(value) => {
                    vr::properties_set_f32(container, prop.code as _, *value)
                }
                OpenvrPropValue::String(value) => {
                    vr::properties_set_str(container, prop.code as _, value)
                }
                OpenvrPropValue::Vector3(value) => vr::properties_set_hmd_vec3(
                    container,
                    prop.code as _,
                    &vr::HmdVector3_t { v: *value },
                ),
                OpenvrPropValue::Matrix34(_) => todo!(),
            }
        };

        if res > 0 {
            warn!("Failed to set openvr property {:?} with code={}", prop, res);
        }
    }
}

pub fn create_hmd_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::TrackedDeviceServerDriverCallbacks<HmdContext> {
    vr::TrackedDeviceServerDriverCallbacks {
        context: hmd_context,
        activate: |context, object_id| {
            *context.id.lock() = Some(object_id);
            let container =
                unsafe { vr::properties_tracked_device_to_property_container(object_id) };

            //todo: set default props

            set_custom_props(container, &context.settings.lock().hmd_custom_properties);

            vr::VRInitError_None
        },
        deactivate: |context| {
            *context.id.lock() = None;

            context
                .shutdown_signal_sender
                .lock()
                .send(ShutdownSignal::BackendShutdown)
                .map_err(|e| debug!("{}", e))
                .ok();
        },
        enter_standby: |_| (),
        debug_request: |_, request| format!("debug request: {}", request),
        get_pose: |context| *context.pose.lock(),
    }
}

pub struct ControllerContext {
    pub index: usize, // 0: left, 1: right
    pub id: Mutex<Option<u32>>,
    pub settings: Arc<Mutex<OpenvrSettings>>,
    pub pose: Mutex<vr::DriverPose_t>,
    pub controller_input_to_component_map: Mutex<HashMap<String, vr::VRInputComponentHandle_t>>,
    pub haptic_component: Mutex<vr::VRInputComponentHandle_t>,
}

pub fn create_controller_callbacks(
    controller_context: Arc<ControllerContext>,
) -> vr::TrackedDeviceServerDriverCallbacks<ControllerContext> {
    vr::TrackedDeviceServerDriverCallbacks {
        context: controller_context,
        activate: |context, object_id| {
            *context.id.lock() = Some(object_id);
            let container =
                unsafe { vr::properties_tracked_device_to_property_container(object_id) };

            //todo: set default props

            set_custom_props(
                container,
                &context.settings.lock().controllers_custom_properties[context.index],
            );

            let mut component_map = context.controller_input_to_component_map.lock();
            for (path, input_type, controller_paths) in
                &context.settings.lock().input_mapping[context.index]
            {
                let maybe_component = unsafe {
                    match input_type {
                        InputType::Boolean => vr::driver_input_create_boolean(container, &path),
                        InputType::NormalizedOneSided => vr::driver_input_create_scalar(
                            container,
                            &path,
                            vr::VRScalarType_Absolute,
                            vr::VRScalarUnits_NormalizedOneSided,
                        ),
                        InputType::NormalizedTwoSided => vr::driver_input_create_scalar(
                            container,
                            &path,
                            vr::VRScalarType_Absolute,
                            vr::VRScalarUnits_NormalizedTwoSided,
                        ),
                        _ => todo!(),
                    }
                    .map_err(|e| warn!("Create {}: {}", path, e))
                };

                if let Ok(component) = maybe_component {
                    for controller_path in controller_paths {
                        component_map.insert(controller_path.to_owned(), component);
                    }
                }
            }

            let maybe_haptic_component =
                unsafe { vr::driver_input_create_haptic(container, HAPTIC_PATH) }
                    .map_err(|e| warn!("Create {}: {}", HAPTIC_PATH, e));
            if let Ok(component) = maybe_haptic_component {
                *context.haptic_component.lock() = component;
            }

            vr::VRInitError_None
        },
        deactivate: |context| *context.id.lock() = None,
        enter_standby: |_| (),
        debug_request: |_, request| format!("debug request: {}", request),
        get_pose: |context| *context.pose.lock(),
    }
}
