#!/usr/bin/env bash
set -euo pipefail

# Load .env if present
if [ -f .env ]; then
  # shellcheck disable=SC2046
  export $(grep -v '^#' .env | xargs)
fi

DATABASE_URL="${DATABASE_URL:-postgres://ddd_user:ddd_password@localhost:5432/ddd_backend}"

echo "==> Running standalone database migrations via 'cargo run --bin migrate'..."
DATABASE_URL="${DATABASE_URL}" cargo run --bin migrate
echo "==> Migrations completed."
