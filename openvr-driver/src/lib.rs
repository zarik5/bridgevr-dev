#![allow(non_snake_case, clippy::missing_safety_doc)]

// export structures
pub use openvr_driver_sys::root::vr::*;

use openvr_driver_sys::root as private;
use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::os::raw::*;
use std::sync::*;

const MAX_USER_STRING_SIZE: usize = k_unMaxPropertyStringSize as usize;

pub struct Components<T> {
    pub display: Option<DisplayComponent<T>>,
    pub driver_direct_mode: Option<DriverDirectModeComponent<T>>,
    pub camera: Option<CameraComponent<T>>,
    pub virtual_display: Option<VirtualDisplay<T>>,
}

impl<T> Components<T> {
    pub fn none() -> Self {
        Self {
            display: None,
            driver_direct_mode: None,
            camera: None,
            virtual_display: None,
        }
    }
}

macro_rules! to_native_type {
        (NativeComponent) => (*mut c_void);
        (&str) => (*const c_char);
        (&mut $t:ty) => (*mut $t);
        (&$t:ty) => (*const $t);
        ($t:ty) => ($t);
    }

macro_rules! cast_from_native {
    ($e:expr, &str) => {
        CStr::from_ptr($e).to_str().unwrap()
    };
    ($e:expr, &mut $t:ty) => {
        &mut *$e as _
    };
    ($e:expr, &$t:ty) => {
        &*$e as _
    };
    ($e:expr, $($t:ty)?) => {
        $e as _
    };
}

macro_rules! cast_to_native {
    ($e:expr, NativeComponent) => {
        $e.0
    };
    ($e:expr, &str) => {
        $e.as_ptr() as _
    };
    ($e:expr, $($t:ty)?) => {
        $e as _
    };
}

macro_rules! extern_callbacks_with_components {
    (
        $callbacks_t:ty;
        $(fn $rust_fn_name:ident ( $($params:ident: $params_t:ty),* ) $(-> $return_t:ty)? ;)*
    ) => {
        paste::item!{
            $(unsafe extern "C" fn [<_ $rust_fn_name>](
                native_context: *mut c_void,
                $($params: to_native_type!($params_t)),*
            ) $(-> to_native_type!($return_t))? {
                let callbacks = &((*(native_context as *mut ($callbacks_t, Components<T>))).0);
                cast_to_native!(
                    (callbacks.$rust_fn_name)(
                        callbacks.context.clone(),
                        $(cast_from_native!($params, $params_t)),*),
                    $($return_t)?
                )
            })*
        }
    }
}

macro_rules! create_native_class_bindings {
    (
        $struct_name:ident;
        $(
            $($native_callback_name:ident => fn $rust_callback:ident(
                $($params:ident : $params_t:ty),* $(,)?
            ) $(-> $return_t:ty)?)?
            $(
                [custom] $custom_native_callback_name:ident => $custom_native_callback_item:item
                $(=> fn $custom_rust_callback:ident (
                    $($custom_params:ident : $custom_params_t:ty),* $(,)?
                ) $(-> $custom_return_t:ty)?)?
            )?
        ;)*
    ) => {
        paste::item! {

            pub struct [<$struct_name Callbacks>]<T> {
                pub context: Arc<T>,
                $(
                    $(pub $rust_callback: fn(Arc<T>, $($params_t),*) $(-> $return_t)? ,)?
                    $($(pub $custom_rust_callback: fn(
                        $($custom_params_t),*
                    ) $(-> $custom_return_t)? ,)?)?
                )*
            }

            pub struct $struct_name<T> {
                native_class: *mut private::$struct_name,

                _callbacks: [<$struct_name Callbacks>]<T>,
                _native_callbacks: private::[<$struct_name Callbacks>],
            }

            unsafe impl<T: Send> Send for $struct_name<T> {}

            unsafe impl<T: Sync> Sync for $struct_name<T> {}

            impl<T> $struct_name<T> {
                $(
                    $(unsafe extern "C" fn [<_ $rust_callback>](
                        native_context: *mut c_void,
                        $($params: to_native_type!($params_t)),*
                    ) $(-> to_native_type!($return_t))? {
                        let callbacks = &*(native_context as *mut [<$struct_name Callbacks>]<T>);
                        cast_to_native!(
                            (callbacks.$rust_callback)(
                                callbacks.context.clone(),
                                $(cast_from_native!($params, $params_t)),*
                            ),
                            $($return_t)?
                        )
                    })?

                    $($custom_native_callback_item)?
                )*

                pub unsafe fn new(callbacks: [<$struct_name Callbacks>]<T>) -> Self {
                    let mut native_callbacks = private::[<$struct_name Callbacks>] {
                        context: &callbacks as *const _ as _,
                        $(
                            $($native_callback_name: Some(Self::[<_ $rust_callback>]),)?
                            $($custom_native_callback_name:
                                Some(Self::[<_ $custom_native_callback_name>]),)?
                        )*
                    };

                    let native_class = private::[<vrCreate $struct_name>](&mut native_callbacks);

                    Self {
                        _callbacks: callbacks,
                        native_class,
                        _native_callbacks: native_callbacks,
                    }
                }

                pub unsafe fn to_raw(&self) -> *mut c_void {
                    self.native_class as _
                }
            }

            impl<T> Drop for $struct_name<T> {
                fn drop(&mut self) {
                    unsafe {
                        private::[<vrDestroy $struct_name>](&mut self.native_class);
                    }
                }
            }
        }
    };
}

