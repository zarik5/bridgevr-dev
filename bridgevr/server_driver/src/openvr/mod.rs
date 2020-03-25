mod hmd;
mod settings;
mod tracked_device;

use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{data::*, input_mapping::*, graphics::*, sockets::*, *};
use hmd::*;
use log::*;
use openvr_driver_sys as vr;
use parking_lot::Mutex;
use settings::*;
use std::{
    collections::HashMap,
    ffi::*,
    mem::size_of,
    os::raw::*,
    ptr::null_mut,
    sync::{mpsc::*, Arc},
    time::*,
};
use tracked_device::*;

const RESET_POSE_TIMING_THRESHOLD_NS: i64 = 50_000_000;

const VIRTUAL_DISPLAY_MAX_TEXTURES: usize = 3;

const DEFAULT_COMPOSITOR_TYPE: CompositorType = CompositorType::Custom;

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

fn should_restart(old_settings: &OpenvrSettings, new_settings: &OpenvrSettings) -> bool {
    new_settings.fov != old_settings.fov
        || new_settings.frame_interval != old_settings.frame_interval
}

struct ServerContext {
    settings: Arc<Mutex<OpenvrSettings>>,
    tracked_devices_ptrs: Vec<(TrackedDeviceType, *mut vr::TrackedDeviceServerDriver)>,
    tracked_devices_contexts: Vec<(TrackedDeviceType, Arc<TrackedDeviceContext>)>,
    haptic_enqueuer: Mutex<Option<PacketEnqueuer>>,
    shutdown_signal_sender: Arc<Mutex<Sender<ShutdownSignal>>>,
}

extern "C" fn init(
    context: *mut c_void,
    driver_context: *mut vr::IVRDriverContext,
) -> vr::EVRInitError {
    let context = unsafe { &*(context as *mut ServerContext) };

    unsafe { vr::vrInitServerDriverContext(driver_context) };

    for (device_type, ptr) in &context.tracked_devices_ptrs {
        let openvr_tracked_device_type = match device_type {
            TrackedDeviceType::HMD => vr::TrackedDeviceClass_HMD,
            TrackedDeviceType::LeftController | TrackedDeviceType::RightController => {
                vr::TrackedDeviceClass_Controller
            }
            _ => vr::TrackedDeviceClass_GenericTracker,
        };

        // unwrap never fails
        let controller_id_c_string = CString::new((*device_type as u8).to_string()).unwrap();
        unsafe {
            vr::vrServerDriverHostTrackedDeviceAdded(
                controller_id_c_string.as_ptr(),
                openvr_tracked_device_type,
                *ptr,
            )
        };
    }

    vr::VRInitError_None
}

unsafe extern "C" fn cleanup(context: *mut c_void) {
    let context = context as *mut ServerContext;

    (*context)
        .shutdown_signal_sender
        .lock()
        .send(ShutdownSignal::BackendShutdown)
        .map_err(|e| debug!("{}", e))
        .ok();

    vr::vrCleanupDriverContext();
}

extern "C" fn get_interface_versions(_: *mut c_void) -> *const *const c_char {
    lazy_static::lazy_static! {
        static ref NATIVE_CLASSES_VERSIONS: Vec<usize> = vec![
            vr::IVRSettings_Version as *const _ as _,
            vr::ITrackedDeviceServerDriver_Version as *const _ as _,
            vr::IVRDisplayComponent_Version as *const _ as _,
            vr::IVRDriverDirectModeComponent_Version as *const _ as _,
            vr::IVRCameraComponent_Version as *const _ as _,
            vr::IServerTrackedDeviceProvider_Version as *const _ as _,
            vr::IVRWatchdogProvider_Version as *const _ as _,
            vr::IVRVirtualDisplay_Version as *const _ as _,
            vr::IVRDriverManager_Version as *const _ as _,
            vr::IVRResources_Version as *const _ as _,
            vr::IVRCompositorPluginProvider_Version as *const _ as _,
            0,
        ];
    }
    NATIVE_CLASSES_VERSIONS.as_ptr() as _
}

