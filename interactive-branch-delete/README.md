# Interactive Branch Delete

Rust-based CLI that provides a TUI for reviewing Git branches (merged or not), selecting the ones you want, and deleting them in a single action.

## Status
- ✅ Ratatui-powered selector with keyboard controls, branch ages, and merged/unmerged status.
- ✅ Safe deletion pipeline with dry-run mode and result summary.
- ✅ Installed via `install.sh` as the `us-interactive-branch-delete` binary.
- ⏳ Enhancements like branch filtering and protected rules.

## Build
```bash
cargo build --release
```

After building, the executable lives at `target/release/us-interactive-branch-delete`.

## Usage
```bash
us-interactive-branch-delete \
  --base main \
  --remote origin
```

Branches are sorted by last commit age (oldest first). Merged branches display in green, while unmerged branches remain highlighted in red so you can make deliberate choices before deleting.

Or run directly from source during development:
```bash
cargo run -- \
  --base main \
  --remote origin
```

### Flags
- `--repo <path>`: target repository (defaults to current directory).
- `--base <branch>`: set the base branch explicitly.
- `--remote <name>`: remote used when auto-resolving the default base branch.
- `--dry-run`: show which branches would be deleted without performing the deletions.
- `--list-only`: print merged branches and skip launching the TUI.

## Next Steps
- Add fuzzy filtering/protected-branch presets inside the selector.
- Offer an undo script that records `git branch` commands for each deletion.
- Package via `cargo install`/Homebrew once the feature set settles.
