use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Pilot {
    /// Summarize declared pilot assignments and digest-verified full reports.
    Summarize {
        /// JSON inventory; report paths are relative to its containing directory.
        #[arg(long)]
        input: PathBuf,
    },
}
