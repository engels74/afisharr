#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Rebuilds the scratch database the sqlx query macros are checked against, then
# regenerates the offline query data in `.sqlx/`.
#
# `sqlx::query!` verifies every statement against a real schema at compile time.
# CI and a fresh clone have no database, so the verified metadata is committed
# and `SQLX_OFFLINE=true` (see `.cargo/config.toml`) makes the macros read it.
# Run this after changing a migration or a query, and commit what it writes.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

db="$root/.dev/afisharr.db"
mkdir -p "$root/.dev"
rm -f "$db" "$db-wal" "$db-shm"

for migration in crates/afisharr/migrations/*.sql; do
  sqlite3 "$db" < "$migration"
done
sqlite3 "$db" "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version        BIGINT PRIMARY KEY,
    description    TEXT NOT NULL,
    installed_on   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success        BOOLEAN NOT NULL,
    checksum       BLOB NOT NULL,
    execution_time BIGINT NOT NULL
);"

echo "scratch database rebuilt at $db"

if command -v sqlx >/dev/null 2>&1; then
  SQLX_OFFLINE=false DATABASE_URL="sqlite://$db" cargo sqlx prepare --workspace -- --all-targets
  echo "offline query data regenerated in .sqlx/"
else
  echo "sqlx-cli is not installed; skipping 'cargo sqlx prepare'." >&2
  echo "Install it with: cargo install sqlx-cli --no-default-features --features sqlite,rustls" >&2
  exit 1
fi
