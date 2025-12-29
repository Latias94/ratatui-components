#!/usr/bin/env zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
rat="$repo_root/docs/fixtures/golden/ratatui"
glow="$repo_root/docs/fixtures/golden/glow"

if [[ ! -d "$rat" || ! -d "$glow" ]]; then
  echo "missing golden dirs. Run: $repo_root/scripts/gen-goldens.zsh" >&2
  exit 1
fi

for f in "$rat"/*.txt; do
  base="$(basename "$f")"
  g="$glow/$base"
  if [[ -f "$g" ]]; then
    echo "== $base =="
    git -c color.ui=always diff --no-index -- "$g" "$f" || true
    echo
  fi
done

