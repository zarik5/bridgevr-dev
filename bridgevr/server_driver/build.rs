use std::{env::var, path::PathBuf};
use bridgevr_xtask::server_build_path;

fn main() {
    let server_install_root = if let Ok(install_root_str) = var("INSTALL_ROOT") {
        PathBuf::from(install_root_str)
    } else {
        server_build_path()
    };

    println!(
        "cargo:rustc-env=INSTALL_ROOT={}",
        server_install_root.to_string_lossy()
    );
}
