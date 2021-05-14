use shaderc::*;
use std::{env::var, fs::*, path::*};

fn compile_shader(name: &str, options: &CompileOptions, out_dir: &Path) {
    let shader_text = read_to_string(format!("shaders/{}.glsl", name)).unwrap();

    let mut shader_compiler = Compiler::new().unwrap();
    let shader_artifact = shader_compiler
        .compile_into_spirv(
            &shader_text,
            ShaderKind::Compute,
            &format!("{}.glsl", name),
            "main",
            Some(options),
        )
        .unwrap();
    let shader_bytecode = shader_artifact.as_binary_u8();

    write(out_dir.join(format!("{}.spv", name)), shader_bytecode).unwrap();
}

fn shader_options<'a>() -> CompileOptions<'a> {
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_include_callback(|source_name, _, _, _| {
        let content = read_to_string(format!("shaders/{}.glsl", source_name)).unwrap();
        Ok(ResolvedInclude {
            resolved_name: source_name.to_owned(),
            content,
        })
    });

    if cfg!(debug_assertions) {
        options.set_generate_debug_info();
        options.set_optimization_level(OptimizationLevel::Zero);
    } else {
        options.set_optimization_level(OptimizationLevel::Performance);
    }

    options
}

fn main() {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());

    let shader_options = shader_options();

    compile_shader("shader", &shader_options, &out_dir);
}
