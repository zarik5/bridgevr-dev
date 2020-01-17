#![allow(clippy::type_complexity)]

mod tracked_devices;

use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, input_mapping::*, rendering::*, ring_channel::*, sockets::*, *};
use log::*;
use openvr_driver as vr;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{mpsc::*, Arc},
    time::*,
};
use tracked_devices::*;

const DEFAULT_COMPOSITOR_TYPE: CompositorType = CompositorType::Custom;

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

const RESET_POSE_TIMING_THRESHOLD_NS: i64 = 50_000_000;

const VIRTUAL_DISPLAY_MAX_TEXTURES: usize = 3;

fn create_openvr_settings(
    settings: Option<&Settings>,
    session_desc: &SessionDesc,
) -> OpenvrSettings {
    let block_standby;
    let hmd_custom_properties;
    let controllers_custom_properties;
    let input_mapping;
    if let Some(settings) = settings {
        block_standby = settings.openvr.block_standby;
        hmd_custom_properties = settings.openvr.hmd_custom_properties.clone();
        controllers_custom_properties = settings.openvr.controllers_custom_properties.clone();
        input_mapping = settings.openvr.input_mapping.clone();
    } else {
        block_standby = DEFAULT_BLOCK_STANDBY;
        hmd_custom_properties = vec![];
        controllers_custom_properties = [vec![], vec![]];
        input_mapping = [vec![], vec![]];
    };

    let fov;
    let frame_interval;
    if let Some(client_handshake_packet) = &session_desc.last_client_handshake_packet {
        fov = client_handshake_packet.fov;
        frame_interval = Duration::from_secs_f32(1_f32 / client_handshake_packet.fps as f32);
    } else {
        fov = DEFAULT_FOV;
        frame_interval = DEFAULT_FRAME_INTERVAL;
    };

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

pub struct ServerContext {
    settings: Arc<Mutex<OpenvrSettings>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
    hmd: vr::TrackedDeviceServerDriver<HmdContext>,
    controllers: Vec<vr::TrackedDeviceServerDriver<ControllerContext>>,
    controller_contexts: Vec<Arc<ControllerContext>>,
    connection_manager: Mutex<Option<Arc<Mutex<ConnectionManager<ServerMessage>>>>>,
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
        run_frame: |context| {
            if let Some(connection_manager) = &mut *context.connection_manager.lock() {
                while let Some(event) = unsafe { vr::server_driver_host_poll_next_event() } {
                    if event.eventType == vr::VREvent_Input_HapticVibration as u32 {
                        for (i, ctx) in context.controller_contexts.iter().enumerate() {
                            let haptic = unsafe { event.data.hapticVibration };
                            if haptic.componentHandle == *ctx.haptic_component.lock() {
                                let haptic_data = HapticData {
                                    hand: i as u8,
                                    amplitude: haptic.fAmplitude,
                                    duration_seconds: haptic.fDurationSeconds,
                                    frequency: haptic.fFrequency,
                                };
                                connection_manager
                                    .lock()
                                    .send_message_udp(&ServerMessage::Haptic(haptic_data))
                                    .map_err(|e| debug!("{}", e))
                                    .ok();
                            }
                        }
                    }
                }
            }
        },
        should_block_standby_mode: |context| context.settings.lock().block_standby,
        enter_standby: |_| (),
        leave_standby: |_| (),
    }
}

pub struct OpenvrBackend {
    server: Arc<vr::ServerTrackedDeviceProvider<ServerContext>>,
    server_context: Arc<ServerContext>,
    hmd_context: Arc<HmdContext>,
    controller_contexts: Vec<Arc<ControllerContext>>,
    pose_timer: Instant,
    additional_pose_time_offset_ns: i64,
}

impl OpenvrBackend {
    pub fn new(
        graphics: Arc<GraphicsContext>,
        settings: Option<&Settings>,
        session_desc: &SessionDesc,
        shutdown_signal_sender: Sender<ShutdownSignal>,
    ) -> Self {
        let openvr_settings = Arc::new(Mutex::new(create_openvr_settings(settings, &session_desc)));
        let shutdown_signal_sender = Arc::new(Mutex::new(shutdown_signal_sender));

        let swap_texture_manager = Mutex::new(SwapTextureManager::new(
            graphics.clone(),
            VIRTUAL_DISPLAY_MAX_TEXTURES,
        ));

        let hmd_context = Arc::new(HmdContext {
            id: Mutex::new(None),
            settings: openvr_settings.clone(),
            graphics,
            swap_texture_manager,
            present_producer: Mutex::new(None),
            current_layers: Mutex::new(vec![]),
            current_sync_texture_mutex: Mutex::new(None),
            pose: Mutex::new(DEFAULT_DRIVER_POSE),
            latest_vsync: Mutex::new((Instant::now(), 0)),
            shutdown_signal_sender: shutdown_signal_sender.clone(),
        });

        let mut hmd_components = vr::Components::none();

        let display_callbacks = create_display_callbacks(hmd_context.clone());
        let display_component = unsafe { vr::DisplayComponent::new(display_callbacks) };
        hmd_components.display = Some(display_component);

        let compositor_type = if let Some(settings) = settings {
            settings.openvr.compositor_type
        } else {
            DEFAULT_COMPOSITOR_TYPE
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
            controller_contexts: controller_contexts.clone(),
            connection_manager: Mutex::new(None),
        });

