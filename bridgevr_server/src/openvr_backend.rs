use bridgevr_common::{packets::*, settings::*};
use log::warn;
use openvr_driver as vr;
use std::sync::*;

const DEVICE_ID_FACTOR: usize = 256;

// The "contexts" are the structs given to the openvr callbacks and are internally mutable.
// Using internal mutability enables me to use the contexts concurrently as long as only one
// function uses a certain field at a time.

struct HmdContext {
    object_id: Mutex<Option<u32>>,
    settings: Arc<Settings>,
    target_eye_width: u32,
    target_eye_height: u32,
    fov: [Fov; 2],
    pose: Mutex<vr::DriverPose_t>,
    present_callback: Mutex<Box<dyn FnMut(u64) + Send>>,
    wait_for_present_callback: Mutex<Box<dyn FnMut() + Send>>,
}

struct ControllerContext {
    object_id: Mutex<Option<u32>>,
    pose: Mutex<vr::DriverPose_t>,
}

pub struct OpenvrClient {
    // store hmd to keep it alive, otherwise OpenVR runtime would use an invalid pointer.
    _hmd: vr::TrackedDeviceServerDriver<HmdContext>,
    hmd_context: Arc<HmdContext>,
    controllers: Vec<(
        vr::TrackedDeviceServerDriver<ControllerContext>,
        Arc<ControllerContext>,
    )>,
}

impl OpenvrClient {
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
        device_id: usize,
        settings: Arc<Settings>,
        target_eye_width: u32,
        target_eye_height: u32,
        handshake_packet: ClientHandshakePacket,
        present_callback: impl FnMut(u64) + Send + 'static,
        wait_for_present_callback: impl FnMut() + Send + 'static,
    ) -> Self {
        let hmd_context = Arc::new(HmdContext {
            settings,
            present_callback: Mutex::new(Box::new(present_callback)),
            wait_for_present_callback: Mutex::new(Box::new(wait_for_present_callback)),
            target_eye_width,
            target_eye_height,
            fov: handshake_packet.fov.clone(),
            object_id: Mutex::new(None),
            pose: <_>::default(),
        });

        let openvr_display_component_callbacks = vr::DisplayComponentCallbacks {
            context: hmd_context.clone(),
            get_window_bounds: |context, x, y, width, height| {
                *x = 0;
                *y = 0;
                *width = context.target_eye_width * 2;
                *height = context.target_eye_height;
            },
            is_display_on_desktop: |_| false,
            is_display_real_display: |_| false,
            get_recommended_render_target_size: |context, width, height| {
                *width = context.target_eye_width * 2;
                *height = context.target_eye_height;
            },
            get_eye_output_viewport: |context, eye, x, y, width, height| {
                *x = context.target_eye_width * (eye as u32);
                *y = 0;
                *width = context.target_eye_width;
                *height = context.target_eye_height;
            },
            get_projection_raw: |context, eye, left, right, top, bottom| {
                let eye = eye as usize;
                *left = context.fov[eye].left;
                *right = context.fov[eye].right;
                *top = context.fov[eye].top;
                *bottom = context.fov[eye].bottom;
            },
            compute_distortion: |_, _, u, v| vr::DistortionCoordinates_t {
                rfRed: [u, v],
                rfGreen: [u, v],
                rfBlue: [u, v],
            },
        };
        let openvr_display_component =
            unsafe { vr::DisplayComponent::new(openvr_display_component_callbacks) };

        let openvr_virtual_display_callbacks = vr::VirtualDisplayCallbacks {
            context: hmd_context.clone(),
            present: |context, present_info| {
                (&mut *context.present_callback.lock().unwrap())(
                    present_info.backbufferTextureHandle,
                );
            },
            // assumption: `wait_for_present` is called immediately after `present`
            // todo: verify
            wait_for_present: |context| {
                (&mut *context.wait_for_present_callback.lock().unwrap())();
            },
            get_time_since_last_vsync: |context, seconds_since_last_vsync, frame_counter| {
                // *seconds_since_last_vsync =
                //     (Instant::now() - *context.last_vsync_time.lock().unwrap()).as_secs() as _;
                // *frame_counter = *context.vsync_counter.lock().unwrap();
                true
            },
        };
        let openvr_virtual_display =
            unsafe { vr::VirtualDisplay::new(openvr_virtual_display_callbacks) };

        let openvr_hmd_callbacks = vr::TrackedDeviceServerDriverCallbacks {
            context: hmd_context.clone(),
            activate: |context, object_id| {
                *context.object_id.lock().unwrap() = Some(object_id);
                let container =
                    unsafe { vr::properties_tracked_device_to_property_container(object_id) };

                //todo: set common props

                OpenvrClient::set_custom_props(
                    container,
                    &context.settings.openvr.hmd_custom_properties,
                );

                vr::VRInitError_None
            },
            deactivate: |_| (),
            enter_standby: |_| (),
            debug_request: |_, request| format!("debug request: {}", request),
            get_pose: |context| *context.pose.lock().unwrap(),
        };

        let mut openvr_hmd_components = vr::Components::none();
        openvr_hmd_components.display = Some(openvr_display_component);
        openvr_hmd_components.virtual_display = Some(openvr_virtual_display);

        let hmd = unsafe {
            vr::TrackedDeviceServerDriver::new(openvr_hmd_callbacks, openvr_hmd_components)
        };

        unsafe {
            vr::server_driver_host_tracked_device_added(
                &(DEVICE_ID_FACTOR * device_id).to_string(),
                vr::TrackedDeviceClass_HMD,
                &hmd,
            )
        };

        for input in handshake_packet.input_devices_initial_data {
            match input {
                InputDeviceData::OculusTouchPair { .. } => {}
                _ => unimplemented!(),
            }
        }

        // let openvr_controllers = (..2)
        //     .map(|i| {
        //         let openvr_controller_callbacks = vr::TrackedDeviceServerDriverCallbacks {
        //             context: client_context.clone(),
        //             activate: |context, object_id| {
        //                 let _container =
        //                     vr::properties_tracked_device_to_property_container(object_id);
        //                 // todo
        //                 vr::VRInitError_None
        //             },
        //             deactivate: |_| (),
        //             enter_standby: |_| (),
        //             debug_request: |_, request| format!("debug request: {}", request),
        //             get_pose: |context| context.controllers_pose[i],
        //         };
        //         unsafe {
        //             vr::TrackedDeviceServerDriver::new(
        //                 openvr_controller_callbacks,
        //                 vr::Components::none(),
        //             )
        //         }
        //     })
        //     .collect();

        //     for i in 0..openvr_controllers.len() {
        //         vr::server_driver_host_tracked_device_added(
        //             (i + 1).to_string(),
        //             vr::TrackedDeviceClass_Controller,
        //             client.openvr_controllers.get_mut().unwrap()[i],
        //         );
        //     }

        Self {
            _hmd: hmd,
            hmd_context,
            controllers: vec![],
        }
    }

    fn update_pose(
        &self,
        object_id: &Mutex<Option<u32>>,
        new_pose: &Pose,
        driver_pose: &Mutex<vr::DriverPose_t>,
    ) {
        if let Some(id) = *object_id.lock().unwrap() {
            let mut driver_pose = driver_pose.lock().unwrap();
            let p = new_pose;
            *driver_pose = vr::DriverPose_t {
                vecWorldFromDriverTranslation: [p.0[3] as _, p.0[7] as _, p.0[11] as _],
                ..<_>::default() // todo: remove and implement manually
            };

            unsafe { vr::server_driver_host_tracked_device_pose_updated(id, &driver_pose) };
        }
    }

    pub fn update_input(
        &mut self,
        hmd_pose: &Pose,
        controller_poses: &[Pose],
        /*todo: digital input*/
    ) {
        self.update_pose(
            &self.hmd_context.object_id,
            hmd_pose,
            &self.hmd_context.pose,
        );

        for (i, pose) in controller_poses.iter().enumerate() {
            let controller_context = &self.controllers[i].1;
            self.update_pose(
                &controller_context.object_id,
                pose,
                &controller_context.pose,
            );
        }
    }
}

