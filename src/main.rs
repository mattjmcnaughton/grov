use clap::Parser;
use tracing_subscriber::EnvFilter;

/// grov: dev workspace services
#[derive(Parser)]
#[command(name = "grov", version, about)]
struct Cli {
    /// Increase log verbosity (-v for info, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

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
}
