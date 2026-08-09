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
# There is no OpenAPI surface yet — the HTTP routes arrive with the API crate —
# so today the script asserts that, and fails the moment the surface appears
# without a generator to keep it honest. It reports its own obsolescence rather
# than passing quietly through the change that makes it wrong.

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

echo "backend/crates/api now declares utoipa, so this lane must regenerate the" >&2
echo "client and diff it against the committed one. Extend" >&2
echo "scripts/check-openapi-contract.sh in the change that introduced the OpenAPI" >&2
echo "surface (§A.5, §24.5)." >&2
exit 1
