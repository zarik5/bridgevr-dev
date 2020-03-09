#![allow(
    clippy::all,
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    improper_ctypes // u128 not FFi safe
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
