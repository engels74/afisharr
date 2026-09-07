# CI and dependency maintenance

Every PR, merge queue commit, push to `main`, and manual CI dispatch runs
`.github/workflows/ci.yml`. Require `ci / required` from GitHub Actions with
strict up-to-date protection. The shared aggregate rejects failed, cancelled,
pending, missing, and skipped prerequisites. All checkout credentials are
removed; only the separate Biome publisher can update a Renovate branch.

## Development checks

The existing merge lanes remain: Rust formatting, Clippy with warnings denied,
nextest and doctests, SQLx offline-query regeneration, cargo-deny, cargo-machete,
the unsafe-code exception guard, file-size limits, Biome, interface rules,
Svelte type checking, Bun coverage, Chromium component tests, the static SPA
build, OpenAPI contract regeneration, and the nightly failure gate. Prek adds
repository hygiene without rerunning dedicated language checks in CI. Its local
commit and push hooks continue to use the same project commands.

Rust commands must run from `backend/` through rustup so the pinned
`rust-toolchain.toml` and SQLx configuration apply. Bun is pinned by
`frontend/.bun-version`; installs and Cargo resolution are frozen. The shared
Rust setup action respects the backend directory. Cargo helper versions are
explicit and maintained by Renovate. The frontend must build before backend
tests; `AFISHARR_REQUIRE_SPA=1` makes a missing embedded interface fail.

The nightly and release suites retain their existing invariant checks and
reference-client/live-Plex distinction. A failed, skipped, or cancelled nightly
blocks both ordinary PRs and explicitly dispatched repaired PR heads unless a
named `Nightly-Waiver` exists in the live PR description. Unreadable API results
fail. Eight local regression cases exercise these decisions:

```sh
python3 scripts/test-nightly-status.py
```

## Renovate

The versioned `engels74/automation` default and mixed-ecosystem presets provide
shared grouping and manager configuration. Actions use full version tags. The
repository-specific regex tracks versioned Cargo helpers in the local action.
TypeScript remains below 7 until Svelte compatibility is verified; SQLx CLI and
library changes require approval together with regenerated offline metadata.
Automerge remains off until the corrected shared policy and protection are
verified.

The official Biome versions manager keeps its schema aligned. The shared repair
workflow computes official migrations and safe formatting without write access,
then a separate publisher validates the result and dispatches full CI for the
exact repaired PR head. Config and source paths are explicitly scoped to the
frontend. Failed, stale, or incomplete dispatches cannot satisfy the merge gate.

## Coverage limits

Browser tests cover components, not a full deployed browser-to-server session.
`rust-miri` enforces the existing unsafe-code prohibition; it does not execute
Miri while no exceptions exist. Nightly/release placeholder suites still report
when their future-phase invariants have no tests; these messages are not coverage
claims. The existing policy permits a known-absent first nightly run. Release
verification against a real Plex server still requires the two existing
`AFISHARR_PLEX_CONTRACT_*` secrets and is not exercised by ordinary PR CI.
