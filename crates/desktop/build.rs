fn main() {
    println!("cargo:rerun-if-changed=translations");
    let emit_debug_info = std::env::var("PROFILE").is_ok_and(|profile| profile != "release");
    let config = slint_build::CompilerConfiguration::new()
        .with_debug_info(emit_debug_info)
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/main.slint", config)
        .unwrap_or_else(|error| panic!("failed to compile production Slint UI: {error}"));
}
