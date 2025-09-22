# Repository Guidelines

## Project Structure & Module Organization
- `git-utils/`: bash scripts prefixed with `us-` (e.g., `us-git-delete-merged-branches`). Each script should be executable and self-contained.
- `interactive-branch-delete/`: Rust workspace for the interactive branch cleanup CLI (`us-interactive-branch-delete`). Source lives under `src/`, documentation under `README.md`.
- `bin/`: symlinks created by `install.sh` pointing to compiled binaries. Never check compiled artifacts into version control.
- Root-level docs (`README.md`, `InteractiveBranchDeletionCommand.md`, `AGENTS.md`) capture onboarding and design notes.

## Build, Test, and Development Commands
- `./install.sh` / `./install.sh --force`: Builds the Rust CLI in release mode and refreshes PATH exports in the configured shell profile.
- `cargo check` (run inside `interactive-branch-delete/`): Type-checks the Rust TUI without producing binaries.
- `cargo build --release`: Produces optimized binaries; required before shipping changes to the interactive CLI.
- `./us-git-delete-merged-branches --dry-run`: Smoke test the bash utility from anywhere after installation.

## Coding Style & Naming Conventions
- Bash scripts: POSIX-friendly, `set -euo pipefail`, functions in `snake_case`, user-facing commands prefixed with `us-`.
- Rust code: stable Rust edition 2024, enforce `cargo fmt` before committing, prefer `anyhow` for errors, modules follow `snake_case.rs` naming.
- Documentation: Markdown with sentence-case headings and fenced code blocks for commands.

## Testing Guidelines
- Rust: rely on `cargo check` + targeted `cargo test` (add tests under `interactive-branch-delete/src` or `tests/` when feasible). Name tests using the pattern `modname_behavior`.
- Shell: add lightweight verification scripts in `test/` or document manual steps; ensure `shellcheck` compatibility when the tool is available.
- Always verify `./install.sh --dry-run` succeeds after altering install logic.

## Commit & Pull Request Guidelines
- Commits: use imperative mood summaries (e.g., “Add branch age sorting to TUI”) and group related file changes together.
- Pull Requests: include a concise change list, testing evidence (`cargo check`, `install.sh --dry-run`), and link tracking issues. Add screenshots or terminal captures when modifying interactive output or installer UX.

## Security & Configuration Tips
- Avoid storing credentials or SSH remotes in scripts; rely on existing Git configuration.
- Keep `install.sh` idempotent—test in both `--dry-run` and `--force` modes before merging.
