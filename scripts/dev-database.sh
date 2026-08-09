#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Rebuilds the scratch database the sqlx query macros are checked against, then
# regenerates the offline query data in `.sqlx/`.
#
# `sqlx::query!` checks every statement against a real schema at compile time.
# CI and a fresh clone have no DATABASE_URL, so the macros read the committed
# metadata in `.sqlx/` instead. This script sets DATABASE_URL and
# SQLX_OFFLINE=false together to force the live check that regenerates it.
# Run this after changing a migration or a query, and commit what it writes.
#
# `--check` compares instead of writing: it fails when the committed data is not
# what the current queries and migrations produce. That is the CI form. `.sqlx/`
# is generated, so nothing except this stops a query change from merging with
# stale metadata that still compiles.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# `--workspace` seeds the array so it is never empty. An empty array expands
# unset under `set -u` on bash before 4.4, which is the bash macOS ships.
prepare_args=(--workspace)
check=false
case "${1:-}" in
"") ;;
--check)
  prepare_args+=(--check)
  check=true
  ;;
*)
  echo "usage: ${0##*/} [--check]" >&2
  exit 2
  ;;
esac

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
  SQLX_OFFLINE=false DATABASE_URL="sqlite://$db" \
    cargo sqlx prepare "${prepare_args[@]}" -- --all-targets
  if [ "$check" = true ]; then
    echo ".sqlx/ matches the current queries"
  else
    echo "offline query data regenerated in .sqlx/"
  fi
else
  echo "sqlx-cli is not installed; skipping 'cargo sqlx prepare'." >&2
  echo "Install it with: cargo install sqlx-cli --no-default-features --features sqlite,rustls" >&2
  exit 1
fi
