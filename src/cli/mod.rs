pub mod commands;

use clap::{Parser, Subcommand};

/// grov: dev workspace services
#[derive(Parser, Debug)]
#[command(name = "grov", version, about)]
pub struct Cli {
    /// Increase log verbosity (-v for info, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Pull Docker images or verify native binaries for services
    Install {
        /// Service names to install
        #[arg(required = true)]
        services: Vec<String>,
    },
    /// Start services for the current grove
    Up {
        /// Service names to start (reads docker-compose.yml if omitted)
        services: Vec<String>,
    },
    /// Stop services for the current grove
    Down {
        /// Service names to stop (stops all if omitted)
        services: Option<Vec<String>>,
    },
    /// Print environment variables for running services
    Env,
    /// Show status of services for the current grove
    Status,
    /// Remove persisted data for the current grove
    Clean {
        /// Clean all groves system-wide
        #[arg(long, conflicts_with = "orphans")]
        all: bool,
        /// Clean only orphaned groves (worktree directory no longer exists)
        #[arg(long, conflicts_with = "all")]
        orphans: bool,
        /// Show what would be cleaned without actually doing it
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_up_with_services() {
        let cli = Cli::parse_from(["grov", "up", "postgres", "minio"]);
        match cli.command {
            Commands::Up { ref services } => {
                assert_eq!(services, &["postgres", "minio"]);
            }
            _ => panic!("expected Up command"),
        }
    }

    #[test]
    fn parse_install_with_services() {
        let cli = Cli::parse_from(["grov", "install", "postgres"]);
        match cli.command {
            Commands::Install { ref services } => {
                assert_eq!(services, &["postgres"]);
            }
            _ => panic!("expected Install command"),
        }
    }

    #[test]
    fn parse_down_with_no_services() {
        let cli = Cli::parse_from(["grov", "down"]);
        match cli.command {
            Commands::Down { ref services } => {
                assert!(services.is_none());
            }
            _ => panic!("expected Down command"),
        }
    }

    #[test]
    fn parse_down_with_services() {
        let cli = Cli::parse_from(["grov", "down", "postgres"]);
        match cli.command {
            Commands::Down { ref services } => {
                assert_eq!(services.as_deref(), Some(&["postgres".to_string()][..]));
            }
            _ => panic!("expected Down command"),
        }
    }

    #[test]
    fn parse_env() {
        let cli = Cli::parse_from(["grov", "env"]);
        assert!(matches!(cli.command, Commands::Env));
    }

    #[test]
    fn parse_status() {
        let cli = Cli::parse_from(["grov", "status"]);
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_clean() {
        let cli = Cli::parse_from(["grov", "clean"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                all: false,
                orphans: false,
                dry_run: false,
            }
        ));
    }

    #[test]
    fn parse_clean_all() {
        let cli = Cli::parse_from(["grov", "clean", "--all"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                all: true,
                orphans: false,
                ..
            }
        ));
    }

    #[test]
    fn parse_clean_orphans() {
        let cli = Cli::parse_from(["grov", "clean", "--orphans"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                all: false,
                orphans: true,
                ..
            }
        ));
    }

    #[test]
    fn parse_clean_all_and_orphans_fails() {
        let result = Cli::try_parse_from(["grov", "clean", "--all", "--orphans"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_clean_dry_run() {
        let cli = Cli::parse_from(["grov", "clean", "--dry-run"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                all: false,
                orphans: false,
                dry_run: true,
            }
        ));
    }

    #[test]
    fn parse_clean_all_dry_run() {
        let cli = Cli::parse_from(["grov", "clean", "--all", "--dry-run"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                all: true,
                dry_run: true,
                ..
            }
        ));
    }

    #[test]
    fn parse_clean_orphans_dry_run() {
        let cli = Cli::parse_from(["grov", "clean", "--orphans", "--dry-run"]);
        assert!(matches!(
            cli.command,
            Commands::Clean {
                orphans: true,
                dry_run: true,
                ..
            }
        ));
    }

    #[test]
    fn parse_verbosity_flags() {
        let cli = Cli::parse_from(["grov", "-v", "status"]);
        assert_eq!(cli.verbose, 1);

        let cli = Cli::parse_from(["grov", "-vv", "status"]);
        assert_eq!(cli.verbose, 2);
    }

    #[test]
    fn parse_unknown_command_fails() {
        let result = Cli::try_parse_from(["grov", "foobar"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_up_with_no_services() {
        let cli = Cli::parse_from(["grov", "up"]);
        match cli.command {
            Commands::Up { ref services } => {
                assert!(services.is_empty());
            }
            _ => panic!("expected Up command"),
        }
    }
}
