use log::*;

pub fn init_logging() {
    if cfg!(debug_assertions) {
        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} [{}] in {}@{}: {}",
                    chrono::Local::now().format("%H:%M:%S.%f"),
                    record.level(),
                    record.file().unwrap(),
                    record.line().unwrap(),
                    message
                ))
            })
            .level(LevelFilter::Trace)
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
    .chain(std::io::stdout())
    .apply()
    .unwrap();
}
