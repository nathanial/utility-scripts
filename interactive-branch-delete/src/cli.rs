use std::path::PathBuf;

use clap::{Parser, ValueHint};

#[derive(Debug, Parser)]
#[command(
    name = "interactive-branch-delete",
    about = "Interactive helper to review and delete merged Git branches",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// Path to the Git repository (defaults to current directory).
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub repo: Option<PathBuf>,

    /// Base branch to compare against.
    #[arg(short, long)]
    pub base: Option<String>,

    /// Remote to inspect when resolving the default base branch.
    #[arg(short, long, default_value = "origin")]
    pub remote: String,

    /// Only list merged branches without entering interactive deletion.
    #[arg(long)]
    pub list_only: bool,

    /// Show what would happen without deleting.
    #[arg(long)]
    pub dry_run: bool,
}
