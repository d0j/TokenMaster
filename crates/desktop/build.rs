fn main() {
    let emit_debug_info = std::env::var("PROFILE").is_ok_and(|profile| profile != "release");
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(emit_debug_info),
    )
    .unwrap_or_else(|error| panic!("failed to compile production Slint UI: {error}"));
}
