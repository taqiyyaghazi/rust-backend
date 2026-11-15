#!/usr/bin/env bash
set -euo pipefail

echo "=== 1. Login to Cloudflare (if not already authenticated) ==="
npx --yes wrangler@3 whoami || npx --yes wrangler@3 login

echo ""
echo "=== 2. Creating D1 Databases (Staging & Production) ==="
echo "Creating rust-backend-db-staging..."
npx --yes wrangler@3 d1 create rust-backend-db-staging || true

echo ""
echo "Creating rust-backend-db-prod..."
npx --yes wrangler@3 d1 create rust-backend-db-prod || true

echo ""
echo "=== Next Steps ==="
echo "1. Salin database_id dari output di atas ke wrangler.toml pada bagian [[env.staging.d1_databases]] dan [[env.production.d1_databases]]."
echo "2. Set secret JWT_SECRET (minimal 32 karakter) untuk staging & production:"
echo "   npx wrangler@3 secret put JWT_SECRET --env staging"
echo "   npx wrangler@3 secret put JWT_SECRET --env production"
echo "3. Tambahkan GitHub Repository Secrets:"
echo "   - CLOUDFLARE_API_TOKEN"
echo "   - CLOUDFLARE_ACCOUNT_ID"
