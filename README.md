# Utility Scripts

This repository collects small command-line helpers for macOS and Linux developers. Scripts are stored in subdirectories grouped by topic (for example, `git-utils/`).

## Quick install on macOS

1. Clone or download this repository to a location you control, for example:
   ```bash
   git clone https://example.com/utility-scripts.git ~/utility-scripts
   cd ~/utility-scripts
   ```
2. Run the installer to add the repository and its first-level folders to your shell `PATH` (defaults to `~/.zshrc`). The script also builds the Rust CLI and symlinks the release binary into `bin/`:
   ```bash
   ./install.sh
   ```
   - Pass `--profile` to target an alternate shell profile (for example `./install.sh --profile ~/.bash_profile`).
   - Use `--dry-run` to preview the PATH block before writing it, or `--force` to refresh an existing installation block.
3. Reload your shell profile so the PATH changes take effect:
   ```bash
   source ~/.zshrc
   ```
4. Validate everything is wired up by running one of the scripts from anywhere:
   ```bash
   us-git-delete-merged-branches --help
   us-interactive-branch-delete --help
   us-http-tap --help
   ```

## Manual PATH setup (alternative)

If you prefer to manage your PATH entries yourself, add something similar to your profile file:
```bash
export UTILITY_SCRIPTS_HOME="$HOME/utility-scripts"
export PATH="$UTILITY_SCRIPTS_HOME/git-utils:$PATH"
```
Add additional subdirectories as you create them, then reload your profile (`source ~/.zshrc`) for changes to take effect.

## Updating

Pull the latest changes and reload your shell profile if new directories were added to the repo:
```bash
cd "$UTILITY_SCRIPTS_HOME"
git pull
source ~/.zshrc
```

## macOS Notes

- Scripts stick to tools bundled with macOS whenever possible; any extra dependencies are documented alongside the relevant script.
- If you change shells or migrate machines, rerun `install.sh` (or copy your PATH block) so the scripts remain available.

Happy scripting!

## Included Commands (highlights)

- `us-interactive-branch-delete`: TUI to review and delete merged/unmerged Git branches, sorted by age.
- `us-http-tap`: Lightweight HTTP tap/proxy. Example:
  ```bash
  us-http-tap --listen 127.0.0.1:8888 --target 127.0.0.1:8080 --include-bodies
  # Point your client at http://127.0.0.1:8888 to see requests/responses
  ```
