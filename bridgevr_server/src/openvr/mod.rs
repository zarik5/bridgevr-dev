mod hmd;
mod settings;
mod tracked_device;

use crate::{compositor::*, shutdown_signal::ShutdownSignal};
use bridgevr_common::{
    data::*,
    input_mapping::*,
    rendering::*,
    sockets::*,
    thread_loop::{self, *},
    *,
};
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

        // unwrap is safe
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
                        let haptic_data = HapticData {
                            device_type: *device_type,
                            amplitude: haptic.fAmplitude,
                            duration_seconds: haptic.fDurationSeconds,
                            frequency: haptic.fFrequency,
                        };
                        haptic_enqueuer
                            .enqueue(&haptic_data)
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
    tracked_devices_contexts: Vec<(TrackedDeviceType, Arc<TrackedDeviceContext>)>,
    input_thread: Option<ThreadLoop>,
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

        VrServer {
            settings: openvr_settings,
            server,
            server_context,
            hmd_context: maybe_hmd_context,
            tracked_devices_contexts,
            input_thread: None,
        }
    }

    fn process_input(
        input: ClientInputs,
        tracked_device_contexts: &HashMap<TrackedDeviceType, Arc<TrackedDeviceContext>>,
        controllers_contexts: &[Arc<TrackedDeviceContext>],
        pose_timer: &mut Instant,
        additional_pose_time_offset_ns: &mut i64,
    ) {
        let pose_time_ns = input.motion_data.time_ns as i64;

        let server_time_ns = pose_timer.elapsed().as_nanos() as i64;
        let pose_time_offset_ns = server_time_ns + *additional_pose_time_offset_ns - pose_time_ns;
        if pose_time_offset_ns < 0 {
            *additional_pose_time_offset_ns = -(server_time_ns - pose_time_ns);
        } else if pose_time_offset_ns > RESET_POSE_TIMING_THRESHOLD_NS {
            *additional_pose_time_offset_ns =
                -(server_time_ns + RESET_POSE_TIMING_THRESHOLD_NS - pose_time_ns);
        }

        let offset_time_ns = pose_time_ns - *additional_pose_time_offset_ns;

        for (device_type, motion) in input.motion_data.motion_descs {
            if let Some(context) = tracked_device_contexts.get(&device_type) {
                let driver_pose = &mut *context.pose.lock();

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
                driver_pose.poseTimeOffset = (pose_timer.elapsed().as_nanos() as i64
                    - offset_time_ns) as f64
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

        // As an optimization, let only the controllers receive input data.
        let input = input_device_data_to_str_value(&input.input_device_data);
        for ctx in controllers_contexts {
            let component_map = ctx.input_to_component_map.lock();
            for (path, value) in &input {
                if let Some(component) = component_map.get(*path) {
                    let time_offset_s = (pose_timer.elapsed().as_nanos() as i64 - offset_time_ns)
                        as f64
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
        mut input_dequeuer: PacketDequeuer,
        haptic_enqueuer: PacketEnqueuer,
    ) -> StrResult {
        // the same openvr settings instance is shared between hmd, controllers and server.
        let new_settings = create_openvr_settings(Some(settings), session_desc);
        if should_restart(&*self.settings.lock(), &new_settings) {
            unsafe {
                let reason_c_string = trace_err!(CString::new(
                    "Critical properties changed. Restarting SteamVR."
                ))?;
                let executable_c_string = trace_err!(CString::new(
                    "" // todo: steamvr_launcher,
                ))?;
                let arguments_c_string = trace_err!(CString::new(
                    "" // steamvr_launcher_args,
                ))?;
                let working_directory_c_string = trace_err!(CString::new(
                    "" // todo: steamvr_launcher_directory,
                ))?;
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
            if let Some(hmd_context) = &self.hmd_context {
                *hmd_context.compositor_interop.lock() = Some(CompositorInterop {
                    present_sender,
                    present_done_notif_receiver,
                });
            }

            *self.server_context.haptic_enqueuer.lock() = Some(haptic_enqueuer);

            let mut pose_timer = Instant::now();
            let mut additional_pose_time_offset_ns = 0;
            let tracked_devices_contexts = self
                .tracked_devices_contexts
                .iter()
                .map(|(device_type, ctx)| (*device_type, ctx.clone()))
                .collect();
            let controllers_contexts = self
                .tracked_devices_contexts
                .iter()
                .map(|(_, ctx)| ctx.clone())
                .collect::<Vec<_>>();
            self.input_thread = Some(thread_loop::spawn("OpenVR input loop", move || {
                let maybe_packet = input_dequeuer.dequeue(TIMEOUT).map_err(|e| debug!("{}", e));
                if let Ok(packet) = maybe_packet {
                    let maybe_input = packet.get().map_err(|e| debug!("{}", e));
                    if let Ok(input) = maybe_input {
                        Self::process_input(
                            input,
                            &tracked_devices_contexts,
                            &controllers_contexts,
                            &mut pose_timer,
                            &mut additional_pose_time_offset_ns,
                        );
                    }
                }
            })?);

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
