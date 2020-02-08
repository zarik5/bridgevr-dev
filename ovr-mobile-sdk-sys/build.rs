use std::{env::var, path::PathBuf};

fn main() {
    let out_path = PathBuf::from(var("OUT_DIR").unwrap());

    let ovr_path = PathBuf::from(var("OVR_MOBILE_SDK_DIR").expect("OVR_MOBILE_SDK_DIR"));
    let ovr_include_path_flag_string =
        format!("-I{}", ovr_path.join("VrApi/Include").to_string_lossy());

    bindgen::builder()
        .header("src/bindings.h")
        .clang_arg("-Isrc")
        .clang_arg(&ovr_include_path_flag_string)
        .layout_tests(false)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        .generate()
        .expect("bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("bindings.rs");
}
