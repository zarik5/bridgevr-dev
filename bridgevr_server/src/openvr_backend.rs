use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, ring_channel::*, *};
use log::warn;
use openvr_driver as vr;
use parking_lot::Mutex;
use std::{
    sync::{mpsc::*, Arc},
    thread,
    time::*,
};

const TIMEOUT: Duration = Duration::from_millis(500);

const DEFAULT_EYE_RESOLUTION: (u32, u32) = (640, 720);

const DEFAULT_FOV: [Fov; 2] = [Fov {
    left: 45_f32,
    top: 45_f32,
    right: 45_f32,
    bottom: 45_f32,
}; 2];

const DEFAULT_BLOCK_STANDBY: bool = false;

// todo: use ::from_secs_f32 if it will be a const fn
const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_nanos((1e9 / 60_f32) as u64);

const DEFAULT_HMD_QUATERNION: vr::HmdQuaternion_t = vr::HmdQuaternion_t {
    w: 1_f64,
    x: 0_f64,
    y: 0_f64,
    z: 0_f64,
};

const DEFAULT_DRIVER_POSE: vr::DriverPose_t = vr::DriverPose_t {
    poseTimeOffset: 0_f64,
    qWorldFromDriverRotation: DEFAULT_HMD_QUATERNION,
    vecWorldFromDriverTranslation: [0_f64; 3],
    qDriverFromHeadRotation: DEFAULT_HMD_QUATERNION,
    vecDriverFromHeadTranslation: [0_f64; 3],
    vecPosition: [0_f64; 3],
    vecVelocity: [0_f64; 3],
    vecAcceleration: [0_f64; 3],
    qRotation: DEFAULT_HMD_QUATERNION,
    vecAngularVelocity: [0_f64; 3],
    vecAngularAcceleration: [0_f64; 3],
    result: vr::TrackingResult_Running_OK,
    poseIsValid: true,
    willDriftInYaw: false,
    shouldApplyHeadModel: false,
    deviceIsConnected: true,
};

// on VirtualDislay interface the same texture is used for left and right eye.
const VIRTUAL_DISPLAY_TEXTURE_BOUNDS: [Bounds; 2] = [
    // left
    Bounds {
        u_min: 0_f32,
        v_min: 0_f32,
        u_max: 0.5_f32,
        v_max: 1_f32,
    },
    //right
    Bounds {
        u_min: 0.5_f32,
        v_min: 0_f32,
        u_max: 1_f32,
        v_max: 1_f32,
    },
];

struct OpenvrSettings {
    target_eye_width: u32,
    target_eye_height: u32,
    fov: [Fov; 2],
    block_standby: bool,
    frame_interval: Duration,
}

fn create_openvr_settings(
    settings: Option<&Settings>,
    session_desc: &SessionDesc,
) -> OpenvrSettings {
    let (target_eye_width, target_eye_height) = if let Some(Settings {
        openvr:
            OpenvrDesc {
                preferred_render_eye_resolution: Some(eye_res),
                ..
            },
        ..
    }) = settings
    {
        *eye_res
    } else if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
        client_handshake_packet.native_eye_resolution
    } else {
        DEFAULT_EYE_RESOLUTION
    };

    let fov = if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
        client_handshake_packet.fov
    } else {
        DEFAULT_FOV
    };

    let block_standby = if let Some(settings) = settings {
        settings.openvr.block_standby
    } else {
        DEFAULT_BLOCK_STANDBY
    };

    let frame_interval =
        if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
            Duration::from_secs_f32(1_f32 / client_handshake_packet.fps)
        } else {
            DEFAULT_FRAME_INTERVAL
        };

    OpenvrSettings {
        target_eye_width,
        target_eye_height,
        fov,
        block_standby,
        frame_interval,
    }
}

