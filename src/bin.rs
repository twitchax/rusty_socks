#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

use clap::Parser;
use tracing_subscriber::EnvFilter;

use rsocks::config::Config;
use rsocks::helpers::Res;
use rsocks::run;

#[tokio::main]
async fn main() -> Res<()> {
    let config = Config::parse();

    // Default to `info`, overridable via `RUST_LOG` (e.g. `RUST_LOG=rsocks=debug`).
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    run(config).await
}
