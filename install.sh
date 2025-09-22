#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'USAGE'
Install utility-scripts into your shell PATH (macOS focus).

Usage: install.sh [--profile FILE] [--dry-run] [--force] [--help]

Options:
  --profile FILE  Shell profile to update (default: ~/.zshrc).
  --dry-run       Show planned changes without modifying files.
  --force         Replace any existing utility-scripts block in the profile.
  --help          Print this help message.
USAGE
}

profile="~/.zshrc"
dry_run=false
force=false

while (($#)); do
  case "$1" in
    --profile)
      [[ $# -lt 2 ]] && { echo "Missing value for --profile" >&2; exit 1; }
      profile="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    --force)
      force=true
      shift
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ $profile == ~* ]]; then
  profile="${HOME}${profile:1}"
fi

if [[ "${OSTYPE:-}" != darwin* ]]; then
  echo "Warning: install.sh is tailored for macOS shells; continuing anyway." >&2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
if [[ -z "$script_dir" || ! -d "$script_dir" ]]; then
  echo "Error: unable to determine repository root." >&2
  exit 1
fi

ibd_manifest="$script_dir/interactive-branch-delete/Cargo.toml"
ibd_binary_name="us-interactive-branch-delete"
ibd_release_binary="$script_dir/interactive-branch-delete/target/release/$ibd_binary_name"

httap_manifest="$script_dir/http-tap/Cargo.toml"
httap_binary_name="us-http-tap"
httap_release_binary="$script_dir/http-tap/target/release/$httap_binary_name"

bin_dir="$script_dir/bin"

build_interactive_branch_delete() {
  if [[ ! -f "$ibd_manifest" ]]; then
    return
  fi

  if $dry_run; then
    echo "[dry-run] Would run: cargo build --release --manifest-path '$ibd_manifest'"
    return
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "Warning: cargo not found; skipping interactive-branch-delete build." >&2
    return
  fi

  echo "Building $ibd_binary_name (release)..."
  if ! cargo build --release --manifest-path "$ibd_manifest"; then
    echo "Warning: cargo build failed; interactive-branch-delete binary may be stale." >&2
  fi
}

build_http_tap() {
  if [[ ! -f "$httap_manifest" ]]; then
    return
  fi

  if $dry_run; then
    echo "[dry-run] Would run: cargo build --release --manifest-path '$httap_manifest'"
    return
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "Warning: cargo not found; skipping http-tap build." >&2
    return
  fi

  echo "Building $httap_binary_name (release)..."
  if ! cargo build --release --manifest-path "$httap_manifest"; then
    echo "Warning: cargo build failed; http-tap binary may be stale." >&2
  fi
}

ensure_bin_dir() {
  if $dry_run; then
    echo "[dry-run] Would ensure directory exists: $bin_dir"
  else
    mkdir -p "$bin_dir"
  fi
}

link_interactive_branch_delete() {
  if [[ ! -f "$ibd_manifest" ]]; then
    return
  fi

  if $dry_run; then
    echo "[dry-run] Would symlink $ibd_release_binary -> $bin_dir/$ibd_binary_name"
    return
  fi

  if [[ -x "$ibd_release_binary" ]]; then
    ln -sf "$ibd_release_binary" "$bin_dir/$ibd_binary_name"
  else
    echo "Warning: release binary not found at $ibd_release_binary; skipping symlink." >&2
  fi
}

link_http_tap() {
  if [[ ! -f "$httap_manifest" ]]; then
    return
  fi

  if $dry_run; then
    echo "[dry-run] Would symlink $httap_release_binary -> $bin_dir/$httap_binary_name"
    return
  fi

  if [[ -x "$httap_release_binary" ]]; then
    ln -sf "$httap_release_binary" "$bin_dir/$httap_binary_name"
  else
    echo "Warning: release binary not found at $httap_release_binary; skipping symlink." >&2
  fi
}

build_interactive_branch_delete
build_http_tap
ensure_bin_dir
link_interactive_branch_delete
link_http_tap

entries=("$script_dir")
while IFS= read -r -d '' dir; do
  entries+=("$dir")
done < <(find "$script_dir" -mindepth 1 -maxdepth 1 -type d ! -name '.*' -print0)

# Ensure bin_dir is represented so the PATH block exposes compiled binaries.
bin_seen=false
for existing in "${entries[@]}"; do
  if [[ $existing == "$bin_dir" ]]; then
    bin_seen=true
    break
  fi
done
if ! $bin_seen; then
  entries+=("$bin_dir")
fi

if ((${#entries[@]} == 0)); then
  echo "Error: no directories detected to add to PATH." >&2
  exit 1
fi

shell_escape() {
  local value="$1"
  printf "'"
  printf '%s' "$value" | sed "s/'/'\\''/g"
  printf "'"
}

marker_start="# >>> utility-scripts install >>>"
marker_end="# <<< utility-scripts install <<<"

escaped_root=$(shell_escape "$script_dir")

build_block() {
  local last_index=$(( ${#entries[@]} - 1 ))
  {
    printf '%s\n' "$marker_start"
    printf 'UTILITY_SCRIPTS_DIR=%s\n' "$escaped_root"
    printf 'for util_dir in \\\n'
    for i in "${!entries[@]}"; do
      local escaped_entry
      escaped_entry=$(shell_escape "${entries[i]}")
      if [[ $i -lt $last_index ]]; then
        printf '  %s \\\n' "$escaped_entry"
      else
        printf '  %s\n' "$escaped_entry"
      fi
    done
    printf 'do\n'
    printf '  if [ -d "$util_dir" ]; then\n'
    printf '    case ":$PATH:" in\n'
    printf '      *":$util_dir:"*) ;;\n'
    printf '      *) PATH="$util_dir:$PATH" ;;\n'
    printf '    esac\n'
    printf '  fi\n'
    printf 'done\n'
    printf 'export PATH\n'
    printf 'unset util_dir UTILITY_SCRIPTS_DIR\n'
    printf '%s\n' "$marker_end"
  } | cat
}

block_content=$(build_block)

if $dry_run; then
  echo "[dry-run] Would update profile: $profile"
  echo "[dry-run] The following block would be appended/replaced:"
  printf '\n%s\n' "$block_content"
  exit 0
fi

mkdir -p "$(dirname "$profile")"
touch "$profile"

if grep -Fq "$marker_start" "$profile"; then
  if ! $force; then
    echo "Error: existing utility-scripts block found in $profile. Use --force to replace." >&2
    exit 1
  fi
  tmp_file=$(mktemp)
  awk -v start="$marker_start" -v end="$marker_end" '
    $0 == start {inblock=1; next}
    $0 == end {if (inblock) {inblock=0; next}}
    !inblock {print}
  ' "$profile" >"$tmp_file"
  mv "$tmp_file" "$profile"
fi

{
  printf '\n%s\n' "$block_content"
} >>"$profile"

cat <<EOM
Updated $profile with utility-scripts PATH block.
Reload your shell configuration, for example:
  source "$profile"
EOM
