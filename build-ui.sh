#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"
cd crates/strex-ui/frontend && npm run build
cd ../../..
cargo build --bin strex
echo ""
echo "Build complete! Run with: ./target/debug/strex ui"
