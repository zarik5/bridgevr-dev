use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, ring_channel::*, *};
use log::warn;
use openvr_driver as vr;
use parking_lot::Mutex;
use std::sync::{mpsc::*, Arc};

// const DEVICE_ID_FACTOR: usize = 256;

const DEFAULT_EYE_RESOLUTION: (u32, u32) = (640, 720);
const DEFAULT_FOV: [Fov; 2] = [Fov {
    left: 45_f32,
    top: 45_f32,
    right: 45_f32,
    bottom: 45_f32,
}; 2];
const DEFAULT_BLOCK_STANDBY: bool = false;
const DEFAULT_HMD_QUAT: vr::HmdQuaternion_t = vr::HmdQuaternion_t {
    w: 1_f64,
    x: 0_f64,
    y: 0_f64,
    z: 0_f64,
};

const DEFAULT_DRIVER_POSE: vr::DriverPose_t = vr::DriverPose_t {
    poseTimeOffset: 0_f64,
    qWorldFromDriverRotation: DEFAULT_HMD_QUAT,
    vecWorldFromDriverTranslation: [0_f64; 3],
    qDriverFromHeadRotation: DEFAULT_HMD_QUAT,
    vecDriverFromHeadTranslation: [0_f64; 3],
    vecPosition: [0_f64; 3],
    vecVelocity: [0_f64; 3],
    vecAcceleration: [0_f64; 3],
    qRotation: DEFAULT_HMD_QUAT,
    vecAngularVelocity: [0_f64; 3],
    vecAngularAcceleration: [0_f64; 3],
    result: vr::TrackingResult_Running_OK,
    poseIsValid: true,
    willDriftInYaw: false,
    shouldApplyHeadModel: false,
    deviceIsConnected: true,
};

fn create_openvr_settings(settings: Option<&Settings>, session: &SessionDesc) -> OpenvrSettings {
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
    } else if let Some(client_handshake_packet) = &session.last_client_handshake_packet {
        client_handshake_packet.native_eye_resolution
    } else {
        DEFAULT_EYE_RESOLUTION
    };

    let fov = if let Some(client_handshake_packet) = &session.last_client_handshake_packet {
        client_handshake_packet.fov
    } else {
        DEFAULT_FOV
    };

    let block_standby = if let Some(settings) = settings {
        settings.openvr.block_standby
    } else {
        DEFAULT_BLOCK_STANDBY
    };

    OpenvrSettings {
        target_eye_width,
        target_eye_height,
        fov,
        block_standby,
    }
}

pub struct OpenvrSettings {
    target_eye_width: u32,
    target_eye_height: u32,
    fov: [Fov; 2],
    block_standby: bool,
}

// The "contexts" are the structs given to the openvr callbacks and are internally mutable.
// Using internal mutability enables the callbacks to use the contexts concurrently.

pub struct HmdContext {
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    compositor: Arc<Mutex<Compositor>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
    pose: Mutex<vr::DriverPose_t>,
}

pub struct ControllerContext {
    id: Mutex<Option<u32>>,
    settings: Arc<Mutex<OpenvrSettings>>,
    pose: Mutex<vr::DriverPose_t>,
}

pub struct ServerContext {
    settings: Arc<Mutex<OpenvrSettings>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
    hmd: vr::TrackedDeviceServerDriver<HmdContext>,
    controllers: Vec<vr::TrackedDeviceServerDriver<ControllerContext>>,
}

pub struct OpenvrBackend {
    server: Arc<vr::ServerTrackedDeviceProvider<ServerContext>>,
    hmd_context: Arc<HmdContext>,
    controller_contexts: Vec<Arc<ControllerContext>>,
}