pub struct ServerContext {
    shutdown_callback: Mutex<Box<dyn FnMut() + Send>>,
}

pub struct OpenvrServer {
    context: Arc<ServerContext>,
    server: vr::ServerTrackedDeviceProvider<ServerContext>,
}

impl OpenvrServer {
    pub fn new(shutdown_callback: impl FnMut() + Send + 'static) -> Self {
        let context = Arc::new(ServerContext {
            shutdown_callback: Mutex::new(Box::new(shutdown_callback)),
        });

        let openvr_server_callbacks = vr::ServerTrackedDeviceProviderCallbacks {
            context: context.clone(),
            init: |_, driver_context| {
                unsafe { vr::init_server_driver_context(driver_context) };
                vr::VRInitError_None
            },
            cleanup: |context| unsafe {
                (&mut *context.shutdown_callback.lock().unwrap())();

                //todo: send vendor specific event {0,0} (shutdown)

                vr::cleanup_driver_context();
            },
            run_frame: |_| (),
            should_block_standby_mode: |_| false,
            enter_standby: |_| (),
            leave_standby: |_| (),
        };

        let server = unsafe { vr::ServerTrackedDeviceProvider::new(openvr_server_callbacks) };

        OpenvrServer { context, server }
    }

    pub fn shutdown(&mut self) {
        (&mut *self.context.shutdown_callback.lock().unwrap())()
    }

    pub fn to_native(&self) -> &vr::ServerTrackedDeviceProvider<ServerContext> {
        &self.server
    }
}
