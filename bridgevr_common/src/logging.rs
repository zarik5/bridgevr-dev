pub type StrResult<T> = std::result::Result<T, String>;

fn default_display_error_fn(_: &str) {}
pub static mut _DISPLAY_ERROR_CB: fn(&str) = default_display_error_fn;

pub fn set_display_error_fn(cb: fn(&str)) {
    unsafe { _DISPLAY_ERROR_CB = cb };
}

pub fn error_format(message: &str, file_name: &str, line: u32) -> String {
    format!("Error in {} at line {}: {}", file_name, line, message)
}

#[macro_export]
macro_rules! trace_err {
    ($res:expr $(, $expect:expr)?) => {
        $res.map_err(|e| {
            String::from(format!("[{}] At {}:{}", TRACE_CONTEXT, file!(), line!()))
                $(+ &format!(", {}", $expect))? +
                &format!(":\n{:?}", e)
        })
    };
}

#[macro_export]
macro_rules! trace_none {
    ($res:expr $(, $none_message:expr)?) => {
        $res.ok_or_else(|| {
            String::from(format!("[{}] At {}:{}", TRACE_CONTEXT, file!(), line!()))
                $(+ ", " + $none_message)?
        })
    };
}

#[macro_export]
macro_rules! display_err {
    ($res:expr) => {
        $res.map_err(|e| {
            log::error!("{:?}", e);
            unsafe { $crate::logging::_DISPLAY_ERROR_CB(&format!("{:?}", e)) };
        })
    };
}