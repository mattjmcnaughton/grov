use clap::Parser;
use grov::cli::Cli;
use tracing_subscriber::EnvFilter;

fn init_tracing(verbosity: u8) {
    let default_level = match verbosity {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
}

fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    tracing::debug!("grov starting with verbosity level {}", cli.verbose);
    tracing::debug!("command: {:?}", cli.command);
}
