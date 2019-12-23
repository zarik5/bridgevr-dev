use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, input_mapping::*, rendering::*, ring_channel::*};
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

struct OpenvrSettings {
    target_eye_resolution: (u32, u32),
    fov: [Fov; 2],
    block_standby: bool,
    frame_interval: Duration,
    hmd_custom_properties: Vec<OpenvrProp>,
    controllers_custom_properties: [Vec<OpenvrProp>; 2],
    input_mapping: [Vec<(String, InputType, Vec<String>)>; 2],
}

fn create_openvr_settings(
    settings: Option<&Settings>,
    session_desc: &SessionDesc,
) -> OpenvrSettings {
    let target_eye_resolution = if let Some(Settings {
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
            Duration::from_secs_f32(1_f32 / client_handshake_packet.fps as f32)
        } else {
            DEFAULT_FRAME_INTERVAL
        };

    let hmd_custom_properties = if let Some(settings) = settings {
        settings.openvr.hmd_custom_properties.clone()
    } else {
        vec![]
    };

    let controllers_custom_properties = if let Some(settings) = settings {
        settings.openvr.controllers_custom_properties.clone()
    } else {
        [vec![], vec![]]
    };

    let input_mapping = if let Some(settings) = settings {
        settings.openvr.input_mapping.clone()
    } else {
        [vec![], vec![]]
    };

    OpenvrSettings {
        target_eye_resolution,
        fov,
        block_standby,
        frame_interval,
        hmd_custom_properties,
        controllers_custom_properties,
        input_mapping,
    }
}

fn should_restart(old_settings: &OpenvrSettings, new_settings: &OpenvrSettings) -> bool {
    new_settings.fov != old_settings.fov
        || new_settings.frame_interval != old_settings.frame_interval
}

// The "contexts" are the structs given to the openvr callbacks and are internally mutable.
// Using internal mutability enables the callbacks to use the contexts concurrently.

struct HmdContext {
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    graphics: Arc<Mutex<Graphics>>,
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
                let pose =
                    pose_from_openvr_matrix(&frame_timing.m_HmdPose.mDeviceToAbsoluteTracking);
                if let Some(present_producer) = &mut *context.present_producer.lock() {
                    let res = present_producer
                        .fill(TIMEOUT, |present_data| {
                            let handle = present_info.backbufferTextureHandle;
                            let [left_bounds, right_bounds] = VIRTUAL_DISPLAY_TEXTURE_BOUNDS;
                            present_data.frame_index = present_info.nFrameId;
                            present_data.layers =
                                vec![([(handle, left_bounds), (handle, right_bounds)], pose)];
                            present_data.sync_texture_handle = handle;
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
            } else {
                debug!("frame timing not found");
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
            let format = format_from_native(swap_texture_set_desc.nFormat);
            let maybe_swap_texture_set = context
                .graphics
                .lock()
                .create_swap_texture_set(
                    (swap_texture_set_desc.nWidth, swap_texture_set_desc.nHeight),
                    format,
                    swap_texture_set_desc.nSampleCount as _,
                )
                .map_err(|e| error!("{}", e));

            if let Ok((id, handles)) = maybe_swap_texture_set {
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
                context
                    .graphics
                    .lock()
                    .destroy_swap_texture_set(id)
                    .map_err(|e| debug!("{}", e))
                    .ok();
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
                context
                    .graphics
                    .lock()
                    .destroy_swap_texture_set(id)
                    .map_err(|e| debug!("{}", e))
                    .ok();
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
                    let bounds = TextureBounds {
                        u_min: b.uMin,
                        v_min: b.vMin,
                        u_max: b.uMax,
                        v_max: b.vMax,
                    };
                    (eye_layer.hTexture, bounds)
                })
                .collect();
            let pose = pose_from_openvr_matrix(pose);
            context
                .current_layers
                .lock()
                .push(([layer_per_eye[0], layer_per_eye[1]], pose));
        },
        present: |context, sync_texture| {
            if let Some(present_producer) = &mut *context.present_producer.lock() {
                let res = present_producer
                    .fill(TIMEOUT, |present_data| {
                        present_data.frame_index = context.latest_vsync.lock().1;
                        present_data.layers = context.current_layers.lock().drain(..).collect();
                        present_data.sync_texture_handle = sync_texture;
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

fn create_hmd_callbacks(
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

struct ControllerContext {
    index: usize, // 0: left, 1: right
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    pose: Mutex<vr::DriverPose_t>,
    controller_input_to_component_map: Mutex<HashMap<String, vr::VRInputComponentHandle_t>>,
    haptic_component: Mutex<vr::VRInputComponentHandle_t>,
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
            unsafe {
                vr::init_server_driver_context(driver_context);

                vr::server_driver_host_tracked_device_added(
                    "0", // HMD device must always have ID = "0"
                    vr::TrackedDeviceClass_HMD,
                    &context.hmd,
                );

                for (idx, controller) in context.controllers.iter().enumerate() {
                    vr::server_driver_host_tracked_device_added(
                        &(idx + 1).to_string(),
                        vr::TrackedDeviceClass_Controller,
                        &controller,
                    );
                }
            }

            vr::VRInitError_None
        },
        cleanup: |context| unsafe {
            context
                .shutdown_signal_sender
                .lock()
                .send(ShutdownSignal::BackendShutdown)
                .map_err(|e| debug!("{}", e))
                .ok();

            vr::cleanup_driver_context();
        },
        run_frame: |_| (),
        should_block_standby_mode: |context| context.settings.lock().block_standby,
        enter_standby: |_| (),
        leave_standby: |_| (),
    }
}

type HapticCallback = Box<dyn FnMut(HapticData) + Send>;

pub struct OpenvrBackend {
    server: Arc<vr::ServerTrackedDeviceProvider<ServerContext>>,
    hmd_context: Arc<HmdContext>,
    controller_contexts: Vec<Arc<ControllerContext>>,
    haptic_callback: Arc<Mutex<Option<HapticCallback>>>,
}

impl OpenvrBackend {
    pub fn new(
        settings: Option<&Settings>,
        session_desc: &SessionDesc,
        graphics: Arc<Mutex<Graphics>>,
        shutdown_signal_sender: Sender<ShutdownSignal>,
    ) -> Self {
        let openvr_settings = Arc::new(Mutex::new(create_openvr_settings(settings, &session_desc)));
        let shutdown_signal_sender = Arc::new(Mutex::new(shutdown_signal_sender));

        let hmd_context = Arc::new(HmdContext {
            id: Mutex::new(None),
            settings: openvr_settings.clone(),
            graphics,
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

        let hmd_callbacks = create_hmd_callbacks(hmd_context.clone());

        let hmd = unsafe { vr::TrackedDeviceServerDriver::new(hmd_callbacks, hmd_components) };

        let controller_contexts: Vec<_> = (0..2)
            .map(|i| {
                Arc::new(ControllerContext {
                    index: i,
                    id: Mutex::new(None),
                    settings: openvr_settings.clone(),
                    pose: Mutex::new(DEFAULT_DRIVER_POSE),
                    controller_input_to_component_map: Mutex::new(HashMap::new()),
                    haptic_component: Mutex::new(vr::k_ulInvalidInputComponentHandle),
                })
            })
            .collect();
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
            haptic_callback: Arc::new(Mutex::new(None)),
        }
    }

    pub fn initialize_for_client_or_request_restart(
        &mut self,
        settings: &Settings,
        session_desc: &SessionDesc,
        present_producer: Producer<PresentData>,
        sync_handle_mutex: Arc<Mutex<()>>,
        haptic_callback: impl FnMut(HapticData) + Send + 'static,
    ) {
        // the same openvr settings instance is shared between hmd, controllers and server.
        let new_settings = create_openvr_settings(Some(settings), session_desc);
        if should_restart(&*self.hmd_context.settings.lock(), &new_settings) {
            unsafe {
                vr::server_driver_host_request_restart(
                    "Critical properties changed. Restarting SteamVR.",
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
            *self.haptic_callback.lock() = Some(Box::new(haptic_callback));

            // todo: notify settings changes to openvr
        }
    }

    pub fn deinitialize_for_client(&mut self) {
        *self.hmd_context.present_producer.lock() = None;
        *self.hmd_context.sync_handle_mutex.lock() = None;
        *self.haptic_callback.lock() = None;
    }

    fn update_pose(
        object_id: Option<u32>,
        motion: &MotionDesc,
        driver_pose: &mut vr::DriverPose_t,
        time_offset: Duration,
    ) {
        if let Some(id) = object_id {
            let p = motion.pose.position;
            let o = motion.pose.orientation;
            let v = motion.linear_velocity;
            let av = motion.angular_velocity;
            driver_pose.poseTimeOffset = time_offset.as_secs_f64();
            driver_pose.vecPosition = [p[0] as _, p[1] as _, p[2] as _];
            driver_pose.vecVelocity = [v[0] as _, v[1] as _, v[2] as _];
            driver_pose.qRotation = vr::HmdQuaternion_t {
                w: o[0] as _,
                x: o[1] as _,
                y: o[2] as _,
                z: o[3] as _,
            };
            driver_pose.vecAngularVelocity = [av[0] as _, av[1] as _, av[2] as _];

            unsafe { vr::server_driver_host_tracked_device_pose_updated(id, driver_pose) };
        }
    }

    pub fn update_input(&mut self, client_update: &ClientUpdate) {
        let time_offset = Duration::from_nanos(client_update.pose_time_offset_ns);
        Self::update_pose(
            *self.hmd_context.id.lock(),
            &client_update.hmd_motion,
            &mut self.hmd_context.pose.lock(),
            time_offset,
        );

        for (i, motion) in client_update.controllers_motion.iter().enumerate() {
            let context = &self.controller_contexts[i];
            Self::update_pose(
                *context.id.lock(),
                motion,
                &mut context.pose.lock(),
                time_offset,
            );
        }

        for ctx in &self.controller_contexts {
            let component_map = ctx.controller_input_to_component_map.lock();
            for (path, value) in input_device_data_to_str_value(&client_update.input_data) {
                if let Some(component) = component_map.get(path) {
                    unsafe {
                        match value {
                            InputValue::Boolean(value) => {
                                vr::driver_input_update_boolean(*component, value, 0_f64);
                            }
                            InputValue::NormalizedOneSided(value)
                            | InputValue::NormalizedTwoSided(value) => {
                                vr::driver_input_update_scalar(*component, value, 0_f64);
                            }
                            _ => todo!(),
                        }
                    }
                }
            }
        }

        // todo: do this elsewhere?
        while let Some(event) = unsafe { vr::server_driver_host_poll_next_event() } {
            if event.eventType == vr::VREvent_Input_HapticVibration as u32 {
                if let Some(callback) = &mut *self.haptic_callback.lock() {
                    for (i, ctx) in self.controller_contexts.iter().enumerate() {
                        let haptic = unsafe { event.data.hapticVibration };
                        if haptic.componentHandle == *ctx.haptic_component.lock() {
                            callback(HapticData {
                                hand: i as u8,
                                amplitude: haptic.fAmplitude,
                                duration_seconds: haptic.fDurationSeconds,
                                frequency: haptic.fFrequency,
                            });
                        }
                    }
                }
            }
        }
    }

    pub fn server_native(&self) -> Arc<vr::ServerTrackedDeviceProvider<ServerContext>> {
        self.server.clone()
    }
}
