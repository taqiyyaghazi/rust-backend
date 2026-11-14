#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "====================================================================="
echo " [Integration Test Flow] Docker Compose -> Migrations -> Cargo Test "
echo "====================================================================="

# 1. Start PostgreSQL with Docker Compose (or ensure local cluster is up)
if command -v docker &> /dev/null; then
  echo "==> Starting PostgreSQL via docker compose..."
  docker compose up -d postgres

  echo "==> Waiting for PostgreSQL to be healthy..."
  until docker compose exec -T postgres pg_isready -U "${POSTGRES_USER:-ddd_user}" -d "${POSTGRES_DB:-ddd_backend}" &> /dev/null; do
    echo "    Waiting for PostgreSQL container..."
    sleep 1
  done
elif command -v pg_isready &> /dev/null && pg_isready -h localhost -p 5432 &> /dev/null; then
  echo "==> PostgreSQL is already running on localhost:5432."
else
  echo "==> Neither Docker nor running PostgreSQL detected."
  exit 1
fi

# 2. Run standalone migrations
echo "==> Running standalone database migrations..."
cargo run --bin migrate

# 3. Run cargo test against PostgreSQL
echo "==> Running integration tests against PostgreSQL database..."
DATABASE_URL="${DATABASE_URL:-postgres://ddd_user:ddd_password@localhost:5432/ddd_backend}" cargo test --test integration_ddd_flow -- --nocapture

echo "====================================================================="
echo " Integration tests finished successfully!"
echo "====================================================================="
