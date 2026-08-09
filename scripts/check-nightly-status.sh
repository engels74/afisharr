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
# The waiver therefore lives on a pull request, and this gate decides only where
# one exists to carry it: the pull request's own check, and the merge queue run
# that merges it. `github.event.pull_request.body` is populated for the first
# and empty for the second, so the merge queue resolves the body from the
# reference naming the queued pull request — otherwise a waived pull request
# passes its check and is blocked again on the way in. A push to `main` is after
# that decision, and the release lane runs the whole nightly suite itself rather
# than reading its last result, so neither gates.
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

case "${GITHUB_EVENT_NAME:-}" in
pull_request)
	body="${PR_BODY:-}"
	;;
merge_group)
	# refs/heads/gh-readonly-queue/<base branch>/pr-<number>-<sha>
	queued=$(printf '%s' "${MERGE_GROUP_HEAD_REF:-}" |
		sed -n 's|.*/pr-\([0-9][0-9]*\)-[0-9a-f]*$|\1|p')
	if [ -z "$queued" ]; then
		echo "The last nightly run on main concluded '$last_conclusion'." >&2
		echo "No pull request could be read from '${MERGE_GROUP_HEAD_REF:-}', so a" >&2
		echo "waiver on it is unobservable rather than absent." >&2
		exit 1
	fi
	if ! body=$(gh api "repos/$repo/pulls/$queued" --jq '.body // ""'); then
		echo "The last nightly run on main concluded '$last_conclusion'." >&2
		echo "Could not read pull request #$queued's body, so a waiver on it is" >&2
		echo "unobservable rather than absent." >&2
		exit 1
	fi
	;;
push | workflow_dispatch)
	# A push to `main` is after the merge the gate decides, and the release
	# lane — which reaches this through `workflow_call`, so the event name is
	# the caller's — runs the whole nightly suite itself rather than reading
	# its last result.
	echo "The last nightly run on main concluded '$last_conclusion'."
	echo "A '$GITHUB_EVENT_NAME' run is not a merge to main; nothing to gate."
	exit 0
	;;
*)
	# A new trigger reaches this with no decision made about it. Passing would
	# un-gate D-035 silently, which is the failure this script exists to stop.
	echo "The last nightly run on main concluded '$last_conclusion'." >&2
	echo "A '${GITHUB_EVENT_NAME:-unknown}' run has no waiver source, so whether" >&2
	echo "this merge is waived is unobservable. Decide it here, in D-035's terms." >&2
	exit 1
	;;
esac

reason=$(printf '%s' "$body" | sed -n 's/^Nightly-Waiver:[[:space:]]*//p' | head -n 1)

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
