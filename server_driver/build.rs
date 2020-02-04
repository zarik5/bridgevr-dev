use std::{env::var, path::PathBuf};

fn main() {
    let is_release = var("PROFILE").unwrap() == "release";

    // forward environment variables info to current crate
    let bvr_install_root = if is_release {
        PathBuf::from(var("INSTALL_ROOT").expect("Environment variable INSTALL_ROOT"))
    } else {
        dirs::home_dir().unwrap().join(".bridgevr")
    };

    println!(
        "cargo:rustc-env=LOG_PATH={}",
        bvr_install_root.join("log.txt").to_str().unwrap()
    );

    println!(
        "cargo:rustc-env=SETTINGS_PATH={}",
        bvr_install_root.join("settings.json").to_str().unwrap()
    );

    println!(
        "cargo:rustc-env=SESSION_PATH={}",
        bvr_install_root.join("session.json").to_str().unwrap()
    );
}
