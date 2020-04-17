fn main() {
    println!(
        "cargo:rustc-env=SERVER_VERSION={}",
        bridgevr_xtask::server_driver_version()
    );

    println!(
        "cargo:rustc-env=CLIENT_VERSION={}",
        bridgevr_xtask::client_version()
    );
}