pub struct TrackedDeviceServerDriverCallbacks<T> {
    pub context: Arc<T>,
    pub activate: fn(Arc<T>, TrackedDeviceIndex_t) -> EVRInitError,
    pub deactivate: fn(Arc<T>),
    pub enter_standby: fn(Arc<T>),
    pub debug_request: fn(Arc<T>, &str) -> String,
    pub get_pose: fn(Arc<T>) -> DriverPose_t,
}

pub struct TrackedDeviceServerDriver<T> {
    native_class: *mut private::TrackedDeviceServerDriver,

    _callbacks: TrackedDeviceServerDriverCallbacks<T>,
    _native_callbacks: private::TrackedDeviceServerDriverCallbacks,
    _components: Components<T>,
}

unsafe impl<T: Send> Send for TrackedDeviceServerDriver<T> {}

unsafe impl<T: Sync> Sync for TrackedDeviceServerDriver<T> {}

impl<T> TrackedDeviceServerDriver<T> {
    extern_callbacks_with_components! {
        TrackedDeviceServerDriverCallbacks<T>;
        fn activate(object_id: TrackedDeviceIndex_t) -> EVRInitError;
        fn deactivate();
        fn enter_standby();
        fn get_pose() -> DriverPose_t;
    }

    unsafe extern "C" fn _get_component(
        native_context: *mut c_void,
        pch_component_name_and_version: *const c_char,
    ) -> *mut c_void {
        let components =
            &((*(native_context as *mut (TrackedDeviceServerDriverCallbacks<T>, Components<T>))).1);
        let name_and_version_str = cast_from_native!(pch_component_name_and_version, &str);

        let compare = |str1: &str, str2: &[u8]| {
            str1 == CStr::from_bytes_with_nul(str2).unwrap().to_str().unwrap()
        };

        if compare(name_and_version_str, IVRDisplayComponent_Version) {
            components.display.as_ref().map(|c| c.to_raw())
        } else if compare(name_and_version_str, IVRDriverDirectModeComponent_Version) {
            components.driver_direct_mode.as_ref().map(|c| c.to_raw())
        } else if compare(name_and_version_str, IVRCameraComponent_Version) {
            components.camera.as_ref().map(|c| c.to_raw())
        } else if compare(name_and_version_str, IVRVirtualDisplay_Version) {
            components.virtual_display.as_ref().map(|c| c.to_raw())
        } else {
            None
        }
        .unwrap_or(std::ptr::null_mut())
    }

    unsafe extern "C" fn _debug_request(
        native_context: *mut c_void,
        pch_request: *const c_char,
        pch_response_buffer: *mut c_char,
        un_response_buffer_size: u32,
    ) {
        let callbacks = &((*(native_context
            as *mut (&TrackedDeviceServerDriverCallbacks<T>, &Components<T>)))
            .0);
        let response = (callbacks.debug_request)(
            callbacks.context.clone(),
            cast_from_native!(pch_request, &str),
        );
        let string_wrapper = CString::new(response).unwrap();
        std::ptr::copy_nonoverlapping(
            string_wrapper.as_ptr(),
            pch_response_buffer,
            std::cmp::min(
                un_response_buffer_size as usize,
                string_wrapper.as_bytes_with_nul().len(),
            ),
        );
    }

    pub unsafe fn new(
        callbacks: TrackedDeviceServerDriverCallbacks<T>,
        components: Components<T>,
    ) -> Self {
        let mut native_callbacks = private::TrackedDeviceServerDriverCallbacks {
            context: &(&callbacks, &components) as *const _ as _,
            Activate: Some(Self::_activate),
            Deactivate: Some(Self::_deactivate),
            EnterStandby: Some(Self::_enter_standby),
            GetComponent: Some(Self::_get_component),
            DebugRequest: Some(Self::_debug_request),
            GetPose: Some(Self::_get_pose),
        };

        let native_class = private::vrCreateTrackedDeviceServerDriver(&mut native_callbacks);

        Self {
            _callbacks: callbacks,
            native_class,
            _native_callbacks: native_callbacks,
            _components: components,
        }
    }
}

impl<T> Drop for TrackedDeviceServerDriver<T> {
    fn drop(&mut self) {
        unsafe {
            private::vrDestroyTrackedDeviceServerDriver(&mut self.native_class);
        }
    }
}

create_native_class_bindings! {
    DisplayComponent;
    GetWindowBounds => fn get_window_bounds(
        x: &mut i32,
        y: &mut i32,
        width: &mut u32,
        height: &mut u32
    );
    IsDisplayOnDesktop => fn is_display_on_desktop() -> bool;
    IsDisplayRealDisplay => fn is_display_real_display() -> bool;
    GetRecommendedRenderTargetSize => fn get_recommended_render_target_size(
        width: &mut u32,
        height: &mut u32
    );
    GetEyeOutputViewport => fn get_eye_output_viewport(
        eye: EVREye,
        x: &mut u32,
        y: &mut u32,
        width: &mut u32,
        height: &mut u32
    );
    GetProjectionRaw => fn get_projection_raw(
        eye: EVREye,
        left: &mut f32,
        right: &mut f32,
        top: &mut f32,
        bottom: &mut f32
    );
    ComputeDistortion => fn compute_distortion(
        eye: EVREye,
        u: f32,
        v: f32
    ) -> DistortionCoordinates_t;
}

