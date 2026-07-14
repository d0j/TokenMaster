use std::path::Path;

use clap::Parser;
use tokenmaster_m0::args::Args;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.stress.is_some() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        let receipt = tokenmaster_m0::stress::run(&args, &root)?;
        println!("{receipt}");
        return Ok(());
    }

    let renderer_override = match std::env::var("TOKENMASTER_RENDERER") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("TOKENMASTER_RENDERER must be valid Unicode")
        }
    };
    let preferred =
        tokenmaster_m0::shell::RendererChoice::from_override(renderer_override.as_deref())?;
    let _renderer = tokenmaster_m0::shell::run_desktop(preferred)?;
    Ok(())
}
