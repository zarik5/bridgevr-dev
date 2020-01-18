// #![allow(clippy::type_complexity)]

use super::settings::*;
use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, rendering::*, ring_channel::*};
use log::*;
use openvr_driver_sys as vr;
use parking_lot::Mutex;
use std::{
    ffi::*,
    mem::size_of,
    os::raw::*,
    ptr,
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

// The "contexts" are the structs given to the openvr callbacks and are internally mutable.
// Using internal mutability enables the callbacks to use the contexts concurrently.

#[derive(Default)]
pub struct AuxiliaryTextureData(#[cfg(target_os = "linux")] vr::VRVulkanTextureData_t);

unsafe impl Send for AuxiliaryTextureData {}
unsafe impl Sync for AuxiliaryTextureData {}

pub struct HmdContext {
    pub id: Mutex<Option<u32>>,
    pub display_component: Mutex<*mut vr::DisplayComponent>,
    pub virtual_display: Mutex<*mut vr::VirtualDisplay>,
    pub driver_direct_mode_component: Mutex<*mut vr::DriverDirectModeComponent>,
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

unsafe extern "C" fn get_window_bounds(
    context: *mut c_void,
    x: *mut i32,
    y: *mut i32,
    width: *mut u32,
    height: *mut u32,
) {
    let context = context as *const HmdContext;
    let (eye_width, eye_height) = (*context).settings.lock().target_eye_resolution;
    *x = 0;
    *y = 0;
    *width = eye_width * 2;
    *height = eye_height;
}

extern "C" fn return_false(_: *mut c_void) -> bool {
    false
}

unsafe extern "C" fn get_recommended_render_target_size(
    context: *mut c_void,
    width: *mut u32,
    height: *mut u32,
) {
    let context = context as *const HmdContext;
    let (eye_width, eye_height) = (*context).settings.lock().target_eye_resolution;
    *width = eye_width * 2;
    *height = eye_height;
}

unsafe extern "C" fn get_eye_output_viewport(
    context: *mut c_void,
    eye: vr::EVREye,
    x: *mut u32,
    y: *mut u32,
    width: *mut u32,
    height: *mut u32,
) {
    let context = context as *const HmdContext;
    let (eye_width, eye_height) = (*context).settings.lock().target_eye_resolution;
    *x = eye_width * (eye as u32);
    *y = 0;
    *width = eye_width;
    *height = eye_height;
}

unsafe extern "C" fn get_projection_raw(
    context: *mut c_void,
    eye: vr::EVREye,
    left: *mut f32,
    right: *mut f32,
    top: *mut f32,
    bottom: *mut f32,
) {
    let context = context as *const HmdContext;
    let settings = (*context).settings.lock();
    let eye = eye as usize;
    *left = settings.fov[eye].left;
    *right = settings.fov[eye].right;
    *top = settings.fov[eye].top;
    *bottom = settings.fov[eye].bottom;
}

extern "C" fn compute_distortion(
    _: *mut c_void,
    _: vr::EVREye,
    u: f32,
    v: f32,
) -> vr::DistortionCoordinates_t {
    vr::DistortionCoordinates_t {
        rfRed: [u, v],
        rfGreen: [u, v],
        rfBlue: [u, v],
    }
}

pub fn create_display_callbacks(hmd_context: Arc<HmdContext>) -> vr::DisplayComponentCallbacks {
    vr::DisplayComponentCallbacks {
        context: &*hmd_context as *const _ as _,
        GetWindowBounds: Some(get_window_bounds),
        IsDisplayOnDesktop: Some(return_false),
        IsDisplayRealDisplay: Some(return_false),
        GetRecommendedRenderTargetSize: Some(get_recommended_render_target_size),
        GetEyeOutputViewport: Some(get_eye_output_viewport),
        GetProjectionRaw: Some(get_projection_raw),
        ComputeDistortion: Some(compute_distortion),
    }
}

fn update_vsync(context: &HmdContext) {
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

extern "C" fn virtual_display_present(
    context: *mut c_void,
    present_info: *const vr::PresentInfo_t,
    _: u32,
) {
    let context = context as *const HmdContext;

    let handle = get_texture_handle(unsafe { (*present_info).backbufferTextureHandle });

    let maybe_texture = unsafe {
        (*context)
            .swap_texture_manager
            .lock()
            .get(handle)
            .or_else(|| {
                #[cfg(target_os = "linux")]
                let maybe_texture = {
                    let data =
                        &*(present_info.backbufferTextureHandle as *mut vr::VRVulkanTextureData_t);

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
                    Texture::from_handle(handle, (*context).graphics.clone()).map(Arc::new);

                if let Ok(texture) = maybe_texture.as_ref().map_err(|e| debug!("{}", e)) {
                    (*context)
                        .swap_texture_manager
                        .lock()
                        .add_single(texture.clone());
                }

                maybe_texture.ok()
            })
    };

    let maybe_frame_timing = {
        let mut frame_timings = vec![
            vr::Compositor_FrameTiming {
                m_nSize: size_of::<vr::Compositor_FrameTiming>() as _,
                ..<_>::default()
            };
            2
        ];

        // this function returns a number of frame timings <= frame_count.
        // frame_count is choosen == 2 > 1 to compensate for missed frames.
        let filled_count =
            unsafe { vr::vrServerDriverHostGetFrameTimings(frame_timings.as_mut_ptr(), 2) };

        if filled_count > 0 {
            Some(frame_timings[0])
        } else {
            None
        }
    };

    if let (Some(texture), Some(frame_timing), Some(present_producer)) =
        (&maybe_texture, &maybe_frame_timing, unsafe {
            &mut *(*context).present_producer.lock()
        })
    {
        let pose = pose_from_openvr_matrix(&frame_timing.m_HmdPose.mDeviceToAbsoluteTracking);

        let res = present_producer
            .fill(TIMEOUT, |present_data| {
                let [left_bounds, right_bounds] = VIRTUAL_DISPLAY_TEXTURE_BOUNDS;
                present_data.frame_index = unsafe { (*present_info).nFrameId };
                present_data.layers = vec![(
                    [
                        (texture.clone(), left_bounds),
                        (texture.clone(), right_bounds),
                    ],
                    pose,
                )];
                present_data.sync_texture = texture.clone();
                unsafe {
                    // NB: this lock is for writing in the contaner for the mutex
                    *(*context).current_sync_texture_mutex.lock() =
                        Some(present_data.sync_texture_mutex.clone());
                }

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
}

extern "C" fn wait_for_present(context: *mut c_void) {
    let context = context as *const HmdContext;

    if let Some(sync_texture_mutex) = unsafe { (*context).current_sync_texture_mutex.lock().take() }
    {
        if !sync_texture_mutex.wait_for_unlock(TIMEOUT) {
            debug!("Sync texture has not been unlocked");
        }
    };

    update_vsync(unsafe { &*context });
}

unsafe extern "C" fn get_time_since_last_vsync(
    context: *mut c_void,
    seconds_since_last_vsync: *mut f32,
    frame_counter: *mut u64,
) -> bool {
    let context = context as *const HmdContext;

    let (vsync_time, vsync_index) = &*(*context).latest_vsync.lock();
    *seconds_since_last_vsync = (Instant::now() - *vsync_time).as_secs_f32();
    *frame_counter = *vsync_index;

    true
}

pub fn create_virtual_display_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::VirtualDisplayCallbacks {
    vr::VirtualDisplayCallbacks {
        context: &*hmd_context as *const _ as _,
        Present: Some(virtual_display_present),
        WaitForPresent: Some(wait_for_present),
        GetTimeSinceLastVsync: Some(get_time_since_last_vsync),
    }
}

extern "C" fn create_swap_texture_set(
    context: *mut c_void,
    pid: u32,
    swap_texture_set_desc: *const vr::IVRDriverDirectModeComponent_SwapTextureSetDesc_t,
    shared_texture_handles: *mut [vr::SharedTextureHandle_t; 3],
) {
    let context = context as *const HmdContext;

    let maybe_swap_texture_set = unsafe {
        let format = format_from_native((*swap_texture_set_desc).nFormat);

        (*context)
            .swap_texture_manager
            .lock()
            .create_set(
                SWAP_TEXTURE_SET_SIZE,
                (
                    (*swap_texture_set_desc).nWidth,
                    (*swap_texture_set_desc).nHeight,
                ),
                format,
                (*swap_texture_set_desc).nSampleCount as _,
                pid,
            )
            .map_err(|e| error!("{}", e))
    };

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
        let shared_texture_handles_vec: Vec<_> = data.iter().map(|(handle, _)| *handle).collect();

        unsafe { (*shared_texture_handles).copy_from_slice(&shared_texture_handles_vec) };
    }
}

unsafe extern "C" fn destroy_swap_texture_set(
    context: *mut c_void,
    shared_texture_handle: vr::SharedTextureHandle_t,
) {
    let context = context as *const HmdContext;

    (*context)
        .swap_texture_manager
        .lock()
        .destroy_set_with_handle(get_texture_handle(shared_texture_handle));
}

unsafe extern "C" fn destroy_all_swap_texture_sets(context: *mut c_void, pid: u32) {
    let context = context as *const HmdContext;

    (*context)
        .swap_texture_manager
        .lock()
        .destroy_sets_with_pid(pid);
}

extern "C" fn get_next_swap_texture_set_index(
    _: *mut c_void,
    _shared_texture_handles: *mut [vr::SharedTextureHandle_t; 2],
    indices: *mut [u32; 2],
) {
    // shared_texture_handles can be ignored because there is always only one texture per
    // set used at any given time, so there are no race conditions.
    for idx in unsafe { (*indices).iter_mut() } {
        *idx = (*idx + 1) % 3;
    }
}

unsafe extern "C" fn submit_layer(
    context: *mut c_void,
    per_eye: *mut [vr::IVRDriverDirectModeComponent_SubmitLayerPerEye_t; 2],
    pose: *const vr::HmdMatrix34_t,
) {
    let context = context as *const HmdContext;

    let eyes_layer_data: Vec<_> = (*per_eye)
        .iter()
        .map(|eye_layer| {
            let b = eye_layer.bounds;
            let bounds = TextureBounds {
                u_min: b.uMin,
                v_min: b.vMin,
                u_max: b.uMax,
                v_max: b.vMax,
            };
            let texture = (*context)
                .swap_texture_manager
                .lock()
                .get(eye_layer.hTexture);
            (texture, bounds)
        })
        .collect();
    let pose = pose_from_openvr_matrix(&*pose);

    if let ((Some(left_texture), left_bounds), (Some(right_texture), right_bounds)) =
        (eyes_layer_data[0].clone(), eyes_layer_data[1].clone())
    {
        (*context).current_layers.lock().push((
            [(left_texture, left_bounds), (right_texture, right_bounds)],
            pose,
        ));
    }
}

extern "C" fn direct_mode_present(context: *mut c_void, sync_texture: vr::SharedTextureHandle_t) {
    let context = context as *const HmdContext;

    let sync_handle = get_texture_handle(sync_texture);
    if let (Some(present_producer), Some(sync_texture)) = unsafe {
        (
            &mut *(*context).present_producer.lock(),
            &mut (*context).swap_texture_manager.lock().get(sync_handle),
        )
    } {
        let res = present_producer
            .fill(TIMEOUT, |present_data| unsafe {
                present_data.frame_index = (*context).latest_vsync.lock().1;
                present_data.layers = (*context).current_layers.lock().drain(..).collect();
                present_data.sync_texture = sync_texture.clone();
                // NB: this lock is for writing in the contaner for the mutex
                *(*context).current_sync_texture_mutex.lock() =
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
}

unsafe extern "C" fn post_present(context: *mut c_void) {
    let context = context as *const HmdContext;

    if let Some(sync_texture_mutex) = (*context).current_sync_texture_mutex.lock().take() {
        if !sync_texture_mutex.wait_for_unlock(TIMEOUT) {
            debug!("Sync texture has not been unlocked");
        }
    };

    update_vsync(&*context);

    let (vsync_time, _) = &*(*context).latest_vsync.lock();
    thread::sleep((*vsync_time + (*context).settings.lock().frame_interval) - Instant::now());
}

extern "C" fn get_frame_timing(
    _: *mut c_void,
    _frame_timing: *mut vr::DriverDirectMode_FrameTiming,
) {
    // todo: do something here?
}

pub fn create_driver_direct_mode_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::DriverDirectModeComponentCallbacks {
    vr::DriverDirectModeComponentCallbacks {
        context: &*hmd_context as *const _ as _,
        CreateSwapTextureSet: Some(create_swap_texture_set),
        DestroySwapTextureSet: Some(destroy_swap_texture_set),
        DestroyAllSwapTextureSets: Some(destroy_all_swap_texture_sets),
        GetNextSwapTextureSetIndex: Some(get_next_swap_texture_set_index),
        SubmitLayer: Some(submit_layer),
        Present: Some(direct_mode_present),
        PostPresent: Some(post_present),
        GetFrameTiming: Some(get_frame_timing),
    }
}

unsafe extern "C" fn activate(context: *mut c_void, object_id: u32) -> vr::EVRInitError {
    let context = context as *const HmdContext;

    *(*context).id.lock() = Some(object_id);
    let container = vr::vrTrackedDeviceToPropertyContainer(object_id);

    //todo: set default props

    set_custom_props(container, &(*context).settings.lock().hmd_custom_properties);

    vr::VRInitError_None
}

unsafe extern "C" fn deactivate(context: *mut c_void) {
    let context = context as *const HmdContext;

    *(*context).id.lock() = None;

    (*context)
        .shutdown_signal_sender
        .lock()
        .send(ShutdownSignal::BackendShutdown)
        .map_err(|e| debug!("{}", e))
        .ok();
}

extern "C" fn empty_fn(_: *mut c_void) {}

unsafe extern "C" fn get_component(
    context: *mut c_void,
    component_name_and_version: *const c_char,
) -> *mut c_void {
    let context = context as *const HmdContext;

    let component_name_and_version_c_str = CStr::from_ptr(component_name_and_version);
    if component_name_and_version_c_str
        == CStr::from_bytes_with_nul_unchecked(vr::IVRDisplayComponent_Version)
    {
        *(*context).display_component.lock() as _
    } else if component_name_and_version_c_str
        == CStr::from_bytes_with_nul_unchecked(vr::IVRVirtualDisplay_Version)
    {
        *(*context).virtual_display.lock() as _
    } else if component_name_and_version_c_str
        == CStr::from_bytes_with_nul_unchecked(vr::IVRDriverDirectModeComponent_Version)
    {
        *(*context).driver_direct_mode_component.lock() as _
    } else {
        ptr::null_mut()
    }
}

extern "C" fn debug_request(_: *mut c_void, _: *const c_char, _: *mut c_char, _: u32) {
    // format!("debug request: {}", request)
}

extern "C" fn get_pose(context: *mut c_void) -> vr::DriverPose_t {
    let context = context as *const HmdContext;

    unsafe { *(*context).pose.lock() }
}

pub fn create_hmd_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::TrackedDeviceServerDriverCallbacks {
    vr::TrackedDeviceServerDriverCallbacks {
        context: &*hmd_context as *const _ as _,
        Activate: Some(activate),
        Deactivate: Some(deactivate),
        EnterStandby: Some(empty_fn),
        GetComponent: Some(get_component),
        DebugRequest: Some(debug_request),
        GetPose: Some(get_pose),
    }
}