create_native_class_bindings! {
    DriverDirectModeComponent;
    CreateSwapTextureSet => fn create_swap_texture_set(
        pid: u32,
        swap_texture_set_desc: &IVRDriverDirectModeComponent_SwapTextureSetDesc_t,
        shared_texture_handles: &mut [SharedTextureHandle_t; 3]
    );
    DestroySwapTextureSet => fn destroy_swap_texture_set(
        shared_texture_handle: SharedTextureHandle_t
    );
    DestroyAllSwapTextureSets => fn destroy_all_swap_texture_sets(pid: u32);
    GetNextSwapTextureSetIndex => fn get_next_swap_texture_set_index(
        shared_texture_handles: &mut [SharedTextureHandle_t; 2],
        indices: &mut [u32; 2]
    );
    SubmitLayer => fn submit_layer(
        per_eye: &mut [IVRDriverDirectModeComponent_SubmitLayerPerEye_t; 2],
        pose: &HmdMatrix34_t
    );
    Present => fn present(sync_texture: SharedTextureHandle_t);
    PostPresent => fn post_present();
    GetFrameTiming => fn get_frame_timing(frame_timing: &mut DriverDirectMode_FrameTiming);
}

create_native_class_bindings! {
    CameraVideoSinkCallback;
    OnCameraVideoSinkCallback => fn on_camera_video_sink_callback();
}

create_native_class_bindings! {
    CameraComponent;
    GetCameraFrameDimensions => fn get_camera_frame_dimensions(
        video_stream_format: ECameraVideoStreamFormat,
        width: &mut u32,
        height: &mut u32
    ) -> bool;
    GetCameraFrameBufferingRequirements => fn get_camera_frame_buffering_requirements(
        default_frame_queue_size: &mut i32,
        frame_buffer_data_size: &mut u32
    ) -> bool;
    SetCameraFrameBuffering => fn set_camera_frame_buffering(
        frame_buffer_count: i32,
        frame_buffers: &mut *mut c_void,
        frame_buffer_data_size: u32
    ) -> bool;
    SetCameraVideoStreamFormat => fn set_camera_video_stream_format(
        video_stream_format: ECameraVideoStreamFormat
    ) -> bool;
    GetCameraVideoStreamFormat => fn get_camera_video_stream_format() -> ECameraVideoStreamFormat;
    StartVideoStream => fn start_video_stream() -> bool;
    StopVideoStream => fn stop_video_stream();
    IsVideoStreamActive => fn is_video_stream_active(
        paused: &mut bool,
        elapsed_time: &mut f32
    ) -> bool;
    GetVideoStreamFrame => fn get_video_stream_frame() -> *const CameraVideoStreamFrame_t;
    ReleaseVideoStreamFrame => fn release_video_stream_frame(frame_image: &CameraVideoStreamFrame_t);
    SetAutoExposure => fn set_auto_exposure(enable: bool) -> bool;
    PauseVideoStream => fn pause_video_stream() -> bool;
    ResumeVideoStream => fn resume_video_stream() -> bool;
    GetCameraDistortion => fn get_camera_distortion(
        camera_index: u32,
        input_u: f32,
        input_v: f32,
        output_u: &mut f32,
        output_v: &mut f32
    ) -> bool;
    GetCameraProjection => fn get_camera_projection(
        camera_index: u32,
        frame_type: EVRTrackedCameraFrameType,
        z_near: f32,
        z_far: f32,
        projection: &mut HmdMatrix44_t
    ) -> bool;
    SetFrameRate => fn set_frame_rate(isp_frame_rate: i32, sensor_frame_rate: i32) -> bool;
    SetCameraVideoSinkCallback => fn set_camera_video_sink_callback(
        camera_video_sink_callback: *mut ICameraVideoSinkCallback
    ) -> bool;
    GetCameraCompatibilityMode => fn get_camera_compatibility_mode(
        camera_compatibility_mode: &mut ECameraCompatibilityMode
    ) -> bool;
    SetCameraCompatibilityMode => fn set_camera_compatibility_mode(
        camera_compatibility_mode: ECameraCompatibilityMode
    ) -> bool;
    GetCameraFrameBounds => fn get_camera_frame_bounds(
        frame_type: EVRTrackedCameraFrameType,
        left: &mut u32,
        top: &mut u32,
        width: &mut u32,
        height: &mut u32
    ) -> bool;
    GetCameraIntrinsics => fn get_camera_intrinsics(
        camera_index: u32,
        frame_type: EVRTrackedCameraFrameType,
        focal_length: &mut HmdVector2_t,
        center: &mut HmdVector2_t,
        distortion_type: &mut EVRDistortionFunctionType,
        coefficients: *mut f64
    ) -> bool;
}