extern "C" fn run_frame(context: *mut c_void) {
    let context = unsafe { &*(context as *mut ServerContext) };

    if let Some(haptic_enqueuer) = &mut *context.haptic_enqueuer.lock() {
        loop {
            const EVENT_SIZE: u32 = size_of::<vr::VREvent_t>() as u32;
            let mut event = <_>::default();
            if !unsafe { vr::vrServerDriverHostPollNextEvent(&mut event, EVENT_SIZE) } {
                break;
            }

            if event.eventType == vr::VREvent_Input_HapticVibration as u32 {
                for (device_type, ctx) in &context.tracked_devices_contexts {
                    let haptic = unsafe { event.data.hapticVibration };
                    if haptic.componentHandle == *ctx.haptic_component.lock() {
                        let packet = OtherServerPacket::Haptic {
                            device_type: *device_type,
                            sample: HapticSample {
                                amplitude: haptic.fAmplitude,
                                duration_seconds: haptic.fDurationSeconds,
                                frequency: haptic.fFrequency,
                            },
                        };
                        haptic_enqueuer
                            .enqueue(&packet)
                            .map_err(|e| debug!("{}", e))
                            .ok();
                    }
                }
            }
        }
    }
}

extern "C" fn should_block_standby_mode(context: *mut c_void) -> bool {
    let context = context as *mut ServerContext;

    unsafe { (*context).settings.lock().block_standby }
}

extern "C" fn empty_fn(_: *mut c_void) {}

fn create_server_callbacks(
    server_context: Arc<ServerContext>,
) -> vr::ServerTrackedDeviceProviderCallbacks {
    vr::ServerTrackedDeviceProviderCallbacks {
        context: &*server_context as *const _ as _,
        Init: Some(init),
        Cleanup: Some(cleanup),
        GetInterfaceVersions: Some(get_interface_versions),
        RunFrame: Some(run_frame),
        ShouldBlockStandbyMode: Some(should_block_standby_mode),
        EnterStandby: Some(empty_fn),
        LeaveStandby: Some(empty_fn),
    }
}

pub struct VrServer {
    settings: Arc<Mutex<OpenvrSettings>>,
    server: *mut vr::ServerTrackedDeviceProvider,
    server_context: Arc<ServerContext>,
    hmd_context: Option<Arc<HmdContext>>,
    tracked_devices_contexts: HashMap<TrackedDeviceType, Arc<TrackedDeviceContext>>,
    // input_thread: Option<ThreadLoop>,
    input_timer: Instant,
    controllers_contexts: Vec<Arc<TrackedDeviceContext>>,
}

unsafe impl Send for VrServer {}
unsafe impl Sync for VrServer {}