impl OpenvrBackend {
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
            shutdown_signal_sender: shutdown_signal_sender.clone(),
            pose: Mutex::new(DEFAULT_DRIVER_POSE),
        });

        let mut hmd_components = vr::Components::none();

        let openvr_display_component_callbacks = vr::DisplayComponentCallbacks {
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
        };
        let openvr_display_component =
            unsafe { vr::DisplayComponent::new(openvr_display_component_callbacks) };
        hmd_components.display = Some(openvr_display_component);

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
                let virtual_display_callbacks = vr::VirtualDisplayCallbacks {
                    context: hmd_context.clone(),
                    present: |context, present_info| {
                        // context.compositor.lock().unwrap().
                        // (&mut *context.present_callback.lock().unwrap())(
                        //     present_info.backbufferTextureHandle,
                        // );
                    },
                    // assumption: `wait_for_present` is called immediately after `present`
                    // todo: verify
                    wait_for_present: |context| {
                        // (&mut *context.wait_for_present_callback.lock().unwrap())();
                    },
                    get_time_since_last_vsync:
                        |context, seconds_since_last_vsync, frame_counter| {
                            // *seconds_since_last_vsync =
                            //     (Instant::now() - *context.last_vsync_time.lock().unwrap()).as_secs() as _;
                            // *frame_counter = *context.vsync_counter.lock().unwrap();
                            true
                        },
                };
                let virtual_display_component =
                    unsafe { vr::VirtualDisplay::new(virtual_display_callbacks) };
                hmd_components.virtual_display = Some(virtual_display_component);
            }
            CompositorType::Custom => {
                let driver_direct_mode_callbacks = vr::DriverDirectModeComponentCallbacks {
                    context: hmd_context.clone(),
                    create_swap_texture_set:
                        |context, pid, swap_texture_set_desc, shared_texture_handles| {},
                    destroy_swap_texture_set: |context, shared_texture_handle| {},
                    destroy_all_swap_texture_sets: |context, pid| {},
                    get_next_swap_texture_set_index: |context, shared_texture_handles, indices| {},
                    submit_layer: |context, per_eye, pose| {},
                    present: |context, sync_texture| {},
                    post_present: |context| {},
                    get_frame_timing: |context, frame_timing| {},
                };
                let driver_direct_mode_component =
                    unsafe { vr::DriverDirectModeComponent::new(driver_direct_mode_callbacks) };
                hmd_components.driver_direct_mode = Some(driver_direct_mode_component);
            }
        }

        let hmd_callbacks = vr::TrackedDeviceServerDriverCallbacks {
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
            deactivate: |_| (),
            enter_standby: |_| (),
            debug_request: |_, request| format!("debug request: {}", request),
            get_pose: |context| *context.pose.lock(),
        };

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
            let controller_callbacks = vr::TrackedDeviceServerDriverCallbacks {
                context: context.clone(),
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
            };

            let controller = unsafe {
                vr::TrackedDeviceServerDriver::new(controller_callbacks, vr::Components::none())
            };
        }

        let server_context = Arc::new(ServerContext {
            settings: openvr_settings,
            shutdown_signal_sender,
            hmd,
            controllers,
        });

        let server_callbacks = vr::ServerTrackedDeviceProviderCallbacks {
            context: server_context.clone(),
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
        };

        let server = Arc::new(unsafe { vr::ServerTrackedDeviceProvider::new(server_callbacks) });

        Self {
            server,
            hmd_context,
            controller_contexts,
        }
    }

    pub fn initialize_for_client(
        &mut self,
        settings: Settings,
        present_producer: Producer<PresentData>,
        wait_for_present_mutex: Arc<Mutex<()>>,
    ) {
    }

    pub fn deinitialize_for_client(&mut self) {}

    // pub fn request_shutdown(&mut self) {
    //     //todo: send vendor specific event {0,0} (shutdown)
    // }

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

    pub fn update_input(
        &mut self,
        hmd_motion: &MotionDesc,
        controllers_motion: &[MotionDesc],
        /*todo: digital input*/
    ) {
        self.update_pose(&self.hmd_context.id, hmd_motion);

        for (i, motion) in controllers_motion.iter().enumerate() {
            // let controller_context = &self.controllers[i].1;
            // self.update_pose(
            //     &controller_context.object_id,
            //     pose,
            //     &controller_context.pose,
            // );
        }
    }

    pub fn server_native(&self) -> Arc<vr::ServerTrackedDeviceProvider<ServerContext>> {
        self.server.clone()
    }
}
