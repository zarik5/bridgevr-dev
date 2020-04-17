use log::*;
use std::{path::Path, sync::Once};

static INIT_LOGGING_ENTRY_POINT: Once = Once::new();

#[cfg(target_os = "linux")]
fn show_error_message_box(_: &str, message_with_intro: &str) {
    use gtk::*;

    // init() must be called on the same thread as MessageDialog::new()
    if gtk::init().is_ok() {
        MessageDialog::new(
            None::<&Window>,
            DialogFlags::empty(),
            MessageType::Error,
            ButtonsType::Close,
            &message_with_intro,
        )
        .run();
    } else {
        error!("Failed to initialize GTK. Exit");
    }
}

#[cfg(not(target_os = "linux"))]
fn show_error_message_box(title: &str, message_with_intro: &str) {
    msgbox::create(title, &message_with_intro, msgbox::IconType::Error);
}

pub fn init_logging() {
    // SteamVR keeps calling HmdDriverFactory until a vaild driver is found. If BridgeVR fails to
    // startup, init_logging will be called a second time on the same process. To ensure that
    // logging initialization happens only once, use an Once object.
    INIT_LOGGING_ENTRY_POINT.call_once(|| {
        if cfg!(debug_assertions) {
            fern::Dispatch::new()
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "{} [{}] At {}:{}:\n{}",
                        chrono::Local::now().format("%H:%M:%S.%f"),
                        record.level(),
                        record.file().unwrap(),
                        record.line().unwrap(),
                        message
                    ))
                })
                .level(LevelFilter::Trace)
                .chain(std::io::stdout())
        } else {
            fern::Dispatch::new()
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "{} [{}] {}",
                        chrono::Local::now().format("%H:%M:%S.%f"),
                        record.level(),
                        message
                    ))
                })
                .level(LevelFilter::Info)
        }
        .chain(fern::log_file(Path::new(env!("INSTALL_ROOT")).join("log.txt")).unwrap())
        .apply()
        .unwrap();

        fn log_error_fn(message: &str) {
            show_error_message_box("BridgeVR crashed", &message);
        }

        bridgevr_common::logging::set_show_error_fn(log_error_fn);
    });
}
