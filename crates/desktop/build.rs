fn main() {
    slint_build::compile("ui/main.slint")
        .unwrap_or_else(|error| panic!("failed to compile production Slint UI: {error}"));
}
