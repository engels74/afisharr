#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Regenerates the TypeScript client from the backend's utoipa annotations.
#
# The annotations are the source of truth (§24.5). This script is the only way
# the frontend learns what the API looks like, and `contract-check` runs it in
# CI and fails on a non-empty diff — which is what makes "the client was
# regenerated in this PR" a machine-checked fact rather than a reviewer's trust.
#
# Run this after changing any handler or DTO, and commit what it writes.
#
# `--check` compares instead of writing. That is the CI form.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

check=false
case "${1:-}" in
--check)
	check=true
	;;
"") ;;
*)
	echo "usage: $0 [--check]" >&2
	exit 2
	;;
esac

generated="frontend/src/lib/api/generated"
document="$generated/openapi.json"
types="$generated/schema.d.ts"

mkdir -p "$generated"

# The document is a function of the compiled binary's annotations, so it is
# printed by the binary rather than assembled by a second description of the
# surface. `--quiet` keeps cargo's progress off stdout.
#
# `cd backend` rather than `--manifest-path`: rustup resolves the toolchain by
# walking up from the working directory, so a run from the repository root gets
# whatever default toolchain is installed and refuses the workspace's MSRV.
(cd backend && cargo run --quiet -p afisharr -- openapi) >"$document.new"

# openapi-typescript emits types only; `openapi-fetch` supplies the runtime and
# is typed entirely from them, so there is no generated code to review by hand.
bunx --bun openapi-typescript "$document.new" --output "$types.new" >/dev/null

if [ "$check" = true ]; then
	status=0
	for pair in "$document" "$types"; do
		if ! diff -u "$pair" "$pair.new" >/dev/null 2>&1; then
			echo "The committed $pair is not what the current annotations produce." >&2
			diff -u "$pair" "$pair.new" >&2 || true
			status=1
		fi
	done
	rm -f "$document.new" "$types.new"
	exit "$status"
fi

mv "$document.new" "$document"
mv "$types.new" "$types"
echo "generated client written to $generated"
