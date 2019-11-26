use log::error;
use std::fmt::Debug;

pub type StrResult<T> = std::result::Result<T, String>;

// use crate::utils::Result;

// fn default_display_panic_fn(_: &str) {}

// static mut DISPLAY_PANIC_CB: fn(&str) = default_display_panic_fn;

// pub fn set_display_panic_fn(cb: fn(&str)) {
//     unsafe { DISPLAY_PANIC_CB = cb };
// }

fn default_display_error_fn(_: &str) {}

pub static mut _DISPLAY_ERROR_CB: fn(&str) = default_display_error_fn;

pub fn set_display_error_fn(cb: fn(&str)) {
    unsafe { _DISPLAY_ERROR_CB = cb };
}

pub fn error_format(message: &str, file_name: &str, line: u32) -> String {
    format!("Error in {} at line {}: {}", file_name, line, message)
}

// pub fn _log_panic(message: &str, file_name: &str, line: u32) -> ! {
//     error!("{}", message);
//     let panic_message = panic_format(&message, file_name, line);
//     unsafe { DISPLAY_PANIC_CB(&panic_message) };
//     panic!("{}", panic_message);
// }

// pub fn _ok_or_panic<T, E: std::fmt::Debug>(
//     expr: Result<T, E>,
//     expect: &str,
//     file_name: &str,
//     line: u32,
// ) -> T {
//     match expr {
//         Ok(t) => t,
//         Err(err) => {
//             let message = format!(r#"{} "{:?}""#, expect, err);
//             error!("{}", message);
//             unsafe { DISPLAY_PANIC_CB(&panic_format(&message, file_name, line)) };
//             panic!("{}", message);
//         }
//     }
// }

// pub fn _some_or_panic<T>(expr: Option<T>, none_message: &str, file_name: &str, line: u32) -> T {
//     match expr {
//         Some(t) => t,
//         None => {
//             error!("{}", none_message);
//             unsafe { DISPLAY_PANIC_CB(&panic_format(none_message, file_name, line)) };
//             panic!("{}", none_message);
//         }
//     }
// }

// #[macro_export]
// macro_rules! log_panic {
//     ($($message:expr),+) => {{
//         let message = String::new() $(+ &format!("{}", $message))+;
//         $crate::logging::_log_panic(&message, file!(), line!());
//     }};
// }

// #[macro_export]
// macro_rules! ok_or_panic {
//     ($e:expr, $($expect:expr),+) => {{
//         let message = String::new() $(+ &format!("{}", $expect))+;
//         $crate::logging::_ok_or_panic($e, &message, file!(), line!())
//     }};
// }

// #[macro_export]
// macro_rules! ok_or_panic_prefix {
//     ($($call_path:ident).+($($params:tt)*), $msg_prefix:expr) => {{
//         let message = format!("[{}] {} ", $msg_prefix, stringify!($($call_path).+));
//         $crate::logging::_ok_or_panic($($call_path).+($($params)*), &message, file!(), line!())
//     }};
// }

// #[macro_export]
// macro_rules! some_or_panic {
//     ($e:expr, $($none_message:expr),+) => {{
//         let message = String::new() $(+ &format!("{}", $none_message))+;
//         $crate::logging::_some_or_panic($e, &message, file!(), line!())
//     }};
// }

// pub fn _ok_or_err<T, E: Debug>(
//     expr: Result<T, E>,
//     expect: &str,
//     file_name: &str,
//     line: u32,
// ) -> Result<T, String> {
//     expr.map_err()
//     // match expr {
//     //     Ok(t) => Ok(t),
//     //     Err(err) => {
//     //         let message = format!(r#"{} "{:?}""#, expect, err);
//     //         error!("{}", message);
//     //         unsafe { DISPLAY_ERROR_CB(&error_format(&message, file_name, line)) };
//     //         Err(())
//     //     }
//     // }
// }

// #[macro_export]
// macro_rules! ok_or_err {
//     ($res:expr, $($expect:expr),+) => {
//         $res.map_err(|e| {
//             let message = String::new() $(+ &format!(r#"{} "{:?}""#, $expect, e))+;
//             log::error!("{}", message);
//             $crate::logging::error_format(&message, file!(), line!())
//         })
//     };
// }

// #[macro_export]
// macro_rules! some_or_error {
//     ($res:expr, $($none_message:expr),+) => {
//         $res.ok_or_else(|| {
//             let message = String::new() $(+ &format!("{}", $none_message))+;
//             log::error!("{}", message);
//             $crate::logging::error_format(&message, file!(), line!())
//         })
//     };
// }

#[macro_export]
macro_rules! trace_err {
    ($res:expr, $context:expr $(, $expect:expr)?) => {
        $res.map_err(|e| {
            String::from(format!("[{}] At {}:{}", $context, file!(), line!()))
                $(+ &format!(", {}", $expect))? +
                &format!(":\n{:?}", e)
        })
    };
}

#[macro_export]
macro_rules! trace_none {
    ($res:expr, $context:expr $(, $none_message:expr)?) => {
        $res.ok_or_else(|| {
            String::from(format!("[{}] At {}:{}", $context, file!(), line!()))
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

// pub fn _some_or_error<T>(
//     expr: Option<T>,
//     none_message: &str,
//     file_name: &str,
//     line: u32,
// ) -> Result<T, ()> {
//     match expr {
//         Some(t) => Ok(t),
//         None => {
//             error!("{}", none_message);
//             unsafe { DISPLAY_ERROR_CB(&error_format(&message, file_name, line)) };
//             Err(())
//         }
//     }
// }

// pub fn _some_or_panic<T>(expr: Option<T>, none_message: &str, file_name: &str, line: u32) -> T {
//     match expr {
//         Some(t) => t,
//         None => {
//             error!("{}", none_message);
//             unsafe { DISPLAY_PANIC_CB(&panic_format(none_message, file_name, line)) };
//             panic!("{}", none_message);
//         }
//     }
// }

// #[macro_export]
// macro_rules! ok_or_err {
//     ($res:expr, $($none_message:expr),+) => {{
//         let message = String::new() $(+ &format!("{}", $none_message))+;
//         $crate::logging::_some_or_panic($e, &message, file!(), line!())
//     }};
// }
