use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Pilot {
    /// Add a digest-verified integrity seal before any observations are recorded.
    Seal {
        /// JSON pilot plan. The sealed manifest is written to standard output.
        #[arg(long)]
        input: PathBuf,
    },
    /// Emit the exact sealed subject for an external owner signature.
    AuthorizationSubject {
        /// Sealed JSON pilot plan with no observations.
        #[arg(long)]
        input: PathBuf,
    },
    /// Emit the completed evidence subject for an independent reviewer signature.
    AcceptanceSubject {
        /// Completed JSON pilot inventory and report archive.
        #[arg(long)]
        input: PathBuf,
        /// External trust store containing owner and reviewer keys.
        #[arg(long)]
        trust_store: PathBuf,
        /// External DSSE envelope authorizing the sealed pilot start.
        #[arg(long)]
        authorization: PathBuf,
    },
    /// Summarize declared pilot assignments and digest-verified full reports.
    Summarize {
        /// JSON inventory; report paths are relative to its containing directory.
        #[arg(long)]
        input: PathBuf,
        /// External trust store containing the configured owner's key.
        #[arg(long, requires = "authorization")]
        trust_store: Option<PathBuf>,
        /// External DSSE envelope authorizing the sealed pilot start.
        #[arg(long, requires = "trust_store")]
        authorization: Option<PathBuf>,
        /// External independent-reviewer DSSE acceptance or rejection.
        #[arg(long, requires_all = ["trust_store", "authorization"])]
        acceptance: Option<PathBuf>,
    },
}
