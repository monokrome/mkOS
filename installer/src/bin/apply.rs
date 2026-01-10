use anyhow::{bail, Result};
use std::env;
use tracing_subscriber::EnvFilter;

use mkos::apply;
use mkos::manifest::ManifestSource;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = env::args().collect();
    let source = ManifestSource::from_arg(args.get(1).map(|s| s.as_str()));

    // Validate that a manifest was provided
    if matches!(source, ManifestSource::Interactive) {
        bail!("mkos-apply requires a manifest. Usage: mkos-apply <manifest>");
    }

    apply::run(source)
}
