use bridgevr_common::data::*;

fn main() {
    println!("cargo:rustc-env=BVR_SERVER_VERSION={}", BVR_SERVER_VERSION);
    println!(
        "cargo:rustc-env=SETTINGS_SCHEMA={}",
        serde_json::to_string(&settings_schema(settings_default())).unwrap()
    );
}