lazy_static::lazy_static! {
    static ref NATIVE_CLASSES_VERSIONS: Vec<usize> = vec![
        IVRSettings_Version as *const _ as _,
        ITrackedDeviceServerDriver_Version as *const _ as _,
        IVRDisplayComponent_Version as *const _ as _,
        IVRDriverDirectModeComponent_Version as *const _ as _,
        IVRCameraComponent_Version as *const _ as _,
        IServerTrackedDeviceProvider_Version as *const _ as _,
        IVRWatchdogProvider_Version as *const _ as _,
        IVRVirtualDisplay_Version as *const _ as _,
        IVRDriverManager_Version as *const _ as _,
        IVRResources_Version as *const _ as _,
        IVRCompositorPluginProvider_Version as *const _ as _,
        0,
    ];
}

fn get_k_interface_versions() -> *const *const c_char {
    NATIVE_CLASSES_VERSIONS.as_ptr() as _
}

create_native_class_bindings!(
    ServerTrackedDeviceProvider;
    Init => fn init(driver_context: &mut IVRDriverContext) -> EVRInitError;
    Cleanup => fn cleanup();
    [custom] GetInterfaceVersions => extern "C" fn _GetInterfaceVersions(
        _: *mut c_void
    ) -> *const *const c_char {
        get_k_interface_versions()
    };
    RunFrame => fn run_frame();
    ShouldBlockStandbyMode => fn should_block_standby_mode() -> bool;
    EnterStandby => fn enter_standby();
    LeaveStandby => fn leave_standby();
);

create_native_class_bindings! {
    WatchdogProvider;
    Init => fn init(driver_context: &mut IVRDriverContext) -> EVRInitError;
    Cleanup => fn cleanup();
}

create_native_class_bindings! {
    CompositorPluginProvider;
    Init => fn init(driver_context: &mut IVRDriverContext) -> EVRInitError;
    Cleanup => fn cleanup();
    [custom] GetInterfaceVersions => extern "C" fn _GetInterfaceVersions(
        _: *mut c_void
    ) -> *const *const c_char {
        get_k_interface_versions()
    };
    GetComponent => fn get_component(component_name_and_version: &str) -> *mut c_void;
}

create_native_class_bindings! {
    VirtualDisplay;
    [custom] Present => unsafe extern "C" fn _Present(
        native_context: *mut c_void,
        present_info: *const PresentInfo_t,
        _: u32
    ) {
        let callbacks = &*(native_context as *mut VirtualDisplayCallbacks<T>);
        (callbacks.present)(
            callbacks.context.clone(),
            cast_from_native!(present_info, &PresentInfo_t)
        );
    } => fn present(context: Arc<T>, present_info: &PresentInfo_t);
    WaitForPresent => fn wait_for_present();
    GetTimeSinceLastVsync => fn get_time_since_last_vsync(
        seconds_since_last_vsync: &mut f32,
        frame_counter: &mut u64
    ) -> bool;
}

// I need to use paste::item! otherwise cast_to_native! pattern matching will fail.
macro_rules! forward_fns {
    ($(
        $($native_fn_name:ident => fn $fn_name:ident (
            $($fn_param:ident : $fn_param_t:ty),* $(,)?
        ) $(-> $fn_ret_t:ty)?)?

        $([err] $ret_err_native_fn_name:ident => fn $ret_err_fn_name:ident (
            $($ret_err_fn_param:ident : $ret_err_fn_param_t:ty),* $(,)?
        ) -> $ret_err_fn_err_t:ty)?

        $([res] $ret_res_native_fn_name:ident => fn $ret_res_fn_name:ident (
            $($ret_res_fn_param:ident : $ret_res_fn_param_t:ty),* $(,)?
        ) -> Result<$ret_res_fn_ret_t:ty, $ret_res_fn_err_t:ty>)?

        $([arg res] $arg_res_native_fn_name:ident => fn $arg_res_fn_name:ident (
            $($arg_res_fn_pre_param:ident : $arg_res_fn_pre_param_t:ty,)*
            [out]
            $(, $arg_res_fn_post_param:ident : $arg_res_fn_post_param_t:ty)* $(,)?
        ) -> Result<$arg_res_fn_ret_t:ty, $arg_res_fn_err_t:ty>)?

        // $([res string] fn $res_string_fn_name:ident (
        //       $($res_string_fn_pre_param:ident : $res_string_fn_pre_param_t:ty,)*
        //       [out]
        //       $(, $res_string_fn_post_param:ident : $res_string_fn_post_param_t:ty)* $(,)?
        // ) -> Result<String, $res_string_fn_err_t:ty>)?

        $([custom] $custom_fn:item)?
    ,)*) => {
        $(
            paste::item! {
                $(pub unsafe fn $fn_name($($fn_param: $fn_param_t),*) $(-> $fn_ret_t)? {
                    cast_from_native!(
                        private::$native_fn_name($(cast_to_native!($fn_param, $fn_param_t)),*),
                        $($fn_ret_t)?
                    )
                })?

                $(pub unsafe fn $ret_err_fn_name(
                    $($ret_err_fn_param: $ret_err_fn_param_t),*
                ) -> $ret_err_fn_err_t {
                    let mut err = 0;
                    private::$ret_err_native_fn_name(
                        $(cast_to_native!($ret_err_fn_param, $ret_err_fn_param_t),)*
                        &mut err
                    );
                    err
                })?

                $(pub unsafe fn $ret_res_fn_name(
                    $($ret_res_fn_param: $ret_res_fn_param_t),*
                ) -> Result<$ret_res_fn_ret_t, $ret_res_fn_err_t> {
                    let mut err = 0;
                    let res = private::$ret_res_native_fn_name(
                        $(cast_to_native!($ret_res_fn_param, $ret_res_fn_param_t),)*
                        &mut err
                    );
                    if err == 0 { Ok(res) } else { Err(err) }
                })?

                $(pub unsafe fn $arg_res_fn_name(
                    $($arg_res_fn_pre_param: $arg_res_fn_pre_param_t,)*
                    $($arg_res_fn_post_param: $arg_res_fn_post_param_t),*
                ) -> Result<$arg_res_fn_ret_t, $arg_res_fn_err_t> {
                    let mut res = <_>::default();
                    let err = private::$arg_res_native_fn_name(
                        $(cast_to_native!($arg_res_fn_pre_param, $arg_res_fn_pre_param_t),)*
                        &mut res
                        $(, cast_to_native!($arg_res_fn_post_param, $arg_res_fn_post_param_t))*
                    );
                    if err == 0 { Ok(res) } else { Err(err) }
                })?

                // $(pub unsafe fn $res_string_fn_name(
                //     $($res_string_fn_pre_param: $res_string_fn_pre_param_t,)*
                //     $($res_string_fn_post_param: $res_string_fn_post_param_t),*
                // ) -> Result<String, $res_string_fn_err_t> {
                // })?

                $(pub $custom_fn)?
            }
        )*
    };
}

