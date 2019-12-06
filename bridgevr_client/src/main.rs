mod compositor;
mod input;
mod logging_backend;
mod openxr_backend;
mod video_decoder;

use bridgevr_common::*;
// use openxr as xr;

fn main() {
    logging_backend::init_logging(); //todo when connected to server reinitialize logging to send log to server

    // todo: statically link openxr loader
    // let entry = ok_or_panic!(
    //     xr::Entry::load_from(std::path::Path::new("openxr_loader-1_0.dll")),
    //     "OpenXR loader"
    // );

    // let supported_extensions = entry.enumerate_extensions().unwrap();

    // if !supported_extensions.khr_convert_timespec_time {
    //     log_panic!("timespec conversion unsupported");
    // }

    // // todo: add android and oculus extensions

    // let required_extensions = xr::ExtensionSet {
    //     khr_vulkan_enable: true,
    //     ..<_>::default()
    // };

    // let instance = ok_or_panic!(
    //     entry.create_instance(
    //         &xr::ApplicationInfo {
    //             application_name: constants::BVR_NAME,
    //             application_version: constants::BVR_VERSION_CLIENT,
    //             ..<_>::default()
    //         },
    //         &required_extensions,
    //     ),
    //     "OpenXR instance"
    // );

    // //let instance_props = instance.properties().unwrap();

    // let system = instance
    //     .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
    //     .unwrap();

    // let system_properties = instance.system_properties(system).unwrap();

    // let device_id = system_properties.system_id.into_raw();
    // let device_name = system_properties.system_name;

    // let view_config_views = instance
    //     .enumerate_view_configuration_views(system, xr::ViewConfigurationType::PRIMARY_STEREO)
    //     .unwrap();
}
