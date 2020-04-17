use pico_args::Arguments;
use std::{env, path::Path};
use bridgevr_xtask::*;

fn ok_or_exit(res: Result<(), String>) {
    use std::process::exit;

    if let Err(e) = res {
        #[cfg(not(windows))]
        {
            use termion::color::*;
            println!("{}{}{}", Fg(Red), e, Fg(Reset));
        }
        #[cfg(windows)]
        println!("{}", e);

        exit(1);
    }
}

fn print_help() {
    println!(
        r#"
cargo xtask
Developement actions for BridgeVR.

USAGE:
    cargo xtask <SUBCOMMAND> [FLAG]
    cargo xtask --help

SUBCOMMANDS:
    install-deps        Install required cargo third-party subcommands
    release-server      Resets platform specific server build folder, then 'build-server'
    build-server        Build server driver and GUI, then copy binaries to build folder
    build-client        Build client apk and copy it to build folder
    build-all           Combines 'build-server' and 'build-client'
    open-ports          Open ports 9943, 9944
    register-driver     Register BridgeVR driver in SteamVR
    install-server      Combines 'build-server', 'open-ports' and 'register-driver'

FLAGS:
    --release           Build without debug info. Used only for build subcommands
"#
    );
}

fn main() {
    let target_dir = Path::new(env!("OUT_DIR")).join("../../../..");

    let mut args = Arguments::from_env();

    if args.contains(["-h", "--help"]) {
        print_help();
    } else if let Ok(Some(subcommand)) = args.subcommand() {
        let release = args.contains("--release");

        if args.finish().is_ok() {
            match subcommand.as_str() {
                "install-deps" => todo!(),
                "release-server" => {
                    ok_or_exit(reset_server_build_folder());
                    ok_or_exit(build_server(true, &target_dir));
                }
                "build-server" => ok_or_exit(build_server(release, &target_dir)),
                "build-client" => ok_or_exit(build_client(release, &target_dir)),
                "build-all" => {
                    ok_or_exit(build_server(release, &target_dir));
                    ok_or_exit(build_client(release, &target_dir));
                }
                "open-ports" => todo!(),
                "register-driver" => ok_or_exit(register_driver(&server_build_path())),
                "install-server" => todo!(),
                _ => {
                    println!("\nError parsing subcommand.");
                    print_help();
                    return;
                }
            }
        } else {
            println!("\nWrong arguments.");
            print_help();
            return;
        }
    } else {
        println!("\nError parsing subcommand.");
        print_help();
        return;
    }

    println!("\nDone\n");
}
