#!/usr/bin/env zsh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

echo "[ratatui] updating goldens via tests"
(cd "$repo_root" && UPDATE_GOLDENS=1 cargo test -p ratatui-components-markdown golden)

gen_glow() {
  local width="$1"
  local fixture="$2"
  local out="$3"

  mkdir -p "$(dirname "$out")"
  cat "$fixture" | (cd "$repo_root/repo-ref/glow" && NO_COLOR=1 go run . -w "$width" -s dracula -) \
    | perl -pe 's{\x1b\[[0-9;?]*[ -/]*[@-~]}{}g; s{\x1b\][^\x07]*(\x07|\x1b\\\\)}{}g' \
    | sed -E 's/[[:space:]]+$//' \
    | perl -0777 -pe 'my @l=split(/\n/,$_, -1); my $min; for my $s (@l){ next if $s =~ /^\s*$/; if($s =~ /^( +)/){ my $n=length($1); $min=$n if !defined($min) || $n<$min; } else { $min=0; last } } $min//=0; if($min>0){ for(@l){ s/^ {$min}// } } $_=join("\n", @l);' \
    | perl -0777 -pe 's/\A(?:[ \t]*\n)+//; s/(?:\n[ \t]*)+\z/\n/;' \
    > "$out"
}

echo "[glow] generating reference snapshots (plain text)"
gen_glow 80 "$repo_root/docs/fixtures/glow_parity.md" "$repo_root/docs/fixtures/golden/glow/glow_parity__glow_like__w80.txt"
gen_glow 40 "$repo_root/docs/fixtures/glow_parity.md" "$repo_root/docs/fixtures/golden/glow/glow_parity__glow_like__w40.txt"

echo "done"
