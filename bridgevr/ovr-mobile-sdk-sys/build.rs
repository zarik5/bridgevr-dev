use std::{env::var, path::PathBuf};

#[cfg(feature = "custom-version")]
fn sdk_path() -> PathBuf {
    PathBuf::from(var("OVR_MOBILE_SDK_DIR").expect("OVR_MOBILE_SDK_DIR"))
}

#[cfg(not(feature = "custom-version"))]
fn sdk_path() -> PathBuf {
    // todo: actually fetch and unzip sdk
    PathBuf::from(var("OVR_MOBILE_SDK_DIR").expect("OVR_MOBILE_SDK_DIR"))
}

fn main() {
    let out_path = PathBuf::from(var("OUT_DIR").unwrap());

    let ovr_path = sdk_path();
    let ovr_include_path_flag_string =
        format!("-I{}", ovr_path.join("VrApi/Include").to_string_lossy());

    bindgen::builder()
        .header("src/bindings.h")
        .clang_arg("-Isrc")
        .clang_arg(&ovr_include_path_flag_string)
        .clang_arg("--target=aarch64-linux-android")
        .layout_tests(false)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        .generate()
        .expect("bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("bindings.rs");
}
