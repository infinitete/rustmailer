#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

if ! command -v git >/dev/null 2>&1; then
  printf 'Error: git is not installed or not in PATH.\n' >&2
  exit 1
fi

if ! git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  printf 'Error: %s is not inside a Git worktree.\n' "$REPO_ROOT" >&2
  exit 1
fi

git -C "$REPO_ROOT" config core.hooksPath .githooks
HOOKS_PATH=$(git -C "$REPO_ROOT" config --get core.hooksPath)

printf 'Configured Git hooks path: %s\n' "$HOOKS_PATH"
printf 'Git hooks are now active for this clone.\n'
