# Utility Scripts

This repository collects small command-line helpers for macOS and Linux developers. Scripts are stored in subdirectories grouped by topic (for example, `git-utils/`).

## Installing on macOS

1. Clone or download this repository to a location you control, for example:
   ```bash
   git clone https://example.com/utility-scripts.git ~/utility-scripts
   ```
2. Add the utility script folders to your `PATH` so every script is runnable from any terminal session. Add the following to your shell profile (`~/.zshrc`, `~/.bash_profile`, etc.):
   ```bash
   export UTILITY_SCRIPTS_HOME="$HOME/utility-scripts"
   export PATH="$UTILITY_SCRIPTS_HOME/git-utils:$PATH"
   ```
   After editing the profile, reload it (for `zsh` run `source ~/.zshrc`). Add additional subdirectories as you create them.
3. Make sure scripts are executable (they already ship that way, but confirm after syncing):
   ```bash
   chmod +x "$UTILITY_SCRIPTS_HOME"/**
   ```
4. Validate everything is wired up by running one of the scripts from anywhere:
   ```bash
   us-git-delete-merged-branches --help
   ```

## Updating

Pull the latest changes and reload your shell profile if new directories were added to the repo:
```bash
cd "$UTILITY_SCRIPTS_HOME"
git pull
source ~/.zshrc
```

## macOS Notes

- Scripts stick to tools bundled with macOS whenever possible; any extra dependencies are documented alongside the relevant script.
- If you change shells or migrate machines, copy the `UTILITY_SCRIPTS_HOME` block into the new profile so the scripts remain on your `PATH`.

Happy scripting!
