#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# The `contract-check` lane (§A.5).
#
# The generated OpenAPI client is the sole contract between the two surfaces
# (§24.5): the backend's utoipa annotations are the source of truth, and the
# TypeScript client is regenerated in the same change as any handler or DTO
# edit. This script is what makes "the client was regenerated in this PR" a
# machine-checked fact rather than a reviewer's trust.
#
# It regenerates from the current annotations and diffs against what is
# committed. A non-empty diff fails the lane, and the fix is to run
# `scripts/generate-openapi-client.sh` and commit what it writes.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

generated_client="frontend/src/lib/api/generated"

if ! grep -q '^utoipa' backend/crates/api/Cargo.toml 2>/dev/null; then
	if [ -e "$generated_client" ]; then
		echo "A generated client exists at $generated_client but backend/crates/api" >&2
		echo "declares no utoipa dependency. One of the two is wrong; the annotations are" >&2
		echo "the source of truth (§24.5)." >&2
		exit 1
	fi
	echo "No OpenAPI surface yet: backend/crates/api declares no utoipa dependency and"
	echo "no generated client exists. Nothing to diff."
	exit 0
fi

if [ ! -e "$generated_client/schema.d.ts" ]; then
	echo "backend/crates/api declares utoipa but no client is committed at" >&2
	echo "$generated_client. Run scripts/generate-openapi-client.sh (§24.5)." >&2
	exit 1
fi

exec "$root/scripts/generate-openapi-client.sh" --check
