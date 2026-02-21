#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/8] Rust format check"
cargo fmt --all -- --check

echo "[2/8] Rust lint (clippy, deny warnings)"
cargo clippy --all-targets --all-features -- -D warnings

echo "[3/8] Rust tests"
cargo test --all-targets

echo "[4/8] Rust dependency audit"
if command -v cargo-audit >/dev/null 2>&1; then
  cargo-audit audit
elif [ -x "$HOME/.cargo/bin/cargo-audit" ]; then
  "$HOME/.cargo/bin/cargo-audit" audit
else
  echo "cargo-audit is not installed. Install with: cargo install cargo-audit"
  exit 1
fi

cd "$ROOT_DIR/dashboard"

echo "[5/8] Frontend lint"
npm run lint

echo "[6/8] Frontend production build"
npm run build

echo "[7/8] Frontend production dependency audit"
npm audit --omit=dev

echo "[8/8] Frontend full dependency audit"
npm audit

echo "Publish preflight passed."
