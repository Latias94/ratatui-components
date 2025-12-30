#!/usr/bin/env zsh
set -euo pipefail

echo "== ratatui-components release check =="

echo ""
echo "== Format =="
cargo fmt --check

echo ""
echo "== Workspace build (no extra features) =="
cargo check --workspace --all-targets

echo ""
echo "== Workspace tests (nextest) =="
cargo nextest run

echo ""
echo "== Facade feature matrix (compile-only) =="
cargo check -p ratatui-components --no-default-features
cargo check -p ratatui-components --features markdown
cargo check -p ratatui-components --features ansi
cargo check -p ratatui-components --features diff
cargo check -p ratatui-components --features transcript
cargo check -p ratatui-components --features syntect
cargo check -p ratatui-components --features treesitter,treesitter-langs-common

echo ""
echo "== Examples (build) =="
cargo build -p ratatui-components --example datagrid
cargo build -p ratatui-components --example virtual_list

cargo build -p ratatui-components --example dump --features markdown
cargo build -p ratatui-components --example render_core --features markdown
cargo build -p ratatui-components --example render_core --features markdown,syntect

cargo build -p ratatui-components --example code_ansi --features ansi,syntect
cargo build -p ratatui-components --example preview --features diff,markdown,syntect
cargo build -p ratatui-components --example transcript --features transcript,syntect
cargo build -p ratatui-components --example agent_mvp --features transcript,syntect
cargo build -p ratatui-components --example mdstream --features mdstream,syntect

echo ""
echo "OK"

