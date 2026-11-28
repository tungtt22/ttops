#!/usr/bin/env bash
# Pre-PR quality gate (constitution: Development Workflow and Quality Gates).
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/check-hardcode.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix ui run typecheck
npm --prefix ui test
