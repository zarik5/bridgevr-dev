use bridgevr_common::data::*;

fn main() {
    println!("cargo:rustc-env=BVR_SERVER_VERSION={}", BVR_SERVER_VERSION);
    println!("cargo:rustc-env=SETTINGS_SCHEMA={}", settings_schema(settings_default()));
}
