# SPDX-FileCopyrightText: 2026 Afisharr contributors
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exercise the nightly merge gate through a local gh fixture."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check-nightly-status.sh")


class NightlyGate(unittest.TestCase):
    def check_gate(self, conclusion, *, event="workflow_dispatch", repair="42", waiver="", api_error=False):
        with tempfile.TemporaryDirectory() as directory:
            gh = Path(directory) / "gh"
            gh.write_text("#!/bin/sh\n"
                          'if [ "$FIXTURE_ERROR" = 1 ]; then echo unavailable >&2; exit 1; fi\n'
                          'case "$2" in */pulls/*) printf "%s" "$FIXTURE_WAIVER" ;;\n'
                          '*) printf "%s" "$FIXTURE_RUNS" ;; esac\n')
            gh.chmod(0o755)
            env = dict(os.environ, PATH=directory + os.pathsep + os.environ["PATH"],
                       GH_TOKEN="fixture", GITHUB_REPOSITORY="fixture/repository",
                       GITHUB_EVENT_NAME=event, REPAIR_PR_NUMBER=repair, PR_NUMBER="42",
                       FIXTURE_ERROR=str(int(api_error)), FIXTURE_WAIVER=waiver,
                       FIXTURE_RUNS=json.dumps({"workflow_runs": [{"conclusion": conclusion}]}))
            return subprocess.run(["bash", str(SCRIPT)], env=env, capture_output=True, text=True)

    def test_success_passes_repair(self):
        self.assertEqual(self.check_gate("success").returncode, 0)

    def test_failed_nightly_blocks_repaired_head(self):
        self.assertNotEqual(self.check_gate("failure").returncode, 0)

    def test_skipped_or_cancelled_nightly_blocks_repaired_head(self):
        for result in ("skipped", "neutral", "cancelled", "timed_out"):
            with self.subTest(result=result):
                self.assertNotEqual(self.check_gate(result).returncode, 0)

    def test_named_live_waiver_applies_to_repaired_head(self):
        self.assertEqual(self.check_gate("failure", waiver="Nightly-Waiver: fixture reason").returncode, 0)

    def test_empty_waiver_does_not_pass(self):
        self.assertNotEqual(self.check_gate("failure", waiver="Nightly-Waiver: ").returncode, 0)

    def test_ordinary_pr_still_blocks(self):
        self.assertNotEqual(self.check_gate("failure", event="pull_request", repair="").returncode, 0)

    def test_post_merge_push_does_not_require_a_pr(self):
        self.assertEqual(self.check_gate("failure", event="push", repair="").returncode, 0)

    def test_unreadable_result_blocks(self):
        self.assertNotEqual(self.check_gate("success", api_error=True).returncode, 0)


if __name__ == "__main__":
    unittest.main()