fn pose_from_matrix(matrix: &vr::HmdMatrix34_t) -> Pose {
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

struct HmdContext {
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    compositor: Arc<Mutex<Compositor>>,
    present_producer: Mutex<Option<Producer<PresentData>>>,
    sync_handle_mutex: Mutex<Option<Arc<Mutex<()>>>>,
    pose: Mutex<vr::DriverPose_t>,
    latest_vsync: Mutex<(Instant, u64)>,
    swap_texture_sets_desc: Mutex<Vec<(usize, [u64; 3], u32)>>,
    current_layers: Mutex<Vec<LayerDesc>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
}

fn create_display_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::DisplayComponentCallbacks<HmdContext> {
    vr::DisplayComponentCallbacks {
        context: hmd_context.clone(),
        get_window_bounds: |context, x, y, width, height| {
            let settings = context.settings.lock();
            *x = 0;
            *y = 0;
            *width = settings.target_eye_width * 2;
            *height = settings.target_eye_height;
        },
        is_display_on_desktop: |_| false,
        is_display_real_display: |_| false,
        get_recommended_render_target_size: |context, width, height| {
            let settings = context.settings.lock();
            *width = settings.target_eye_width * 2;
            *height = settings.target_eye_height;
        },
        get_eye_output_viewport: |context, eye, x, y, width, height| {
            let settings = context.settings.lock();
            *x = settings.target_eye_width * (eye as u32);
            *y = 0;
            *width = settings.target_eye_width;
            *height = settings.target_eye_height;
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

fn create_virtual_display_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::VirtualDisplayCallbacks<HmdContext> {
    vr::VirtualDisplayCallbacks {
        context: hmd_context,
        present: |context, present_info| {
            // this function returns a number of frame timings <= frame_count.
            // frame_count is choosen == 2 > 1 to compensate for missed frames.
            // todo: check if this function always return the latest n frame timings.
            let frame_timings = unsafe { vr::server_driver_host_get_frame_timings(2) };
            let maybe_frame_timing = frame_timings
                .iter()
                .rev()
                .find(|ft| ft.m_nFrameIndex == present_info.nFrameId as u32);
            if let Some(frame_timing) = maybe_frame_timing {
                let pose = pose_from_matrix(&frame_timing.m_HmdPose.mDeviceToAbsoluteTracking);
                if let Some(present_producer) = &mut *context.present_producer.lock() {
                    let res = present_producer.fill(TIMEOUT, |present_data| {
                        let handle = present_info.backbufferTextureHandle;
                        let [left_bounds, right_bounds] = VIRTUAL_DISPLAY_TEXTURE_BOUNDS;
                        present_data.frame_index = present_info.nFrameId;
                        present_data.layers =
                            vec![([(handle, left_bounds), (handle, right_bounds)], pose)];
                        present_data.sync_texture_handle = handle;
                        Ok(())
                    });
                    if res.is_ok() {
                        present_producer.wait_for_one(TIMEOUT).ok();
                    }
                }
            }
        },
        wait_for_present: |context| {
            // When the compositor has finished using the sync texture handle, this lock can
            // be taken and the callback can returns.
            let _ = context.sync_handle_mutex.lock();

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

fn create_driver_direct_mode_callbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::DriverDirectModeComponentCallbacks<HmdContext> {
    vr::DriverDirectModeComponentCallbacks {
        context: hmd_context,
        create_swap_texture_set: |context, pid, swap_texture_set_desc, shared_texture_handles| {
            let maybe_swap_texture_set = context.compositor.lock().create_swap_texture_set(
                swap_texture_set_desc.nWidth,
                swap_texture_set_desc.nHeight,
                swap_texture_set_desc.nFormat,
                swap_texture_set_desc.nSampleCount as _,
            );
            if let Ok((id, handles)) = display_err!(maybe_swap_texture_set) {
                context
                    .swap_texture_sets_desc
                    .lock()
                    .push((id, handles, pid));
                *shared_texture_handles = handles;
            }
        },
        destroy_swap_texture_set: |context, shared_texture_handle| {
            let mut swap_texture_sets_desc = context.swap_texture_sets_desc.lock();
            let maybe_set_desc_idx = swap_texture_sets_desc
                .iter()
                .position(|(_, handles, _)| handles.contains(&shared_texture_handle));
            if let Some(idx) = maybe_set_desc_idx {
                let (id, _, _) = swap_texture_sets_desc.remove(idx);
                display_err!(context.compositor.lock().destroy_swap_texture_set(id)).ok();
            }
        },
        destroy_all_swap_texture_sets: |context, pid| {
            let mut swap_texture_sets_desc = context.swap_texture_sets_desc.lock();
            let set_desc_idxs: Vec<_> = swap_texture_sets_desc
                .iter()
                .enumerate()
                .filter(|(_, (_, _, p))| *p == pid)
                .map(|(i, _)| i)
                .collect();
            for idx in set_desc_idxs {
                let (id, _, _) = swap_texture_sets_desc.remove(idx);
                display_err!(context.compositor.lock().destroy_swap_texture_set(id)).ok();
            }
        },
        get_next_swap_texture_set_index: |_, _shared_texture_handles, indices| {
            // shared_texture_handles can be ignored because there is always only one texture per
            // set used at any given time, so there are no race conditions.
            for idx in indices {
                *idx = (*idx + 1) % 3;
            }
        },
        submit_layer: |context, per_eye, pose| {
            let layer_per_eye: Vec<_> = per_eye
                .iter()
                .map(|eye_layer| {
                    let b = eye_layer.bounds;
                    let bounds = Bounds {
                        u_min: b.uMin,
                        v_min: b.vMin,
                        u_max: b.uMax,
                        v_max: b.vMax,
                    };
                    (eye_layer.hTexture, bounds)
                })
                .collect();
            let pose = pose_from_matrix(pose);
            context
                .current_layers
                .lock()
                .push(([layer_per_eye[0], layer_per_eye[1]], pose));
        },
        present: |context, sync_texture| {
            if let Some(present_producer) = &mut *context.present_producer.lock() {
                let res = present_producer.fill(TIMEOUT, |present_data| {
                    present_data.frame_index = context.latest_vsync.lock().1;
                    present_data.layers = context.current_layers.lock().drain(..).collect();
                    present_data.sync_texture_handle = sync_texture;
                    Ok(())
                });
                if res.is_ok() {
                    present_producer.wait_for_one(TIMEOUT).ok();
                }
            }
        },
        post_present: |context| {
            // use block to unlock mutex as soon as possible
            {
                let _ = context.sync_handle_mutex.lock();
            }

            update_vsync(&context);

            let (vsync_time, _) = &*context.latest_vsync.lock();
            thread::sleep((*vsync_time + context.settings.lock().frame_interval) - Instant::now());
        },
        // todo: do something here?
        get_frame_timing: |_, _frame_timing| (),
    }
}

fn create_hmd_cllbacks(
    hmd_context: Arc<HmdContext>,
) -> vr::TrackedDeviceServerDriverCallbacks<HmdContext> {
    vr::TrackedDeviceServerDriverCallbacks {
        context: hmd_context.clone(),
        activate: |context, object_id| {
            *context.id.lock() = Some(object_id);
            let container =
                unsafe { vr::properties_tracked_device_to_property_container(object_id) };

            //todo: set common props

            // OpenvrClient::set_custom_props(
            //     container,
            //     &context.settings.openvr.hmd_custom_properties,
            // );

            vr::VRInitError_None
        },
        deactivate: |context| {
            context
                .shutdown_signal_sender
                .lock()
                .send(ShutdownSignal::BackendShutdown)
                .ok();
        },
        enter_standby: |_| (),
        debug_request: |_, request| format!("debug request: {}", request),
        get_pose: |context| *context.pose.lock(),
    }
}

struct ControllerContext {
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    pose: Mutex<vr::DriverPose_t>,
}

fn create_controller_callbacks(
    controller_context: Arc<ControllerContext>,
) -> vr::TrackedDeviceServerDriverCallbacks<ControllerContext> {
    vr::TrackedDeviceServerDriverCallbacks {
        context: controller_context,
        activate: |context, object_id| {
            *context.id.lock() = Some(object_id);
            let container =
                unsafe { vr::properties_tracked_device_to_property_container(object_id) };

            //todo: set common props

            // OpenvrClient::set_custom_props(
            //     container,
            //     &context.settings.openvr.hmd_custom_properties,
            // );

            vr::VRInitError_None
        },
        deactivate: |_| (),
        enter_standby: |_| (),
        debug_request: |_, request| format!("debug request: {}", request),
        get_pose: |context| *context.pose.lock(),
    }
}

pub struct ServerContext {
    settings: Arc<Mutex<OpenvrSettings>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
    hmd: vr::TrackedDeviceServerDriver<HmdContext>,
    controllers: Vec<vr::TrackedDeviceServerDriver<ControllerContext>>,
}

fn create_server_callbacks(
    server_context: Arc<ServerContext>,
) -> vr::ServerTrackedDeviceProviderCallbacks<ServerContext> {
    vr::ServerTrackedDeviceProviderCallbacks {
        context: server_context,
        init: |context, driver_context| {
            unsafe { vr::init_server_driver_context(driver_context) };

            let mut device_id_iter = 0..;

            unsafe {
                vr::server_driver_host_tracked_device_added(
                    &(device_id_iter.next().unwrap()).to_string(),
                    vr::TrackedDeviceClass_HMD,
                    &context.hmd,
                )
            };

            vr::VRInitError_None
        },
        cleanup: |context| unsafe {
            context
                .shutdown_signal_sender
                .lock()
                .send(ShutdownSignal::BackendShutdown)
                .ok();

            vr::cleanup_driver_context();
        },
        run_frame: |_| (),
        should_block_standby_mode: |context| context.settings.lock().block_standby,
        enter_standby: |_| (),
        leave_standby: |_| (),
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
                OpenvrPropValue::Matrix34(_) => unimplemented!(),
            }
        };

        if res > 0 {
            warn!("Failed to set property {:?} with code={}", prop, res);
        }
    }
}

fn should_restart(old_settings: &OpenvrSettings, new_settings: &OpenvrSettings) -> bool {
    new_settings.target_eye_width != old_settings.target_eye_width
        || new_settings.target_eye_height != old_settings.target_eye_height
        || new_settings.fov != old_settings.fov
        || new_settings.frame_interval != old_settings.frame_interval
}

pub struct OpenvrBackend {
    server: Arc<vr::ServerTrackedDeviceProvider<ServerContext>>,
    hmd_context: Arc<HmdContext>,
    controller_contexts: Vec<Arc<ControllerContext>>,
}

impl OpenvrBackend {
    pub fn new(
        settings: Option<&Settings>,
        session_desc: &SessionDesc,
        compositor: Arc<Mutex<Compositor>>,
        shutdown_signal_sender: Sender<ShutdownSignal>,
    ) -> Self {
        let openvr_settings = Arc::new(Mutex::new(create_openvr_settings(settings, &session_desc)));
        let shutdown_signal_sender = Arc::new(Mutex::new(shutdown_signal_sender));

        let hmd_context = Arc::new(HmdContext {
            id: Mutex::new(None),
            settings: openvr_settings.clone(),
            compositor,
            present_producer: Mutex::new(None),
            sync_handle_mutex: Mutex::new(None),
            pose: Mutex::new(DEFAULT_DRIVER_POSE),
            latest_vsync: Mutex::new((Instant::now(), 0)),
            swap_texture_sets_desc: Mutex::new(vec![]),
            current_layers: Mutex::new(vec![]),
            shutdown_signal_sender: shutdown_signal_sender.clone(),
        });

        let mut hmd_components = vr::Components::none();

        let display_callbacks = create_display_callbacks(hmd_context.clone());
        let display_component = unsafe { vr::DisplayComponent::new(display_callbacks) };
        hmd_components.display = Some(display_component);

        let compositor_type = if let Some(settings) = &settings {
            if cfg!(target_os = "linux") {
                if let CompositorType::SteamVR = settings.openvr.compositor_type {
                    warn!("SteamVR compositor is not supported on linux. Using custom compositor.")
                }
                CompositorType::Custom
            } else {
                settings.openvr.compositor_type
            }
        } else {
            CompositorType::Custom
        };

        match compositor_type {
            CompositorType::SteamVR => {
                let virtual_display_callbacks =
                    create_virtual_display_callbacks(hmd_context.clone());
                let virtual_display_component =
                    unsafe { vr::VirtualDisplay::new(virtual_display_callbacks) };
                hmd_components.virtual_display = Some(virtual_display_component);
            }
            CompositorType::Custom => {
                let driver_direct_mode_callbacks =
                    create_driver_direct_mode_callbacks(hmd_context.clone());
                let driver_direct_mode_component =
                    unsafe { vr::DriverDirectModeComponent::new(driver_direct_mode_callbacks) };
                hmd_components.driver_direct_mode = Some(driver_direct_mode_component);
            }
        }

        let hmd_callbacks = create_hmd_cllbacks(hmd_context.clone());

        let hmd = unsafe { vr::TrackedDeviceServerDriver::new(hmd_callbacks, hmd_components) };

        let controller_contexts = vec![
            Arc::new(ControllerContext {
                id: Mutex::new(None),
                settings: openvr_settings.clone(),
                pose: Mutex::new(DEFAULT_DRIVER_POSE)
            });
            2
        ];
        let mut controllers = vec![];
        for context in &controller_contexts {
            let controller_callbacks = create_controller_callbacks(context.clone());

            let controller = unsafe {
                vr::TrackedDeviceServerDriver::new(controller_callbacks, vr::Components::none())
            };
            controllers.push(controller);
        }

        let server_context = Arc::new(ServerContext {
            settings: openvr_settings,
            shutdown_signal_sender,
            hmd,
            controllers,
        });

        let server_callbacks = create_server_callbacks(server_context);

        let server = Arc::new(unsafe { vr::ServerTrackedDeviceProvider::new(server_callbacks) });

        Self {
            server,
            hmd_context,
            controller_contexts,
        }
    }

    pub fn initialize_for_client_or_request_restart(
        &mut self,
        settings: &Settings,
        session_desc: &SessionDesc,
        present_producer: Producer<PresentData>,
        sync_handle_mutex: Arc<Mutex<()>>,
    ) {
        // the same openvr settings instance is shared between hmd, controllers and server.
        let new_settings = create_openvr_settings(Some(settings), session_desc);
        if should_restart(&*self.hmd_context.settings.lock(), &new_settings) {
            unsafe {
                vr::server_driver_host_request_restart(
                    "Client properties changed. Restarting SteamVR.",
                    "", // todo: steamvr_launcher,
                    "", // todo: steamvr_launcher_args,
                    "", // todo: steamvr_launcher_directory,
                );
                // shutdown signal will be generated from SteamVR
            }
        } else {
            *self.hmd_context.settings.lock() = new_settings;
            *self.hmd_context.present_producer.lock() = Some(present_producer);
            *self.hmd_context.sync_handle_mutex.lock() = Some(sync_handle_mutex);
        }
    }

    pub fn deinitialize_for_client(&mut self) {
        *self.hmd_context.present_producer.lock() = None;
        *self.hmd_context.sync_handle_mutex.lock() = None;
    }

    fn update_pose(&self, object_id: &Mutex<Option<u32>>, motion: &MotionDesc) {
        if let Some(id) = *object_id.lock() {
            // let mut driver_pose = driver_pose.lock().unwrap();
            // let p = new_pose;
            // *driver_pose = vr::DriverPose_t {
            //     vecWorldFromDriverTranslation: [p.0[3] as _, p.0[7] as _, p.0[11] as _],
            //     ..<_>::default() // todo: remove and implement manually
            // };

            // unsafe { vr::server_driver_host_tracked_device_pose_updated(id, &driver_pose) };
        }
    }

    pub fn update_input(&mut self, client_input: &ClientInput) {
        // self.update_pose(&self.hmd_context.id, hmd_motion);

        // for (i, motion) in controllers_motion.iter().enumerate() {
        //     // let controller_context = &self.controllers[i].1;
        //     // self.update_pose(
        //     //     &controller_context.object_id,
        //     //     pose,
        //     //     &controller_context.pose,
        //     // );
        // }
    }

    pub fn server_native(&self) -> Arc<vr::ServerTrackedDeviceProvider<ServerContext>> {
        self.server.clone()
    }
}
