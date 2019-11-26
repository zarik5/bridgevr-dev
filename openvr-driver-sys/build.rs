use std::fs::File;
use std::io::{Read, Write};

fn main() {
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // let header_str = "openvr_driver.h";
    // let modified_openvr_driver_header_path = out_path.join(header_str);
    let include_flag_string = format!("-I{}", out_path.to_string_lossy());

    // Inside "openvr_driver.h" substitute <string> and <vector> with a bindgen friendly "dummy_string.h", "dummy_vector.h"
    // I could't do this with the C++ preprocessor
    // let mut openvr_driver_header_content = String::new();
    // {
    //     File::open(header_str)
    //         .expect(header_str)
    //         .read_to_string(&mut openvr_driver_header_content)
    //         .expect(header_str);
    // }
    // // openvr_driver_header_content = openvr_driver_header_content
    // //     .replace("#include <string>", r#"#include "dummy_string.h""#)
    // //     .replace("#include <vector>", r#"#include "dummy_vector.h""#);
    // {
    //     File::create(modified_openvr_driver_header_path)
    //         .expect(header_str)
    //         .write(openvr_driver_header_content.as_bytes())
    //         .expect(header_str);
    // }

    if cfg!(windows) {
        cc::Build::new()
            .cpp(true)
            .file("src/bindings.cpp")
            .flag("-Isrc")
            .flag("-Iinclude")
            .flag(&include_flag_string)
            .compile("bindings");
    } else {
        cc::Build::new()
            .flag("-Wno-unused-parameter")
            .cpp(true)
            .file("src/bindings.cpp")
            .flag("-Isrc")
            .flag("-Iinclude")
            .flag(&include_flag_string)
            .compile("bindings");
    }

    bindgen::builder()
        .clang_arg("-xc++")
        .header("src/openvr_driver_capi.h")
        .clang_arg("-Isrc")
        .clang_arg("-Iinclude")
        .clang_arg(&include_flag_string)
        .layout_tests(false)
        .enable_cxx_namespaces()
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        // .rustified_enum("vr::ETrackedPropertyError")
        // .rustified_enum("vr::EHDCPError")
        // .rustified_enum("vr::EVRInputError")
        // .rustified_enum("vr::EVRSpatialAnchorError")
        // .rustified_enum("vr::EVRSettingsError")
        // .rustified_enum("vr::EIOBufferError")
        .generate_inline_functions(true)
        .blacklist_function("vr::.*")
        .blacklist_item("std")
        .blacklist_type("vr::IVRSettings")
        .blacklist_type("vr::CVRSettingHelper")
        .blacklist_type("vr::ITrackedDeviceServerDriver")
        .blacklist_type("vr::IVRDisplayComponent")
        .blacklist_type("vr::IVRDriverDirectModeComponent")
        .opaque_type("vr::ICameraVideoSinkCallback")
        .blacklist_type("vr::IVRCameraComponent")
        .opaque_type("vr::IVRDriverContext")
        .blacklist_type("vr::IServerTrackedDeviceProvider")
        .blacklist_type("vr::IVRWatchdogProvider")
        .blacklist_type("vr::IVRCompositorPluginProvider")
        .blacklist_type("vr::IVRProperties")
        .blacklist_type("vr::CVRPropertyHelpers")
        .blacklist_type("vr::IVRDriverInput")
        .blacklist_type("vr::IVRDriverLog")
        .blacklist_type("vr::IVRServerDriverHost")
        .blacklist_type("vr::IVRCompositorDriverHost")
        .blacklist_type("vr::CVRHiddenAreaHelpers")
        .blacklist_type("vr::IVRWatchdogHost")
        .blacklist_type("vr::IVRVirtualDisplay")
        .blacklist_type("vr::IVRResources")
        .blacklist_type("vr::IVRIOBuffer")
        .blacklist_type("vr::IVRDriverManager")
        .blacklist_type("vr::IVRDriverSpatialAnchors")
        .blacklist_type("vr::COpenVRDriverContext")
        .generate()
        .expect("bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("bindings.rs");
}
