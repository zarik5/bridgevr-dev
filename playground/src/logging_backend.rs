// fn display_panic_fn(message: &str) {
    // msgbox::create("playground crashed", &message, msgbox::IconType::Error);
// }

pub fn init_logging() {
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
        .level(log::LevelFilter::Trace)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}
