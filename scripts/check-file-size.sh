#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# The script-checkable half of PRD §24.6 (D-047).
#
# Prints one line per file over its soft limit and exits non-zero if it printed
# anything. A file over its soft limit is split in the change that pushed it
# there, or the change description says in one sentence why not. A file over its
# hard limit is split, or it carries a `// STRUCTURE:` header comment naming the
# category, signed by a reviewer who is not the author.
#
# The `// STRUCTURE:` skip is the whole enforcement of that exception here. A
# comment is cheap to write; the control that makes it expensive is the reviewer
# who has to sign it, not this grep.
#
# This is a commit-stage hook on purpose. The point of the limit is to stop a
# file crossing it, and a check that runs after five commits reports a split
# that is now five commits deep.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

status=0

# Reports every file arriving on stdin that is over `limit` lines, worst first.
#
# Exempt by exclusion, never by raising a threshold (§24.6.4): generated output,
# build directories, and SQL migrations, which are append-only by construction.
report() {
	label="$1"
	limit="$2"

	offenders=""
	while IFS= read -r file; do
		[ -n "$file" ] || continue
		[ -f "$file" ] || continue
		# A signed hard-limit exception stops the file blocking every later
		# commit that touches it.
		if head -n 10 "$file" | grep -q '// STRUCTURE:'; then
			continue
		fi
		lines=$(wc -l <"$file" | tr -d ' ')
		if [ "$lines" -gt "$limit" ]; then
			offenders="${offenders}${lines} ${file}"$'\n'
		fi
	done

	if [ -n "$offenders" ]; then
		printf '%s over %s lines:\n' "$label" "$limit"
		printf '%s' "$offenders" | sort -rn | sed 's/^/  /'
		# `report` is the right-hand side of a pipeline, so it runs in a
		# subshell and cannot set a variable the caller reads. It reports
		# through its exit status instead.
		return 1
	fi
	return 0
}

find backend/crates -name '*.rs' \
	-not -path '*/target/*' \
	-not -path '*/migrations/*' \
	-not -path '*/tests/*' 2>/dev/null |
	sort | report 'Rust (non-test)' 400 || status=1

find backend/crates -path '*/tests/*' -name '*.rs' -not -path '*/target/*' 2>/dev/null |
	sort | report 'Rust (test)' 600 || status=1

find frontend/src -name '*.svelte' 2>/dev/null |
	sort | report 'Svelte components' 250 || status=1

find frontend/src \( -name '*.ts' -o -name '*.svelte.ts' \) \
	-not -path '*/.svelte-kit/*' \
	-not -path '*/generated/*' 2>/dev/null |
	sort | report 'TypeScript and rune modules' 300 || status=1

exit "$status"
