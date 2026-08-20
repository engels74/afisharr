# AGENTS.md

This file provides guidance to AI coding agents when working with code in this
repository.

Two independently tooled trees. `backend/` is the Cargo workspace root — not the
repo root. `frontend/` is a Bun + SvelteKit 2 + Svelte 5 SPA that is prerendered
and embedded into the Rust binary; there is no JavaScript server runtime.

## Running commands

Every `cargo` invocation must start inside `backend/`. `--manifest-path` is not
enough: rustup and cargo find `rust-toolchain.toml` and `.cargo/config.toml` by
walking *up* from the working directory, so a run from the repo root silently
gets the default toolchain and no `SQLX_OFFLINE`.

Frontend work uses `bun`, never `npm` or `npx`, in any lane or hook.

Backend, from `backend/`:

| Task | Command |
| --- | --- |
| format | `cargo fmt --all` (CI: `cargo fmt --check`) |
| lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| test | `cargo nextest run --workspace --all-features`, then `cargo test --doc --workspace` |
| one test | `cargo nextest run --workspace --all-features -E 'test(the_test_name)'` |
| one test file | same, with `-E 'binary(reprojection)'` — the file stem under `tests/` |
| supply chain | `cargo deny check` and `cargo machete` |

Frontend, from the repo root:

| Task | Command |
| --- | --- |
| install | `bun install --frozen-lockfile --cwd frontend` |
| build | `bun run --cwd frontend build` |
| lint / format | `bun run --cwd frontend lint` (Biome, check-only) / `… format` |
| typecheck | `bun run --cwd frontend check` |
| unit tests | `bun test --cwd frontend` |
| one unit test | `bun test --cwd frontend src/lib/utils.test.ts -t 'name'` |
| component tests | `bun run --cwd frontend test:browser` (Vitest + headless chromium; needs `bunx playwright install --with-deps chromium` run in `frontend/`) |
| interface rules | `bun run --cwd frontend lint:interface` |

Repo-root gates: `./scripts/check-file-size.sh`,
`./scripts/check-openapi-contract.sh`, `./scripts/dev-database.sh [--check]`.

## Generated files — never hand-edit

- `frontend/src/lib/api/generated/{openapi.json,schema.d.ts}` — regenerate with
  `./scripts/generate-openapi-client.sh` after any handler or DTO change and
  commit it in the same change. The backend's utoipa annotations are the sole
  contract; CI's `contract-check` fails on any diff.
- `backend/.sqlx/*.json` — the offline data the `sqlx::query!` macros check
  against. Regenerate with `./scripts/dev-database.sh` after changing a
  migration or a query (needs `sqlite3` and `sqlx-cli`). Stale metadata still
  compiles; only `--check` catches it.
- `frontend/build/` — untracked, but the binary embeds it at compile time. Run
  `bun run --cwd frontend build` before `cargo build` or `cargo nextest`, or the
  binary carries no interface and the `embedded_interface` tests skip. Set
  `AFISHARR_REQUIRE_SPA=1` to turn that skip into a failure.

## Backend invariants

- Every mutation goes through `afisharr_core::storage::WriteHandle` — one write
  actor on one write connection. Reads come from the pool (max 4). Do not open a
  second write connection or pass a pool where a `WriteHandle` is expected.
- A `WriteOperation` receives only a `&mut SqliteConnection` — no HTTP client,
  no filesystem root. A pass needing external I/O reads a snapshot, does the I/O
  outside, and commits in short operations at checkpoints (`I-DATA-2`).
- Handlers return `afisharr_api::error::AppError` on every failure path; it
  renders as the one `Problem` shape. Never hand-build a status/body tuple.
- Crate direction: `core` ← `plex` (← `sources`) ← `api` ← `afisharr` (the bin).
  `core` depends on none of the others; keep its domain logic I/O-free and take
  an injected `time::Clock` rather than reading the wall clock.