impl VrServer {
    pub fn new(
        graphics: Arc<GraphicsContext>,
        settings: Option<&Settings>,
        session_desc: &SessionDesc,
        shutdown_signal_sender: Sender<ShutdownSignal>,
    ) -> Self {
        let openvr_settings = Arc::new(Mutex::new(create_openvr_settings(settings, &session_desc)));
        let shutdown_signal_sender = Arc::new(Mutex::new(shutdown_signal_sender));

        let tracked_devices_contexts = openvr_settings
            .lock()
            .tracked_devices
            .iter()
            .map(|td| {
                (
                    td.device_type,
                    Arc::new(TrackedDeviceContext {
                        device_type: td.device_type,
                        object_id: Mutex::new(None),
                        settings: openvr_settings.clone(),
                        pose: Mutex::new(DEFAULT_DRIVER_POSE),
                        input_to_component_map: Mutex::new(HashMap::new()),
                        haptic_component: Mutex::new(vr::k_ulInvalidInputComponentHandle),
                        shutdown_signal_sender: shutdown_signal_sender.clone(),
                    }),
                )
            })
            .collect::<Vec<_>>();

        let mut maybe_hmd_context = None;
        let mut tracked_devices_ptrs = vec![];
        for (device_type, ctx) in &tracked_devices_contexts {
            if let TrackedDeviceType::HMD = device_type {
                let swap_texture_manager = Mutex::new(SwapTextureManager::new(
                    graphics.clone(),
                    VIRTUAL_DISPLAY_MAX_TEXTURES,
                ));

                let hmd_context = Arc::new(HmdContext {
                    tracked_device_context: ctx.clone(),
                    display_component_ptr: Mutex::new(null_mut()),
                    virtual_display_ptr: Mutex::new(null_mut()),
                    driver_direct_mode_component_ptr: Mutex::new(null_mut()),
                    graphics: graphics.clone(),
                    swap_texture_manager,
                    current_layers: Mutex::new(vec![]),
                    sync_texture: Mutex::new(None),
                    compositor_interop: Mutex::new(None),
                    latest_vsync: Mutex::new((Instant::now(), 0)),
                });

                let display_callbacks = create_display_callbacks(hmd_context.clone());
                *hmd_context.display_component_ptr.lock() =
                    unsafe { vr::vrCreateDisplayComponent(display_callbacks) };

                let compositor_type = if let Some(settings) = settings {
                    settings.openvr.compositor_type
                } else {
                    DEFAULT_COMPOSITOR_TYPE
                };

                match compositor_type {
                    CompositorType::SteamVR => {
                        let virtual_display_callbacks =
                            create_virtual_display_callbacks(hmd_context.clone());
                        *hmd_context.virtual_display_ptr.lock() =
                            unsafe { vr::vrCreateVirtualDisplay(virtual_display_callbacks) };
                    }
                    CompositorType::Custom => {
                        let driver_direct_mode_callbacks =
                            create_driver_direct_mode_callbacks(hmd_context.clone());
                        *hmd_context.driver_direct_mode_component_ptr.lock() = unsafe {
                            vr::vrCreateDriverDirectModeComponent(driver_direct_mode_callbacks)
                        };
                    }
                }

                let hmd_callbacks = create_hmd_callbacks(hmd_context.clone());
                let hmd_ptr = unsafe { vr::vrCreateTrackedDeviceServerDriver(hmd_callbacks) };
                tracked_devices_ptrs.push((*device_type, hmd_ptr));
                maybe_hmd_context = Some(hmd_context);
            } else {
                let tracked_device_callbacks = create_tracked_device_callbacks(ctx.clone());
                let tracked_device_ptr =
                    unsafe { vr::vrCreateTrackedDeviceServerDriver(tracked_device_callbacks) };
                tracked_devices_ptrs.push((*device_type, tracked_device_ptr));
            }
        }

        let server_context = Arc::new(ServerContext {
            settings: openvr_settings.clone(),
            tracked_devices_ptrs,
            tracked_devices_contexts: tracked_devices_contexts.clone(),
            haptic_enqueuer: Mutex::new(None),
            shutdown_signal_sender,
        });

        let server_callbacks = create_server_callbacks(server_context.clone());

        let server = unsafe { vr::vrCreateServerTrackedDeviceProvider(server_callbacks) };

        let controllers_contexts = tracked_devices_contexts
            .iter()
            .map(|(_, ctx)| ctx.clone())
            .collect::<Vec<_>>();
        VrServer {
            settings: openvr_settings,
            server,
            server_context,
            hmd_context: maybe_hmd_context,
            tracked_devices_contexts: tracked_devices_contexts.into_iter().collect(),
            input_timer: Instant::now(),
            controllers_contexts,
        }
    }

    pub fn process_motion(
        &mut self,
        device_type: TrackedDeviceType,
        sample: MotionSample6DofDesc,
        timestamp_ns: u64,
    ) {
        let pose_timestamp_ns = timestamp_ns as i64;

        let server_elapsed_ns = self.input_timer.elapsed().as_nanos() as i64;
        let pose_time_offset_ns = server_elapsed_ns - pose_timestamp_ns;
        if pose_time_offset_ns < 0 {
            self.input_timer -= Duration::from_nanos(pose_time_offset_ns as _);
        } else if pose_time_offset_ns > RESET_POSE_TIMING_THRESHOLD_NS {
            self.input_timer -= Duration::from_nanos(
                (server_elapsed_ns + RESET_POSE_TIMING_THRESHOLD_NS - pose_timestamp_ns) as _,
            );
        }

        if let Some(context) = self.tracked_devices_contexts.get(&device_type) {
            let driver_pose = &mut *context.pose.lock();

            let p = sample.pose.position;
            let o = sample.pose.orientation;
            let v = sample.linear_velocity;
            let av = sample.angular_velocity;
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
            driver_pose.poseTimeOffset = (self.input_timer.elapsed().as_nanos() as i64
                - pose_timestamp_ns) as f64
                / 1_000_000_f64;

            if let Some(object_id) = *context.object_id.lock() {
                unsafe {
                    vr::vrServerDriverHostTrackedDevicePoseUpdated(
                        object_id,
                        driver_pose,
                        size_of::<vr::DriverPose_t>() as _,
                    )
                };
            }
        }
    }

    pub fn update_virtual_vsync(&mut self, virtual_vsync_offset_ns: i32) {
        if let Some(hmd_context) = &self.hmd_context {
            let (vsync, _) = &mut *hmd_context.latest_vsync.lock();

            // workaround for forbidden negative Duration
            let abs_offset = Duration::from_nanos(virtual_vsync_offset_ns.abs() as _);
            if virtual_vsync_offset_ns > 0 {
                *vsync += abs_offset;
            } else {
                *vsync -= abs_offset;
            }
        }
    }

