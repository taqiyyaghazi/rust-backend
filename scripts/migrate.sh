#!/usr/bin/env bash
set -euo pipefail

echo "==> Running standalone database migrations via 'cargo run --bin migrate'..."
cargo run --bin migrate
echo "==> Migrations completed."
