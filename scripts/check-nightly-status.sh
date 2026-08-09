#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# A nightly failure blocks the next merge to `main` until it is fixed or
# explicitly waived with a named reason (D-035, PRD §21.10.1). Without this the
# nightly lane becomes a wall of red nobody reads, and the invariants that only
# run there stop being invariants.
#
# The waiver is a line in the pull request body:
#
#     Nightly-Waiver: <reason>
#
# A reason is required and lands in the permanent record of the merge, which is
# the point: a waiver that costs nothing gets used for everything.
#
# Three states, and only the first two are inputs to a decision (P1). A nightly
# that *failed* blocks. A nightly that has never run — no workflow on `main`
# yet, or no completed run — is known-absent and blocks nothing. A result this
# script cannot read is unobservable, and it fails rather than reporting the
# absence of a failure it could not have seen.

set -uo pipefail

if [ -z "${GH_TOKEN:-}" ]; then
	echo "GH_TOKEN is unset; the nightly lane's last result is unobservable." >&2
	exit 1
fi

repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is unset}"
endpoint="repos/$repo/actions/workflows/nightly.yml/runs?branch=main&status=completed&per_page=1"

if ! response=$(gh api "$endpoint" 2>&1); then
	case "$response" in
	*'"status":"404"'* | *'Not Found'*)
		echo "No nightly workflow has run on main yet. Nothing to block on."
		exit 0
		;;
	*)
		echo "Could not read the nightly lane's last result:" >&2
		printf '%s\n' "$response" >&2
		exit 1
		;;
	esac
fi

last_conclusion=$(printf '%s' "$response" | jq -r '.workflow_runs[0].conclusion // "none"')

case "$last_conclusion" in
none)
	echo "The nightly lane has no completed run on main. Nothing to block on."
	exit 0
	;;
success | neutral | skipped)
	echo "The last nightly run on main concluded '$last_conclusion'."
	exit 0
	;;
esac

reason=$(printf '%s' "${PR_BODY:-}" | sed -n 's/^Nightly-Waiver:[[:space:]]*//p' | head -n 1)

if [ -n "$reason" ]; then
	echo "The last nightly run on main concluded '$last_conclusion'."
	echo "Waived: $reason"
	exit 0
fi

cat >&2 <<EOF
The last nightly run on main concluded '$last_conclusion'.

Fix it, or waive it with a named reason by adding a line to this pull request's
description:

    Nightly-Waiver: <why this merge should not wait for the nightly fix>

Recorded as D-035.
EOF
exit 1
