#![allow(clippy::all, clippy::nursery, clippy::pedantic)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
pub use root::*;
pub use root::vr::*;

include!(concat!(env!("OUT_DIR"), "/properties_mappings.rs"));