- `unsafe_code = "forbid"` workspace-wide. A `#![allow(unsafe_code)]` anywhere
  under `backend/crates` fails the `rust-miri` CI job by design — extend that
  lane to run Miri rather than deleting the attribute.
- Migrations live in `backend/crates/afisharr/migrations/` and are compiled in
  with `sqlx::migrate!`. Forward-only: the binary refuses to open a database
  whose applied version exceeds the newest migration it carries.
- The adversarial Plex fake is behind `afisharr-plex`'s `fake` feature. Reach it
  through dev-dependencies; a `#[cfg(test)]` module is invisible to other
  crates' test suites.

## Frontend invariants

Enforced by `bun run --cwd frontend lint:interface` over `src/routes` and
`src/lib/{features,components,shared}`. Exempt a single line with
`afisharr-lint-ignore: <rule> <reason>` on it or on the line above.

- No hard-coded user-facing string (`I-UX-7`). Resolve through `t`/`tn` from
  `$lib/shared/i18n` and add the key to `catalogue.en.ts`.
- No `fetch`, `XMLHttpRequest`, or `axios`. Use `api` from `$lib/api/client` and
  derive types from `ApiSchemas`, never a hand-declared interface.
- No display state read from `response.status` or `array.length` (`I-UX-2`). Use
  the `SurfaceState` union in `$lib/components/state/surface-state.ts`.

Two test lanes, split on the filename:

- `*.svelte.test.ts` renders a component and runs only under Vitest browser mode
  (`vitest.config.ts`). `bunfig.toml` makes `bun test` skip the suffix.
- Every other `*.test.ts` runs under `bun test` with happy-dom. Rune modules
  (`x.svelte.ts`) belong here — `test-setup.ts` compiles runes for Bun.

`bunfig.toml` sets coverage thresholds on the `bun test` lane (85% lines, 90%
functions, 80% statements), so an untested new module fails it with no test
failing. Only `src/lib/api/generated/**` and `scripts/**` are exempt.

In `bun test` files import relatively (`../../shared/i18n`), not via `$lib`:
that alias comes from `.svelte-kit/tsconfig.json`, which `bun test` never reads.

Colours in `uno.config.ts` are `var(--token)` references to `src/app.css`; a
literal colour there is a second palette. The three faces are self-hosted in
`static/fonts/` — do not add `presetWebFonts`.

## File size limits

`scripts/check-file-size.sh` runs at commit stage: Rust non-test 400 lines, Rust
test 600, `.svelte` 250, `.ts` and rune modules 300. Split the file, or put a
`// STRUCTURE: <category>` comment in its first 10 lines — which a reviewer who
is not the author has to sign.

## Commits

Conventional Commits, and `git commit -s`: the DCO sign-off is checked at
commit-msg stage. Branch and open a PR — `no-commit-to-branch` blocks `main`.
Run `prek install` once to wire the pre-commit, commit-msg, and pre-push shims.
When a red nightly blocks the merge lane, fix it or add a
`Nightly-Waiver: <reason>` line to the PR body and re-run the job.

## Reference

- `.agents/rules/backend-rust-dev-pro.md` — Rust 1.97 / edition 2024 idiom,
  errors, ownership, axum/sqlx/tokio conventions. Read before writing new Rust.
- `.agents/rules/frontend-dev-pro.md` — Bun, Svelte 5 runes, SvelteKit 2, UnoCSS
  presetWind4, shadcn-svelte. Read before writing new components or SvelteKit
  boilerplate.
- `prek.toml` — every local gate, each with the reason it exists. Read when a
  hook blocks a commit.
- `.github/workflows/merge.yml` — the lanes a PR must pass and their exact
  commands; `nightly.yml` and `release.yml` hold the slower suites.
- `docs/afisharr_prd.md` (10.5k lines) and `docs/afisharr_implementation_plan.md`
  (3.4k lines) — the specification that code comments cite as `§N.N` and
  `D-0NN`. Grep for the identifier a comment names; never read either in full.