    pub fn process_input(&self, data: InputDeviceData, timestamp_ns: u64) {
        let input_timestamp_ns = timestamp_ns as i64;
        let input = input_device_data_to_str_value_map(&data);

        for ctx in &self.controllers_contexts {
            let component_map = ctx.input_to_component_map.lock();
            for (path, value) in &input {
                if let Some(component) = component_map.get(*path) {
                    let time_offset_s = (self.input_timer.elapsed().as_nanos() as i64
                        - input_timestamp_ns) as f64
                        / 1_000_000_f64;
                    unsafe {
                        match value {
                            InputValue::Boolean(value) => {
                                // todo: update only if necessary!!!

                                vr::vrDriverInputUpdateBooleanComponent(
                                    *component,
                                    *value,
                                    time_offset_s,
                                );
                            }
                            InputValue::NormalizedOneSided(value)
                            | InputValue::NormalizedTwoSided(value) => {
                                vr::vrDriverInputUpdateScalarComponent(
                                    *component,
                                    *value,
                                    time_offset_s,
                                );
                            }
                            _ => todo!(),
                        }
                    }
                }
            }
        }
    }

    pub fn initialize_for_client_or_request_restart(
        &mut self,
        settings: &Settings,
        session_desc: &SessionDesc,
        present_sender: Sender<PresentData>,
        present_done_notif_receiver: Receiver<()>,
        haptic_enqueuer: PacketEnqueuer,
    ) -> StrResult {
        // the same openvr settings instance is shared between hmd, controllers and server.
        let new_settings = create_openvr_settings(Some(settings), session_desc);
        if should_restart(&*self.settings.lock(), &new_settings) {
            // unwraps never fail
            unsafe {
                let reason_c_string =
                    CString::new("Critical properties changed. Restarting SteamVR.").unwrap();
                let executable_c_string = CString::new(
                    "" // todo: steamvr_launcher,
                ).unwrap();
                let arguments_c_string = CString::new(
                    "" // steamvr_launcher_args,
                ).unwrap();
                let working_directory_c_string = CString::new(
                    "" // todo: steamvr_launcher_directory,
                ).unwrap();
                vr::vrServerDriverHostRequestRestart(
                    reason_c_string.as_ptr(),
                    executable_c_string.as_ptr(),
                    arguments_c_string.as_ptr(),
                    working_directory_c_string.as_ptr(),
                );
                // shutdown signal will be generated from SteamVR
            }
        } else {
            *self.settings.lock() = new_settings;
            *self.server_context.haptic_enqueuer.lock() = Some(haptic_enqueuer);
            if let Some(hmd_context) = &self.hmd_context {
                *hmd_context.compositor_interop.lock() = Some(CompositorInterop {
                    present_sender,
                    present_done_notif_receiver,
                });
            }

            // todo: notify settings changes to openvr using properties
        }
        Ok(())
    }

    pub fn deinitialize_for_client(&mut self) {
        if let Some(hmd_context) = &self.hmd_context {
            *hmd_context.compositor_interop.lock() = None;
        }
        *self.server_context.haptic_enqueuer.lock() = None;
    }

    pub fn server_ptr(&self) -> *mut vr::ServerTrackedDeviceProvider {
        self.server
    }
}

impl Drop for VrServer {
    fn drop(&mut self) {
        if let Some(hmd_context) = &self.hmd_context {
            let mut display_component_ptr = *hmd_context.display_component_ptr.lock();
            unsafe { vr::vrDestroyDisplayComponent(&mut display_component_ptr) };

            let mut virtual_display_ptr = *hmd_context.virtual_display_ptr.lock();
            if !virtual_display_ptr.is_null() {
                unsafe { vr::vrDestroyVirtualDisplay(&mut virtual_display_ptr) };
            }

            let mut driver_direct_mode_component_ptr =
                *hmd_context.driver_direct_mode_component_ptr.lock();
            if !driver_direct_mode_component_ptr.is_null() {
                unsafe {
                    vr::vrDestroyDriverDirectModeComponent(&mut driver_direct_mode_component_ptr)
                };
            }
        }

        for (_, mut ptr) in &self.server_context.tracked_devices_ptrs {
            unsafe { vr::vrDestroyTrackedDeviceServerDriver(&mut ptr) };
        }

        //todo: destroy server?
        // unsafe { vr::vrDestroyServerTrackedDeviceProvider(&mut self.server) };
    }
}
