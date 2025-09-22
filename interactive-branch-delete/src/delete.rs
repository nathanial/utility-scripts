use anyhow::Result;
use git2::{BranchType, Repository};

use crate::git::BranchInfo;

#[derive(Debug, Clone)]
pub struct DeleteResult {
    pub name: String,
    pub status: DeleteStatus,
}

#[derive(Debug, Clone)]
pub enum DeleteStatus {
    Deleted,
    DryRun,
    Error(String),
}

pub fn delete_branches(
    repo: &Repository,
    branches: &[BranchInfo],
    dry_run: bool,
) -> Result<Vec<DeleteResult>> {
    let mut results = Vec::with_capacity(branches.len());

    for branch in branches {
        if dry_run {
            results.push(DeleteResult {
                name: branch.name.clone(),
                status: DeleteStatus::DryRun,
            });
            continue;
        }

        let delete_status = match repo.find_branch(&branch.name, BranchType::Local) {
            Ok(mut local_branch) => match local_branch.delete() {
                Ok(_) => DeleteStatus::Deleted,
                Err(err) => {
                    DeleteStatus::Error(format!("Failed to delete branch '{}': {err}", branch.name))
                }
            },
            Err(err) => DeleteStatus::Error(format!(
                "Failed to locate branch '{}' before deletion: {err}",
                branch.name
            )),
        };

        results.push(DeleteResult {
            name: branch.name.clone(),
            status: delete_status,
        });
    }

    Ok(results)
}