        let server_callbacks = create_server_callbacks(server_context.clone());

        let server = Arc::new(unsafe { vr::ServerTrackedDeviceProvider::new(server_callbacks) });

        Self {
            server,
            server_context,
            hmd_context,
            controller_contexts,
            pose_timer: Instant::now(),
            additional_pose_time_offset_ns: 0,
        }
    }

    pub fn initialize_for_client_or_request_restart(
        &mut self,
        settings: &Settings,
        session_desc: &SessionDesc,
        present_producer: Producer<PresentData>,
        // haptic_callback: impl FnMut(HapticData) + Send + 'static,
        connection_manager: Arc<Mutex<ConnectionManager<ServerMessage>>>,
    ) -> StrResult {
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
            *self.server_context.connection_manager.lock() = Some(connection_manager);

            // todo: notify settings changes to openvr using properties
        }
        Ok(())
    }

    pub fn deinitialize_for_client(&mut self) {
        *self.hmd_context.present_producer.lock() = None;
    }

    fn update_pose(
        object_id: Option<u32>,
        timer: &Instant,
        offset_time_ns: i64,
        motion: &MotionDesc,
        driver_pose: &mut vr::DriverPose_t,
    ) {
        if let Some(id) = object_id {
            let p = motion.pose.position;
            let o = motion.pose.orientation;
            let v = motion.linear_velocity;
            let av = motion.angular_velocity;
            driver_pose.vecPosition = [p[0] as _, p[1] as _, p[2] as _];
            driver_pose.vecVelocity = [v[0] as _, v[1] as _, v[2] as _];
            driver_pose.qRotation = vr::HmdQuaternion_t {
                w: o[0] as _,
                x: o[1] as _,
                y: o[2] as _,
                z: o[3] as _,
            };
            driver_pose.vecAngularVelocity = [av[0] as _, av[1] as _, av[2] as _];
            // todo: check if sign needs to be flipped
            driver_pose.poseTimeOffset =
                (timer.elapsed().as_nanos() as i64 - offset_time_ns) as f64 / 1_000_000_f64;

            unsafe { vr::server_driver_host_tracked_device_pose_updated(id, driver_pose) };
        }
    }

    pub fn update_input(&mut self, client_update: &ClientUpdate) {
        let pose_time_ns = client_update.motion_data.time_ns as i64;

        let server_time_ns = self.pose_timer.elapsed().as_nanos() as i64;
        let pose_time_offset_ns =
            server_time_ns + self.additional_pose_time_offset_ns - pose_time_ns;
        if pose_time_offset_ns < 0 {
            self.additional_pose_time_offset_ns = -(server_time_ns - pose_time_ns);
        } else if pose_time_offset_ns > RESET_POSE_TIMING_THRESHOLD_NS {
            self.additional_pose_time_offset_ns =
                -(server_time_ns + RESET_POSE_TIMING_THRESHOLD_NS - pose_time_ns);
        }

        let offset_time_ns = pose_time_ns - self.additional_pose_time_offset_ns;

        Self::update_pose(
            *self.hmd_context.id.lock(),
            &self.pose_timer,
            offset_time_ns,
            &client_update.motion_data.hmd,
            &mut self.hmd_context.pose.lock(),
        );

        for (i, motion) in client_update.motion_data.controllers.iter().enumerate() {
            let context = &self.controller_contexts[i];
            Self::update_pose(
                *context.id.lock(),
                &self.pose_timer,
                offset_time_ns,
                motion,
                &mut context.pose.lock(),
            );
        }

        for ctx in &self.controller_contexts {
            let component_map = ctx.controller_input_to_component_map.lock();
            for (path, value) in input_device_data_to_str_value(&client_update.input_device_data) {
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
    }

    pub fn server_native(&self) -> Arc<vr::ServerTrackedDeviceProvider<ServerContext>> {
        self.server.clone()
    }
}
