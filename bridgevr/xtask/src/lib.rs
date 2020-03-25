use fs_extra::*;
use std::{env, fmt::Display, fs, path::*, process::*};

#[cfg(target_os = "linux")]
const DRIVER_SRC_FNAME: &str = "libbridgevr_server_driver.so";
#[cfg(windows)]
const DRIVER_SRC_FNAME: &str = "bridgevr_server_driver.dll";

#[cfg(target_os = "linux")]
const DRIVER_REL_DIR_STR: &str = "bin/linux64";
#[cfg(windows)]
const DRIVER_REL_DIR_STR: &str = "bin/win64";

#[cfg(target_os = "linux")]
const DRIVER_DST_FNAME: &str = "driver_bridgevr.so";
#[cfg(windows)]
const DRIVER_DST_FNAME: &str = "driver_bridgevr.dll";

#[cfg(target_os = "linux")]
fn steamvr_bin_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap()
        .join(".steam/steam/steamapps/common/SteamVR/bin/linux64")
}
#[cfg(windows)]
fn steamvr_bin_dir() -> PathBuf {
    PathBuf::from("C:/Program Files (x86)/Steam/steamapps/common/SteamVR/bin/win64")
}

#[cfg(target_os = "linux")]
fn exec_fname(name: &str) -> String {
    name.to_owned()
}
#[cfg(windows)]
fn exec_fname(name: &str) -> String {
    format!("{}.exe", name)
}

fn str_err<T, E: Display>(maybe_obj: Result<T, E>) -> Result<T, String> {
    maybe_obj.map_err(|e| format!("{}", e))
}

fn run(cmd: &str) -> Result<(), String> {
    let args = cmd
        .split_whitespace()
        .map(|it| it.to_string())
        .collect::<Vec<_>>();
    let output = str_err(
        str_err(
            Command::new(&args[0])
                .args(&args[1..])
                .stdout(Stdio::inherit())
                .spawn(),
        )?
        .wait_with_output(),
    )?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "\nCommand \"{}\" failed\n{}",
            cmd,
            str_err(String::from_utf8(output.stderr))?
        ))
    }
}

pub fn server_build_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    const OS_STR: &str = "linux";
    #[cfg(windows)]
    const OS_STR: &str = "windows";

    std::env::current_dir()
        .unwrap()
        .join(format!("build/bridgevr_server_{}", OS_STR))
}

pub fn reset_server_build_folder() -> Result<(), String> {
    let build_path = server_build_path();
    fs::remove_dir_all(&build_path).ok();

    str_err(fs::create_dir_all(&build_path))?;

    // get all file and folder paths at depth 1, excluded template root (at index 0)
    let dir_content = str_err(dir::get_dir_content2(
        "server_release_template",
        &dir::DirOptions { depth: 1 },
    ))?;
    let items = dir_content.directories[1..]
        .iter()
        .chain(dir_content.files.iter())
        .collect();

    str_err(copy_items(&items, build_path, &dir::CopyOptions::new()))?;

    Ok(())
}

pub fn build_server(release: bool, target_dir: &Path) -> Result<(), String> {
    let build_type_name = if release { "release" } else { "debug" };
    let build_flag = if release { "--release" } else { "" };

    run(&format!(
        "cargo build -p bridgevr_server_driver {}",
        build_flag
    ))?;
    run(&format!(
        "cargo build -p bridgevr_server_gui {}",
        build_flag
    ))?;

    let artifacts_dir = target_dir.join(build_type_name);
    let build_dir = server_build_path();
    let gui_fname = exec_fname("bridgevr_server_gui");
    let driver_dst_dir = build_dir.join(DRIVER_REL_DIR_STR);

    str_err(fs::create_dir_all(&driver_dst_dir))?;

    fs::copy(
        artifacts_dir.join(DRIVER_SRC_FNAME),
        driver_dst_dir.join(DRIVER_DST_FNAME),
    )
    .map_err(|e| e.to_string())?;

    fs::copy(artifacts_dir.join(&gui_fname), build_dir.join(gui_fname))
        .map_err(|e| e.to_string())?;

    // if cfg!(target_os = "linux") {
    //     use std::io::Write;

    //     let mut shortcut = str_err(
    //         fs::OpenOptions::new()
    //             .append(true)
    //             .open(release_dir.join("bridgevr.desktop")),
    //     )?;
    //     str_err(writeln!(
    //         shortcut,
    //         "Exec={}",
    //         gui_dst_path.to_string_lossy()
    //     ))?;
    // }

    Ok(())
}

pub fn build_client(release: bool, target_dir: &Path) -> Result<(), String> {
    todo!()
}

pub fn register_driver(server_path: &Path) -> Result<(), String> {
    let steamvr_bin_dir = steamvr_bin_dir();
    if cfg!(target_os = "linux") {
        env::set_var("LD_LIBRARY_PATH", &steamvr_bin_dir);
    }
    run(&format!(
        "{} adddriver {}",
        steamvr_bin_dir
            .join(exec_fname("vrpathreg"))
            .to_string_lossy(),
        server_path.to_string_lossy()
    ))
}

pub fn unregister_driver() {}

pub fn open_ports(ports: Vec<u16>) {}
