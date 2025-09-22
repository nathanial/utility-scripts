mod app;
mod cli;
mod delete;
mod git;
mod tui;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;

use crate::app::App;
use crate::cli::Cli;
use crate::delete::{DeleteStatus, delete_branches};
use crate::git::{
    collect_merged_branches, current_branch_name, ensure_local_branch, open_repository,
    resolve_base_branch,
};

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let repo = open_repository(cli.repo.as_deref())?;

    let current_branch_result = current_branch_name(&repo);
    let current_branch_display = current_branch_result
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("(detached)")
        .to_string();

    let current_branch_for_base = current_branch_result.as_ref().ok().map(|s| s.as_str());

    let base_branch = match cli.base {
        Some(base) => base,
        None => resolve_base_branch(&repo, &cli.remote, current_branch_for_base)
            .context("Unable to resolve default base branch; use --base to set explicitly")?,
    };

    ensure_local_branch(&repo, &base_branch)?;

    let mut merged = collect_merged_branches(&repo, &base_branch)?;
    if let Ok(name) = &current_branch_result {
        merged.retain(|branch| branch.name != base_branch && branch.name != *name);
    } else {
        merged.retain(|branch| branch.name != base_branch);
    }

    if merged.is_empty() {
        println!(
            "No merged branches found relative to '{base_branch}' in {}.",
            repo.path().display()
        );
        return Ok(());
    }

    if cli.list_only {
        print_branch_listing(&merged, &base_branch, &current_branch_display);
        return Ok(());
    }

    let mut app = App::new(merged, base_branch.clone(), current_branch_display.clone());
    app.set_message("Use space to toggle branches. Press enter to confirm deletion.");

    tui::run(&mut app)?;

    if !app.confirmed() {
        println!("Aborted - no branches deleted.");
        return Ok(());
    }

    let selections = app.selected_branch_infos();

    let results = delete_branches(&repo, &selections, cli.dry_run)?;

    summarize_results(&results, cli.dry_run);

    Ok(())
}

fn print_branch_listing(
    branches: &[crate::git::BranchInfo],
    base_branch: &str,
    current_branch: &str,
) {
    println!("Merged branches relative to '{base_branch}' (current: {current_branch}):");
    for branch in branches {
        let tip_id = branch.tip.to_string();
        let short = &tip_id[..tip_id.len().min(7)];
        let summary = branch.summary.as_deref().unwrap_or("<no commit message>");
        match &branch.committer {
            Some(committer) => println!("  {:<24} {}  {}", branch.name, short, committer),
            None => println!("  {:<24} {}", branch.name, short),
        }
        println!("      {summary}");
    }
}

fn summarize_results(results: &[crate::delete::DeleteResult], dry_run: bool) {
    if results.is_empty() {
        println!("No branches selected - nothing to do.");
        return;
    }

    let mut deleted = Vec::new();
    let mut skipped = Vec::new();

    for result in results {
        match &result.status {
            DeleteStatus::Deleted => deleted.push(result.name.clone()),
            DeleteStatus::DryRun => deleted.push(result.name.clone()),
            DeleteStatus::Error(err) => skipped.push(err.clone()),
        }
    }

    if dry_run {
        println!("Dry run - branches that would be deleted:");
    } else {
        println!("Deleted branches:");
    }

    for name in &deleted {
        println!("  {name}");
    }

    if !skipped.is_empty() {
        println!("\nWarnings:");
        for warning in skipped {
            println!("  {warning}");
        }
    }
}