forward_fns! {
    [custom] unsafe fn settings_get_error_name_from_enum<'a>(error: EVRSettingsError) -> &'a str {
        CStr::from_ptr(private::vrSettingsGetSettingsErrorNameFromEnum(error))
            .to_str()
            .unwrap()
    },
    [err] vrSettingsSetBool => fn settings_set_bool(
        section: &str,
        settings_key: &str,
        value: bool
    ) -> EVRSettingsError,
    [err] vrSettingsSetInt32 => fn settings_set_i32(
        section: &str,
        settings_key: &str,
        value: i32
    ) -> EVRSettingsError,
    [err] vrSettingsSetFloat => fn settings_set_f32(
        section: &str,
        settings_key: &str,
        value: f32
    ) -> EVRSettingsError,
    [err] vrSettingsSetString => fn settings_set_str(
        section: &str,
        settings_key: &str,
        value: &str
    ) -> EVRSettingsError,
    [res] vrSettingsGetBool => fn settings_get_bool(
        section: &str,
        settings_key: &str
    ) -> Result<bool, EVRSettingsError>,
    [res] vrSettingsGetInt32 => fn settings_get_i32(
        section: &str,
        settings_key: &str
    ) -> Result<i32, EVRSettingsError>,
    [res] vrSettingsGetFloat => fn settings_get_f32(
        section: &str,
        settings_key: &str
    ) -> Result<f32, EVRSettingsError>,
    [custom] unsafe fn settings_get_string(
        section: &str,
        settings_key: &str
    ) -> Result<String, ETrackedPropertyError> {
        const BUF_SIZE: usize = MAX_USER_STRING_SIZE as usize;
        static mut BUFFER: [u8; BUF_SIZE] = [0; BUF_SIZE];
        let mut error = 0;
        private::vrSettingsGetString(
            cast_to_native!(section, &str),
            cast_to_native!(settings_key, &str),
            BUFFER.as_mut_ptr() as _,
            MAX_USER_STRING_SIZE as _,
            &mut error,
        );
        match error {
            crate::TrackedProp_Success => Ok(CStr::from_bytes_with_nul(&BUFFER)
                .unwrap()
                .to_owned()
                .into_string()
                .unwrap()),
            e => Err(e),
        }
    },
    [err] vrSettingsRemoveSection => fn settings_remove_section(section: &str) -> EVRSettingsError,
    [err] vrSettingsRemoveKeyInSection => fn settings_remove_key_in_section(
        section: &str,
        settings_key: &str
    ) -> EVRSettingsError,

    [custom] unsafe fn properties_get_error_name_from_enum<'a>(
        error: ETrackedPropertyError
    ) -> &'a str {
        CStr::from_ptr(private::vrGetPropErrorNameFromEnum(error))
            .to_str()
            .unwrap()
    },
    vrTrackedDeviceToPropertyContainer => fn properties_tracked_device_to_property_container(
        device: TrackedDeviceIndex_t
    ) -> PropertyContainerHandle_t,

    [res] vrGetBoolProperty => fn properties_get_bool(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<bool, ETrackedPropertyError>,
    [res] vrGetFloatProperty => fn properties_get_f32(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<f32, ETrackedPropertyError>,
    [res] vrGetInt32Property => fn properties_get_i32(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<i32, ETrackedPropertyError>,
    [res] vrGetUint64Property => fn properties_get_u64(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<u64, ETrackedPropertyError>,
    [res] vrGetVec2Property => fn properties_get_hmd_vec2(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<HmdVector2_t, ETrackedPropertyError>,
    [res] vrGetVec3Property => fn properties_get_hmd_vec3(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<HmdVector3_t, ETrackedPropertyError>,
    [res] vrGetVec4Property => fn properties_get_hmd_vec4(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<HmdVector4_t, ETrackedPropertyError>,
    [custom] unsafe fn properties_get_string(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
    ) -> Result<String, ETrackedPropertyError> {
        static mut BUFFER: [u8; MAX_USER_STRING_SIZE] = [0; MAX_USER_STRING_SIZE];
        let mut error = 0;
        private::vrGetStringProperty(
            container_handle,
            prop,
            BUFFER.as_mut_ptr() as _,
            MAX_USER_STRING_SIZE as _,
            &mut error,
        );
        match error {
            crate::TrackedProp_Success => Ok(CStr::from_bytes_with_nul(&BUFFER)
                .unwrap()
                .to_owned()
                .into_string()
                .unwrap()),
            e => Err(e),
        }
    },
    vrGetProperty => fn get_property(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        buffer: *mut c_void,
        buffer_size: u32,
        tag: &mut PropertyTypeTag_t,
        error: &mut ETrackedPropertyError
    ) -> u32,
    vrSetBoolProperty => fn properties_set_bool(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: bool
    ) -> ETrackedPropertyError,
    vrSetFloatProperty => fn properties_set_f32(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: f32
    ) -> ETrackedPropertyError,
    vrSetInt32Property => fn properties_set_i32(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: i32
    ) -> ETrackedPropertyError,
    vrSetUint64Property => fn properties_set_u64(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: u64
    ) -> ETrackedPropertyError,
    vrSetVec2Property => fn properties_set_hmd_vec2(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: &HmdVector2_t
    ) -> ETrackedPropertyError,
    vrSetVec3Property => fn properties_set_hmd_vec3(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: &HmdVector3_t
    ) -> ETrackedPropertyError,
    vrSetVec4Property => fn properties_set_hmd_vec4(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: &HmdVector4_t
    ) -> ETrackedPropertyError,
    vrSetStringProperty => fn properties_set_str(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: &str
    ) -> ETrackedPropertyError,
    vrSetProperty => fn set_property(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        new_value: *mut c_void,
        new_value_size: u32,
        tag: PropertyTypeTag_t
    ) -> ETrackedPropertyError,
    vrSetPropertyError => fn properties_set_error(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty,
        error: ETrackedPropertyError
    ) -> ETrackedPropertyError,
    vrEraseProperty => fn erase_property(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> ETrackedPropertyError,
    [res] vrIsPropertySet => fn properties_is_set(
        container_handle: PropertyContainerHandle_t,
        prop: ETrackedDeviceProperty
    ) -> Result<bool, ETrackedPropertyError>,

    vrSetHiddenArea => fn set_hidden_area(
        eye: EVREye,
        hidden_area_mesh_type: EHiddenAreaMeshType,
        verts: *mut HmdVector2_t,
        vert_count: u32
    ) -> ETrackedPropertyError,
    vrGetHiddenArea => fn get_hidden_area(
        eye: EVREye,
        hidden_area_mesh_type: EHiddenAreaMeshType,
        verts: *mut HmdVector2_t,
        vert_count: u32,
        error: *mut ETrackedPropertyError
    ) -> u32,

    vrDriverLog => fn log(log_message: &str),

    [custom] unsafe fn server_driver_host_tracked_device_added<T>(
        device_serial_number: &str,
        device_class: ETrackedDeviceClass,
        tracked_device_server_driver: &TrackedDeviceServerDriver<T>
    ) -> bool {
        private::vrServerDriverHostTrackedDeviceAdded(
            cast_to_native!(device_serial_number, &str),
            device_class,
            tracked_device_server_driver.native_class,
        )
    },
    //todo: check if rust struct has the same size as c struct
    [custom] unsafe fn server_driver_host_tracked_device_pose_updated(
        which_device: u32,
        new_pose: &DriverPose_t,
    ) {
        private::vrServerDriverHostTrackedDevicePoseUpdated(
            which_device,
            new_pose,
            size_of::<DriverPose_t>() as _,
        );
    },
    vrServerDriverHostVsyncEvent => fn server_driver_host_vsync_event(
        vsync_time_offset_seconds: f64
    ),
    vrServerDriverHostVendorSpecificEvent => fn server_driver_host_vendor_specific_event(
        which_device: u32,
        event_type: EVREventType,
        event_data: &VREvent_Data_t,
        event_time_offset: f64
    ),
    vrServerDriverHostIsExiting => fn server_driver_host_is_exiting() -> bool,
    [custom] unsafe fn server_driver_host_poll_next_event() -> Option<VREvent_t> {
        let event_size = size_of::<VREvent_t>() as u32;
        let mut event = <_>::default();
        if private::vrServerDriverHostPollNextEvent(&mut event, event_size) {
            Some(event)
        } else {
            None
        }
    },
    vrServerDriverHostGetRawTrackedDevicePoses => fn server_driver_host_get_raw_tracked_device_poses(
        predicted_seconds_from_now: f32,
        tracked_device_pose_array: *mut TrackedDevicePose_t,
        tracked_device_pose_array_count: u32
    ),
    vrServerDriverHostTrackedDeviceDisplayTransformUpdated =>
        fn server_driver_host_tracked_device_display_transform_updated(
            which_device: u32, eye_to_head_left: HmdMatrix34_t, eye_to_head_right: HmdMatrix34_t
        ),
    vrServerDriverHostRequestRestart => fn server_driver_host_request_restart(
        localized_reason: &str,
        executable_to_start: &str,
        arguments: &str,
        working_directory: &str
    ),
    [custom] unsafe fn server_driver_host_get_frame_timings(
        frames_count: usize
    ) -> Vec<Compositor_FrameTiming> {
        let mut timing_vec = vec![Compositor_FrameTiming {
                m_nSize: size_of::<Compositor_FrameTiming>() as _,
                ..<_>::default()
            }; frames_count];

        let filled_count = private::vrServerDriverHostGetFrameTimings(
            timing_vec.as_mut_ptr(),
            frames_count as u32
        );
        Vec::from(&timing_vec[0..filled_count as _])
    },

    vrWatchdogWakeUp => fn watchdog_wake_up(device_class: ETrackedDeviceClass),

    vrCompositorDriverHostPollNextEvent => fn compositor_driver_host_poll_next_event(
        event: *mut VREvent_t,
        size_bytes: u32
    ) -> bool,

    vrDriverHandle => fn driver_handle() -> DriverHandle_t,

    vrDriverManagerGetDriverCount => fn driver_manager_get_driver_count() -> u32,
    [custom] unsafe fn driver_manager_get_driver_name(driver: DriverId_t) -> String {
        static mut BUFFER: [u8; MAX_USER_STRING_SIZE] = [0; MAX_USER_STRING_SIZE];
        private::vrDriverManagerGetDriverName(
            driver,
            BUFFER.as_mut_ptr() as _,
            MAX_USER_STRING_SIZE as _,
        );
        CStr::from_bytes_with_nul(&BUFFER)
            .unwrap()
            .to_owned()
            .into_string()
            .unwrap()
    },
    vrDriverManagerGetDriverHandle => fn driver_manager_get_driver_handle(
        driver_name: &str
    ) -> DriverHandle_t,
    vrDriverManagerIsEnabled => fn driver_manager_is_enabled(driver: DriverId_t) -> bool,

    vrLoadSharedResource => fn load_shared_resource(
        resource_name: &str,
        buffer: *mut c_char,
        buffer_len: u32
    ) -> u32,
    vrGetResourceFullPath => fn get_resource_full_path(
        resource_name: &str,
        resource_type_directory: &str,
        path_buffer: *mut c_char,
        buffer_len: u32
    ) -> u32,

    [arg res] vrDriverInputCreateBooleanComponent => fn driver_input_create_boolean(
        container: PropertyContainerHandle_t,
        name: &str,
        [out]
    ) -> Result<VRInputComponentHandle_t, EVRInputError>,
    vrDriverInputUpdateBooleanComponent => fn driver_input_update_boolean(
        component: VRInputComponentHandle_t,
        new_value: bool,
        time_offset: f64
    ) -> EVRInputError,
    [arg res] vrDriverInputCreateScalarComponent => fn driver_input_create_scalar(
        container: PropertyContainerHandle_t,
        name: &str,
        [out],
        scalar_type: EVRScalarType,
        units: EVRScalarUnits
    ) -> Result<VRInputComponentHandle_t, EVRInputError>,
    vrDriverInputUpdateScalarComponent => fn driver_input_update_scalar(
        component: VRInputComponentHandle_t,
        new_value: f32,
        time_offset: f64
    ) -> EVRInputError,
    [arg res] vrDriverInputCreateHapticComponent => fn driver_input_create_haptic(
        container: PropertyContainerHandle_t,
        name: &str,
        [out]
    ) -> Result<VRInputComponentHandle_t, EVRInputError>,
    [arg res] vrDriverInputCreateSkeletonComponent => fn driver_input_create_skeleton(
        container: PropertyContainerHandle_t,
        name: &str,
        skeleton_path: &str,
        base_pose_path: &str,
        skeletal_tracking_level: EVRSkeletalTrackingLevel,
        grip_limit_transform: &VRBoneTransform_t,
        grip_limit_transform_count: u32,
        [out]
    ) -> Result<VRInputComponentHandle_t, EVRInputError>,
    vrDriverInputUpdateSkeletonComponent => fn driver_input_update_skeleton(
        component: VRInputComponentHandle_t,
        motion_range: EVRSkeletalMotionRange,
        transforms: *const VRBoneTransform_t,
        transform_count: u32
    ) -> EVRInputError,

    [arg res] vrIOBufferOpen => fn io_buffer_open(
        path: &str,
        mode: EIOBufferMode,
        element_size: u32,
        elements: u32,
        [out]
    ) -> Result<IOBufferHandle_t, EIOBufferError>,
    vrIOBufferClose => fn io_buffer_close(buffer: IOBufferHandle_t) -> EIOBufferError,
    vrIOBufferRead => fn io_buffer_read(
        buffer: IOBufferHandle_t,
        mode: *mut c_void, bytes: u32,
        read: &mut u32
    ) -> EIOBufferError,
    vrIOBufferWrite => fn io_buffer_write(
        buffer: IOBufferHandle_t,
        src: *mut c_void, bytes: u32
    ) -> EIOBufferError,
    vrIOBufferPropertyContainer => fn io_buffer_property_container(
        buffer: IOBufferHandle_t
    ) -> PropertyContainerHandle_t,
    vrIOBufferHasReaders => fn io_buffer_has_readers(buffer: IOBufferHandle_t) -> bool,

    vrDriverSpatialAnchorsUpdateSpatialAnchorPose => fn driver_spatial_anchors_update_pose(
        handle: SpatialAnchorHandle_t,
        pose: &SpatialAnchorDriverPose_t
    ) -> EVRSpatialAnchorError,
    vrDriverSpatialAnchorsSetSpatialAnchorPoseError => fn driver_spatial_anchors_set_pose_error(
        handle: SpatialAnchorHandle_t,
        error: EVRSpatialAnchorError,
        valid_duration: f64
    ) -> EVRSpatialAnchorError,
    vrDriverSpatialAnchorsUpdateSpatialAnchorDescriptor =>
        fn driver_spatial_anchors_update_descriptor(
            handle: SpatialAnchorHandle_t, descriptor: &str
        ) -> EVRSpatialAnchorError,
    [arg res] vrDriverSpatialAnchorsGetSpatialAnchorPose => fn driver_spatial_anchors_get_pose(
        handle: SpatialAnchorHandle_t,
        [out]
    ) -> Result<SpatialAnchorDriverPose_t, EVRSpatialAnchorError>,
    [custom] unsafe fn driver_spatial_anchors_get_descriptor(
        handle: SpatialAnchorHandle_t,
        decorated: bool,
    ) -> Result<String, EVRSpatialAnchorError> {
        static mut BUFFER: [u8; MAX_USER_STRING_SIZE] = [0; MAX_USER_STRING_SIZE];
        static mut _BUFFER_SIZE: u32 = MAX_USER_STRING_SIZE as u32;
        let error = private::vrDriverSpatialAnchorsGetSpatialAnchorDescriptor(
            handle,
            BUFFER.as_mut_ptr() as _,
            &mut _BUFFER_SIZE as _,
            decorated,
        );
        match error {
            crate::VRSpatialAnchorError_Success => Ok(CStr::from_bytes_with_nul(&BUFFER)
                .unwrap()
                .to_owned()
                .into_string()
                .unwrap()),
            e => Err(e),
        }
    },

    vrInitServerDriverContext => fn init_server_driver_context(
        context: &mut IVRDriverContext
    ) -> EVRInitError,
    vrInitWatchdogDriverContext => fn init_watchdog_driver_context(
        context: &mut IVRDriverContext
    ) -> EVRInitError,
    vrInitCompositorDriverContext => fn init_compositor_driver_context(
        context: &mut IVRDriverContext
    ) -> EVRInitError,
    vrCleanupDriverContext => fn cleanup_driver_context(),
}

#[doc(hidden)]
#[macro_export]
macro_rules! _create_entry_point {
    ($create_native_class_fn:ident, $t:ty, $server:expr, $driver_interface_version:ident) => {
        /// # Safety
        ///
        #[no_mangle]
        pub unsafe extern "C" fn HmdDriverFactory(
            interface_name: *const ::std::os::raw::c_char,
            return_code_ptr: *mut ::std::os::raw::c_int,
        ) -> *mut c_void {
            let mut maybe_server: ::std::result::Result<Arc<ServerTrackedDeviceProvider<_>>, _> =
                $server;
            match maybe_server {
                Ok(server) => {
                    let mut native_class_ptr = if ::std::ffi::CStr::from_ptr(interface_name)
                        .to_str()
                        .unwrap()
                        == ::std::ffi::CStr::from_bytes_with_nul($crate::$driver_interface_version)
                            .unwrap()
                            .to_str()
                            .unwrap()
                    {
                        server.to_raw()
                    } else {
                        ::std::ptr::null_mut()
                    };

                    if native_class_ptr.is_null() && !return_code_ptr.is_null() {
                        *return_code_ptr = $crate::VRInitError_Init_InterfaceNotFound as _;
                    }

                    native_class_ptr
                }
                Err(_) => ::std::ptr::null_mut(),
            }
        }
    };
}

// Create the extern function HmdDriverFactory which instantiates and returns a server native class
#[macro_export(local_inner_macros)]
macro_rules! openvr_server_entry_point {
    ($locked_server:expr) => {
        _create_entry_point!(
            create_native_server_tracked_device_provider,
            ServerTrackedDeviceProvider,
            $locked_server,
            IServerTrackedDeviceProvider_Version
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_k_interface_versions() {
        unsafe {
            let ptr_ptr = get_k_interface_versions();
            println!("{}", CStr::from_ptr(*ptr_ptr.offset(10)).to_str().unwrap());
        }
    }
}
