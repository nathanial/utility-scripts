use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use git2::{BranchType, Oid, Repository};

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub tip: Oid,
    pub summary: Option<String>,
    pub committer: Option<String>,
    pub commit_timestamp: Option<i64>,
    pub merged: bool,
}

pub fn open_repository(path: Option<&Path>) -> Result<Repository> {
    match path {
        Some(dir) => Repository::discover(dir)
            .with_context(|| format!("Failed to discover a Git repository from {}", dir.display())),
        None => Repository::discover(".")
            .context("Failed to discover a Git repository from current directory"),
    }
}

pub fn current_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head().context("Failed to resolve HEAD")?;
    if head.is_branch() {
        head.shorthand()
            .map(|s| s.to_string())
            .context("HEAD does not have a branch shorthand")
    } else {
        Err(anyhow!("Detached HEAD state; please checkout a branch"))
    }
}

pub fn resolve_base_branch(
    repo: &Repository,
    remote: &str,
    current_branch: Option<&str>,
) -> Result<String> {
    let remote_head = format!("refs/remotes/{remote}/HEAD");
    if let Ok(reference) = repo.find_reference(&remote_head) {
        if let Some(target) = reference.symbolic_target() {
            if let Some(stripped) = target.strip_prefix(&format!("refs/remotes/{remote}/")) {
                return Ok(stripped.to_string());
            }
        }
    }

    for candidate in ["refs/heads/main", "refs/heads/master"] {
        if repo.find_reference(candidate).is_ok() {
            return Ok(candidate.trim_start_matches("refs/heads/").to_string());
        }
    }

    if let Some(branch) = current_branch {
        Ok(branch.to_string())
    } else {
        Err(anyhow!(
            "Unable to determine a base branch. Specify one with --base."
        ))
    }
}

pub fn ensure_local_branch(repo: &Repository, name: &str) -> Result<()> {
    let reference_name = format!("refs/heads/{name}");
    repo.find_reference(&reference_name)
        .with_context(|| format!("Local branch '{name}' not found"))?
        .resolve()
        .context("Unable to resolve base branch reference")?;
    Ok(())
}

pub fn collect_local_branches(repo: &Repository, base_branch: &str) -> Result<Vec<BranchInfo>> {
    let base_ref = repo
        .find_reference(&format!("refs/heads/{base_branch}"))
        .with_context(|| format!("Failed to find reference for base branch '{base_branch}'"))?;
    let base_commit = base_ref
        .peel_to_commit()
        .context("Failed to peel base branch to commit")?;
    let base_oid = base_commit.id();

    let mut merged = Vec::new();

    let branches = repo
        .branches(Some(BranchType::Local))
        .context("Failed to enumerate local branches")?;
    for branch_result in branches {
        let (branch, _) = branch_result.context("Encountered an error while iterating branches")?;
        let name = match branch.name() {
            Ok(Some(name)) => name.to_string(),
            Ok(None) => continue,
            Err(err) => {
                eprintln!("Skipping branch with invalid UTF-8 name: {err}");
                continue;
            }
        };

        let reference = branch.into_reference();
        let target = match reference.target() {
            Some(oid) => oid,
            None => continue,
        };
        let commit = repo
            .find_commit(target)
            .with_context(|| format!("Failed to resolve commit for branch '{name}'"))?;

        let commit_time = commit.time();
        let timestamp = commit_time.seconds() - i64::from(commit_time.offset_minutes()) * 60;

        let merged_into_base = repo.graph_descendant_of(base_oid, commit.id())?;

        merged.push(BranchInfo {
            name,
            tip: commit.id(),
            summary: commit.summary().map(|s| s.trim().to_string()),
            committer: commit.author().name().map(|s| s.to_string()),
            commit_timestamp: (timestamp >= 0).then_some(timestamp),
            merged: merged_into_base,
        });
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(merged)
}

impl BranchInfo {
    pub fn age(&self, now: SystemTime) -> Option<Duration> {
        let timestamp = self.commit_timestamp?;
        let commit_time = UNIX_EPOCH.checked_add(Duration::from_secs(timestamp as u64))?;
        now.duration_since(commit_time).ok()
    }
}
