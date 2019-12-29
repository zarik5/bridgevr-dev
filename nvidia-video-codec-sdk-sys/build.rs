use std::{io::Write, *};

fn main() {
    let out_path = path::PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut bindings_header_content = String::new();
    if cfg!(feature = "encoder") {
        bindings_header_content += "\n#include \"encoder.h\"";
    }
    if cfg!(feature = "decoder") {
        bindings_header_content += "\n#include \"decoder.h\"";
    }
    if cfg!(feature = "cuda") {
        bindings_header_content += "\n#include <cuda.h>";
    }

    let bindings_header_path = out_path.join("bindings.h");
    fs::File::create(&bindings_header_path)
        .expect("bindings")
        .write(bindings_header_content.as_bytes())
        .expect("bindings");

    bindgen::builder()
        .clang_arg("-Isrc/")
        .clang_arg("-Iinclude/")
        .header(bindings_header_path.to_string_lossy())
        .blacklist_item(".+_GUID")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .derive_default(true)
        .generate()
        .expect("bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("bindings.rs");
}
