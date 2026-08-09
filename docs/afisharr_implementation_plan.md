# Afisharr — Implementation Plan

Fifteen phases in dependency order, plus one parallel spike track. Each phase breaks into tasks, and
each task into subtasks that say what to build, where it lands, and how to check it is done.

This plan is the companion to the PRD. The PRD says what to build and why; this document says in what
order, and how each step proves itself. Where the two disagree, the PRD wins.

**This plan carries no dates and no capacity figures.** Not because they were forgotten, but because
they would be fabrications: nothing here has been built, and a schedule derived from a guess about
throughput is a guess wearing a calendar. What the plan carries instead is dependency order, a
relative size per phase, and an exit criterion per phase that a build can fail on. Multiply by
whatever capacity turns out to be real. Recorded as D-039. Do not add dates.

---

## How to read this

**Before you write code for a task, read the rule file for the surface it touches.** Two files in
the repository carry the stack-level coding guidelines, and both are normative:

| File | Read it before touching |
| --- | --- |
| `.augment/rules/backend-rust-dev-pro.md` | Any Rust: `backend/crates/**` — tokio, axum, SQLx, serde, errors, concurrency |
| `.augment/rules/frontend-dev-pro.md` | Any frontend: `frontend/**` — Bun, Svelte 5 runes, SvelteKit 2, UnoCSS `presetWind4`, shadcn-svelte |

This is a read-first obligation, not a review-time check, and it binds every author, human or agent.
Both stacks compile the previous generation's idiom without complaint, so the gates in §A.1 stay
green while the code is wrong. PRD §24.1 states the rule, §0.2 states its authority against §24, and
D-048 records why. §A.2 and §A.3 open with the confirmation line.

**Exit criteria are invariants, not opinions.** Every phase names the invariants from *Invariants* in
the PRD that must pass before it is done. All 97 are assigned exactly once (*Invariant coverage*), so no invariant is
orphaned and none is claimed twice. A phase whose invariants pass is finished; one whose invariants
do not is not, however complete it looks.

**Sizes are relative.** `S` `M` `L` `XL`, comparing phases to each other and to nothing else. `XL`
means "this is where the work is", not a number of days.

**Each phase names what it excludes.** Scope creep in this project would come from a phase quietly
absorbing the next one, so the exclusions are written down.

**Interface work is distributed, never deferred.** A plan that stacks fifteen pages into a final
phase discovers its interface problems last. Each phase ships the pages for the capability it adds,
against the shell built in Phase 1.

**A task is done when its check passes.** Every task ends with a **Done when** clause that is a named
invariant, a command that exits zero, or a specific observable behaviour. "Looks correct" is not a
completion criterion anywhere in this plan.

**The boxes record that check. They never replace it.** Every subtask carries a `- [ ]`, and so does
every **Done when** clause. Tick a subtask when it is built. Tick the **Done when** box only when the
clause is independently true — the command was run and exited zero, or the named invariant's test
passes. Nothing is ticked on the strength of having done the steps, and a ticked box beside a failing
gate is a documentation bug rather than a finished task: the gate wins, and the box is wrong.
Recorded as D-049.

**A phase carries no box.** A phase is finished when every task box inside it is ticked *and* its exit
invariants pass. The box on its last task is not a claim about the phase, and no summary of the boxes
below is kept anywhere — a hand-maintained summary is believed over the thing it summarises the first
time the two disagree.

**Two kinds of checkbox live in this document, and only one of them is ever ticked here.** The boxes
in the phase bodies are the progress ledger: they are marked in place as work lands. The boxes in
Appendix A (§A.2–§A.4) are a template — one master copy, worked through fresh by every author and
every reviewer on every task, and never marked in this file. A `- [x]` in Appendix A means somebody
ticked the template instead of their own copy.

**Modular structure is a constraint on every task, from Task 0.1 onward.** The source tree divides
into subfolders by domain; every file states one thing; no module collects unrelated
responsibilities; every file carries a soft and a hard size limit; every module exposes a narrow
public surface it declares in one place. PRD §24.6 states the rule and D-047 records why. It is
gated per change (§A.1) and checked per task (§A.2, §A.3, §A.4). It is not a refactor to schedule
after a phase, because by then every later phase is written against the structure it would change.

**Every task is also subject to the build gates.** Appendix A carries the coding standards as
checkable gates. A task that passes its **Done when** clause but fails a gate in Appendix A is not
done.

### Where the code lands

The repository holds two surfaces, one directory each, and nothing of either at the root.

```
backend/           the Cargo workspace, and everything scoped to it:
                   Cargo.toml, Cargo.lock, rust-toolchain.toml, rustfmt.toml,
                   clippy.toml, deny.toml, .cargo/config.toml, .sqlx/
  crates/core      domain model, definition schema, reconciliation engine,
                   filter/order pipeline, lifecycle state machine, scheduling
  crates/sources   one module per provider behind a common SourceBuilder trait;
                   each with typed client, rate limiter, circuit breaker,
                   response validation, health status
  crates/plex      Plex client: libraries, collections, hubs, labels,
                   media streams, artwork, filter-metadata discovery
  crates/render    poster and overlay renderer, element model, layers,
                   font handling, content-addressed cache
  crates/packs     pack format (manifest, definitions, assets) and installer
  crates/api       Axum routes, auth and sessions, SSE, OpenAPI
  crates/afisharr  binary: wiring, embedded SPA, migrations, CLI entrypoints
frontend/          SvelteKit 2, Svelte 5, UnoCSS (presetWind4), shadcn-svelte,
                   adapter-static, Bun tooling; .bun-version pins Bun
scripts/           the gates that span both surfaces, so they belong to neither
docs/              this plan and the PRD
.github/           the merge, nightly, and release lanes (§A.5)
```

`prek.toml`, `.gitignore`, and `LICENSE` stay at the root with `scripts/`.

Rust toolchain pinned at 1.97.1, edition 2024.

**Every cargo command runs from `backend/`.** Cargo discovers `.cargo/config.toml` and rustup
discovers `rust-toolchain.toml` by walking *up* from the working directory; neither ever descends. A
cargo step left at the repository root gets the default toolchain and no `[env]` block, and says
nothing about it. The CI lanes set `working-directory: backend`, the prek hooks `cd backend` first,
and `scripts/dev-database.sh` does the same before it calls `cargo sqlx prepare`. The offline query
data needs no such care: sqlx resolves `.sqlx/` through `cargo metadata` → `workspace_root`, which
follows the manifest rather than the working directory.

**Inside a crate, the division continues by domain.** Each `src/` holds subfolders named after a
thing the product has or a job it does — `placement/`, `lifecycle/`, `sources/trakt/`,
`definition/validation/` — never a flat `src/` of siblings, and never a folder named after a layer
(`utils/`, `helpers/`, `common/`, `types/`, `models/`). Code shared across domains is not banned —
it goes in `backend/crates/core/src/<named>/` or `frontend/src/lib/shared/<named>/` under a name that
predicts what it does (`text/slug.rs`, not `utils.rs`), and is itself a domain (§24.6.1). Frontend
domain code lives in `frontend/src/lib/features/<domain>/`, holding that domain's components,
`.svelte.ts` state, and calls to the generated client together; `frontend/src/lib/components/ui/`
stays the shared primitive layer. Where a domain spans crates, the folder name matches on both sides:
`backend/crates/core/src/placement/` and `backend/crates/api/src/routes/placement/`. Rule in PRD
§24.6.1.

---

## Sequencing, and why it is this order

Foundations first, with the spikes as a parallel track from Phase 2. Recorded as D-039.

*Why foundations first, given placement is the riskiest subsystem:* the foundations here are
unusually low-risk. The schema is fully specified and its DDL was already executed against SQLite
3.45; the definition engine and the registries are specified to the field. Building them is
execution, not discovery. The discovery in this project is concentrated almost entirely in placement,
and it is drained by two spikes rather than by building placement early.

*Why the spikes run in parallel rather than first:* Q-014 and Q-015 need a real Plex server and a
Plex client. They need nothing else — no schema, no engine, no collections. So they can start the
moment Phase 2 exists and run alongside everything through Phase 6, at almost no cost to the main
line. What they must not do is finish after Phase 7 starts, because Phase 7 designed against an
assumed answer is Phase 7 built twice.

*Why the fake is Phase 2 rather than later:* every phase from Phase 4 onward tests against it
(D-036). A fake introduced halfway through means every test written before it gets rewritten.

```
main line   P0 ─ P1 ─ P2 ─ P3 ─ P4 ─ P5 ─ P6 ─────── P7 ─ P8 ─ P9 ─ P10 ─ P11 ─ P12 ─ P13 ─ P14
                      │                              ▲
spike track           └── Q-015 ── Q-014 ────────────┘  must land before P7 starts
```

| Phase | Name | Size |
| --- | --- | --- |
| 0 | Skeleton, schema, and the gates | `L` |
| 1 | HTTP surface, security spine, and the interface shell | `L` |
| 2 | Plex client and the adversarial fake | `L` |
| — | Spike track — Q-015 then Q-014 | `M` |
| 3 | Definition engine and registries | `L` |
| 4 | Library cache and identity | `L` |
| 5 | Sources and the reconciliation pipeline | `XL` |
| 6 | Collections in Plex | `M` |
| 7 | Placement | `XL` |
| 8 | Rendering | `XL` |
| 9 | Lifecycle | `XL` |
| 10 | Acquisition | `M` |
| 11 | Teardown | `L` |
| 12 | Backup, restore, and upgrade | `M` |
| 13 | Onboarding, packs, doctor, observability | `L` |
| 14 | Release engineering | `M` |

---
## Phase 0 — Skeleton, schema, and the gates

**Size:** `L`. The workspace, the database, and the machinery that will fail every later build honestly.

**Exit invariants:** `I-DATA-2`, `I-DATA-3`, `I-DATA-5`, `I-DATA-6`, `I-DATA-7`, `I-DATA-8`, `I-DATA-10`, `I-DATA-11`.

**Not here:** any Plex call, any HTTP route beyond health, any interface.

**Why the whole schema now, rather than table-by-table:** `I-DATA-5` and `I-DATA-10` are properties of
the schema's *shape* — per-user targeting needing no migration, one subject per identity enforced by
a unique index. Both are cheap now and are table rebuilds later.

### Task 0.1 Workspace, prek, and the CI lanes
- **Build:** the Cargo workspace under `backend/` and the `frontend/` project, pre-commit hooks
  installed, and CI running the three test lanes recorded as D-035 — merge, nightly, release.
- **Where:** repository root; `backend/` and its `crates/core`, `crates/sources`, `crates/plex`,
  `crates/render`, `crates/packs`, `crates/api`, `crates/afisharr`; `frontend/`.
- **Subtasks:**
  - [x] 1. Scaffold the seven workspace crates under `backend/`, with `backend/crates/afisharr` as the
     binary depending on the rest.
  - [x] 2. Scaffold `frontend/` as a SvelteKit 2 / Svelte 5 project with UnoCSS (`presetWind4`), shadcn-svelte,
     `adapter-static`, and Bun tooling.
  - [x] 3. Install prek and commit its hook configuration.
  - [x] 4. Wire the merge lane: every table-driven and unit-level invariant, budgeted at 10 minutes.
  - [x] 5. Wire the nightly-lane and release-lane job shapes now, even though they have nothing but
     placeholder checks to run — later phases add jobs to an existing lane rather than build one.
  - [x] 6. Wire the rule that a nightly failure blocks the next merge until fixed or explicitly waived with
     a named reason.
  - [x] 7. Add the structure gate from §A.1 as a prek hook and a merge-lane step, with the exempt paths
     (generated client, `target/`, `.svelte-kit/`, migrations, registry constant tables) excluded in
     the script rather than by raising a threshold. It exists from the first commit because a limit
     introduced in Phase 6 is a limit that first fires as a backlog (§24.6.4, D-047).
- [x] **Done when:** `prek install` leaves hooks active in a fresh clone, and a trivial commit runs the
  merge lane to a pass inside the 10-minute budget, with the nightly and release lane jobs present and
  schedulable. The structure gate runs in that lane and passes on the empty tree.

### Task 0.2 The complete schema
- **Build:** all 68 tables and their indexes, exactly as specified, as SQLx migrations.
- **Where:** `backend/crates/afisharr/migrations/`; `backend/crates/core` (row types, projection functions).
- **Subtasks:**
  - [x] 1. Write migration `0001`, setting `auto_vacuum = INCREMENTAL`, `journal_mode = WAL`, and
     `page_size = 8192` before the first `CREATE TABLE` — these are one-way doors that cannot be set
     after the first write.
  - [x] 2. Create every `STRICT` table and every index in the schema, including the four tables that store
     the volatile-parameter feed and bulk-dataset machinery Phase 5 will use.
  - [x] 3. Seed the three fixed-ULID principal rows: `Everyone`, `Owner`, `SharedAll`.
  - [x] 4. Implement the SQLx migration runner in the binary's startup path.
  - [x] 5. Implement the per-table derived-column projection functions (one function per table, per the
     derived-column rule) and the `afisharr db reproject` command.
  - [x] 6. Write the reprojection test: for every row in the database, `project(body_json)` equals the
     stored derived columns.
- [x] **Done when:** `sqlx migrate run` against a fresh database creates all 68 tables with zero errors;
  `I-DATA-5` passes — inserting a `PlexUser` or `LocalUser` principal row and a `placement_visibility`
  row referencing it requires no `ALTER TABLE`; `I-DATA-10` passes — a second whole-title lifecycle
  subject for the same identity is rejected by its unique index; and `I-DATA-6` passes —
  `afisharr db reproject` is a no-op against a populated database.

### Task 0.3 Startup sequence
- **Build:** the sequence that runs on every boot — downgrade refusal, automatic pre-migration backup,
  and post-migration integrity verification.
- **Where:** `backend/crates/afisharr` (startup path); `backend/crates/core` (backup and integrity checks).
- **Subtasks:**
  - [x] 1. On open, read the applied-migration table; if it names a version this binary does not know,
     refuse to start with a message naming both the found version and the binary's newest known one.
  - [x] 2. Before running a pending migration, copy the database using SQLite's online backup API — never a
     file copy — to `backups/pre-migration-<version>-<timestamp>.db`, retaining the last three.
  - [x] 3. Run pending migrations, then run `PRAGMA foreign_key_check` and `PRAGMA integrity_check`.
  - [x] 4. Reconcile unconfirmed lifecycle intents and expired leases as the final startup step.
  - [x] 5. Clear leases whose owner names this process instance on startup, before touching anything else.
- [x] **Done when:** `I-DATA-7` and `I-DATA-8` pass — a binary pointed at a database with a newer applied
  migration than it knows refuses to start rather than proceeding; a pending migration produces a
  backup file via the online backup API before it runs; and `foreign_key_check` plus `integrity_check`
  both return clean on first start after a migration.

### Task 0.4 Concurrency
- **Build:** WAL mode, a single write actor, per-pass leases, and the rule that no transaction spans
  network or filesystem I/O.
- **Where:** `backend/crates/core` (connection pool, write actor, lease acquisition).
- **Subtasks:**
  - [x] 1. Set the connection pragmas on every pooled connection: `foreign_keys = ON`,
     `busy_timeout = 5000`, `synchronous = NORMAL`, `cache_size = -32000`, `temp_store = MEMORY`.
  - [x] 2. Build a read pool of `min(4, cores)` connections and exactly one writer connection owned by a
     write actor; route every mutation through it as a message, never a second write path.
  - [x] 3. Implement the `leases` table and its conditional insert-or-update acquisition (steal only an
     expired lease).
  - [x] 4. Enforce hierarchical lease names (`pass:collection:<id>`, `pass:placement:<library_id>`,
     `pass:lifecycle:<library_id>`, `job:<job_id>`) and heartbeat-driven abort on lease loss.
  - [x] 5. Structure every long-running pass as: snapshot read, do the I/O, commit in short transactions at
     defined checkpoints — never one transaction wrapping the I/O.
- [x] **Done when:** `I-DATA-2` and `I-DATA-3` pass — two logical passes racing for the same lease name
  never both proceed, the write actor is the only code path that opens a write connection, and no test
  or lint finds a transaction held open across an HTTP call or a library-root filesystem write.

### Task 0.5 Structured logging and configuration
- **Build:** the application log, the single-row `settings` table with history, and instance identity.
- **Where:** `backend/crates/afisharr` (log init, config loading); `backend/crates/core` (settings, `instance` table).
- **Subtasks:**
  - [x] 1. Implement structured logging to `logs/afisharr.log`, rotated, distinct from the database-backed
     run-event log the GUI reads.
  - [x] 2. Implement the `instance` table, generating `client_identifier` once on first start and never
     regenerating it.
  - [x] 3. Implement `settings` and `settings_history` as one JSON body deserialised into a typed struct with
     unknown-field rejection, never a key-value table.
  - [x] 4. Implement config loading (environment, file) that populates `settings` on first start.
- [x] **Done when:** a fresh instance boots, writes one `instance` row with a `client_identifier` that
  survives a restart unchanged, and every settings write lands as one versioned body in
  `settings_history` rather than a partial key-value update.

### Task 0.6 The secret table and key handling
- **Build:** encrypted credential storage per D-032, isolated from `settings`.
- **Where:** `backend/crates/core` (secrets table, cipher); `backend/crates/afisharr` (key file lifecycle).
- **Subtasks:**
  - [x] 1. Implement the `secrets` table: ciphertext, nonce, algorithm, per-secret.
  - [x] 2. Implement XChaCha20-Poly1305 encryption with a random nonce per secret.
  - [x] 3. Generate a 32-byte key from the OS CSPRNG on first start, store it beside the database at
     `secrets.key` with mode `0600`, and support the `AFISHARR_SECRET_KEY` override.
  - [x] 4. Verify no secret value can reach `settings.body_json` or a `settings_history` diff.
- [x] **Done when:** a secret written through the table round-trips through encrypt and decrypt; the key
  file is created with mode `0600` on first start; `AFISHARR_SECRET_KEY` overrides it when set; and
  `I-DATA-11` passes — a database copied without `secrets.key` cannot decrypt any stored secret, and no
  secret value appears anywhere in `settings` or its history.

---

## Phase 1 — HTTP surface, security spine, and the interface shell

**Size:** `L`. Everything reachable, and everything that makes reachable safe. D-029 assumes the
instance is on the internet, so this is not hardening added later.

**Exit invariants:** `I-SEC-1`, `I-SEC-2`, `I-SEC-3`, `I-SEC-8`, `I-UX-1`, `I-UX-2`, `I-UX-3`,
`I-UX-7`, `I-UX-9`.

**Not here:** any page with real data behind it.

**Why i18n now:** `I-UX-7` forbids hard-coded user-facing strings. Extraction added after fifteen pages
exist is a mechanical sweep over every one of them; added now it is a habit.

**Why the bootstrap claim is here rather than with the wizard in Phase 13:** the first-run
admin-account page ships in this phase (Task 1.11), and the moment it exists on a reachable instance
it is an unauthenticated grant of administrator. The gate has to arrive with the page it guards, not
with the wizard built around it eleven phases later. Task 1.12 builds the mechanism; Phase 13 builds
the eight-step journey on top of it.

### Task 1.1 Axum routing and error model
- **Build:** the HTTP surface's spine — routing, a structured error model, and the health route.
- **Where:** `backend/crates/api`.
- **Subtasks:**
  - [ ] 1. Stand up Axum with a router that groups routes by the six primary destinations plus settings.
  - [ ] 2. Define one structured error type carrying a JSON pointer, expected-versus-actual, and an HTTP
     status mapping, so every error surface downstream reuses it rather than inventing shapes.
  - [ ] 3. Implement the health route with no authentication requirement.
  - [ ] 4. Wire OpenAPI generation via utoipa and the generated TypeScript client build step.
- [ ] **Done when:** the health route returns 200 with no credentials; every other route returns the one
  structured error shape on failure; and the generated TypeScript client builds from the OpenAPI
  document with zero manual edits.

### Task 1.2 Local and Plex authentication
- **Build:** local password authentication and the Plex PIN/OAuth login flow.
- **Where:** `backend/crates/api` (routes); `backend/crates/core` (`users`, `plex_pin_logins`); `backend/crates/plex` (the
  plex.tv PIN and OAuth calls).
- **Subtasks:**
  - [ ] 1. Implement local login: Argon2id password hashing at PHC-string parameters tuned to roughly 250 ms
     on the reference machine.
  - [ ] 2. Implement the first-run rule: no default credentials, nothing reachable until the admin account
     exists, and no admin account creatable without the setup claim built in Task 1.12.
  - [ ] 3. Implement the Plex PIN login flow: create a pin resource, present the code, poll until a token
     appears or the pin expires, checking `client_identifier` against the instance value.
  - [ ] 4. Implement the Plex OAuth variant of the same flow, sharing the polling machinery.
  - [ ] 5. Store the returned token in `secrets`, never in `plex_pin_logins`.
- [ ] **Done when:** a fresh instance rejects every route except health and the claim endpoint, and
  rejects first-run admin creation itself without an active claim; a completed PIN or OAuth flow
  produces a working session; and a pin issued under a mismatched `client_identifier` fails visibly
  rather than yielding a token that silently does not work.

### Task 1.3 Sessions and API keys
- **Build:** session lifecycle and revocable, scoped API keys.
- **Where:** `backend/crates/api`; `backend/crates/core` (`sessions`, `api_keys`).
- **Subtasks:**
  - [ ] 1. Store sessions by the SHA-256 of the cookie value, never the value itself; set `Secure`,
     `HttpOnly`, `SameSite=Lax`.
  - [ ] 2. Implement a 7-day idle timeout sliding on `last_seen_at` and a 30-day absolute timeout with no
     extension.
  - [ ] 3. Rotate the session id on privilege change and on password change; revoke every session for a user
     on password change, with individual revocation available from Settings.
  - [ ] 4. Implement API keys hashed at rest, individually revocable, showing the plaintext once at creation
     and a last-used timestamp thereafter.
- [ ] **Done when:** a database read never yields a working session id or API key in plaintext; a password
  change revokes every existing session for that user; and a revoked API key is rejected on its next
  use.

### Task 1.4 Rate limiting with the trusted-proxy list
- **Build:** per-IP and per-account rate limiting that cannot be defeated by a forged forwarded header.
- **Where:** `backend/crates/api`.
- **Subtasks:**
  - [ ] 1. Implement the limit table: login per account (5 failures / 15 min, exponential lockout to 24 h),
     login per IP (20 failures / 15 min), authenticated API (600 req/min), provider-calling endpoints
     (60 req/min).
  - [ ] 2. Implement `trustProxy` as a list of trusted proxy addresses or CIDR ranges, never a boolean.
  - [ ] 3. Honour `X-Forwarded-For` only when the immediate peer is in that list; use the peer address
     otherwise.
  - [ ] 4. Return 429 with `Retry-After` on exceed.
- [ ] **Done when:** `I-SEC-1` passes — a request carrying a forged `X-Forwarded-For` from a peer outside
  the trusted list is rate-limited against its real address, not the forged one, and the same request
  from a trusted proxy honours the forwarded value.

### Task 1.5 Security headers and CSRF
- **Build:** the fixed response-header set and always-on CSRF protection.
- **Where:** `backend/crates/api` (middleware).
- **Subtasks:**
  - [ ] 1. Apply `Strict-Transport-Security`, `Content-Security-Policy` (`default-src 'self'`, no inline
     script, `frame-ancestors 'none'`), `X-Content-Type-Options: nosniff`, `Referrer-Policy:
     no-referrer`, and a `Permissions-Policy` denying camera, microphone, and geolocation — as
     middleware, not per-handler.
  - [ ] 2. Emit `Strict-Transport-Security` only over HTTPS, trusting the forwarded-protocol header only from
     an address on the trusted-proxy list.
  - [ ] 3. Implement CSRF protection with no toggle, per D-002-class decisions on this surface.
- [ ] **Done when:** `I-SEC-2` passes — every route, including ones added after this task, carries the full
  header set on every response, verified by a test that asserts headers at the middleware layer rather
  than per-route.

### Task 1.6 The filesystem browser and its root jail
- **Build:** the jailed browser used by the asset picker and placeholder roots.
- **Where:** `backend/crates/api` (routes); `backend/crates/core` (path canonicalisation and containment check).
- **Subtasks:**
  - [ ] 1. Canonicalise every requested path and resolve symbolic links before checking containment within
     an enabled root — never before.
  - [ ] 2. Refuse traversal sequences, absolute paths, and links resolving outside a root with one message
     naming the root, never the resolved path.
  - [ ] 3. Apply the same containment rule to placeholder writes against `placeholderRoots`.
- [ ] **Done when:** `I-SEC-3` passes — traversal sequences, absolute paths, and symlinks pointing outside
  a configured root are all refused with the root named in the message; `I-SEC-4`'s placeholder-write
  path reuses the identical containment function rather than a second implementation.

### Task 1.7 The SvelteKit shell, embedded in the binary
- **Build:** the prerendered SPA embedded into the release binary, with no JavaScript server runtime in
  production.
- **Where:** `frontend/`; `backend/crates/afisharr` (embedding).
- **Subtasks:**
  - [ ] 1. Configure `adapter-static` prerendering with no server load functions, form actions, server
     hooks, or server-side database access — these are structurally unavailable on this stack.
  - [ ] 2. Embed the built SPA into the `afisharr` binary so the release artefact is one file.
  - [ ] 3. Wire the generated OpenAPI TypeScript client as the SPA's only data-access path.
- [ ] **Done when:** the release binary serves the SPA with no separate static-file directory required at
  runtime, and every SPA data call goes through the generated typed client.

### Task 1.8 The nine-state component vocabulary
- **Build:** one shared component per interface state, so no page invents its own flattening of what
  the API already distinguishes.
- **Where:** `frontend/` (shared component library).
- **Subtasks:**
  - [ ] 1. Build one component each for Loading, Empty, Error, Frozen, Degraded, Stale, Pending, Blocked,
     and Non-convergent, each independently importable.
  - [ ] 2. Implement the Loading sub-policy: nothing under 300 ms, skeleton from 300 ms to ~3 s, progress
     text beyond ~3 s, SSE progress for long operations, partial data rendering as it arrives.
  - [ ] 3. Implement the three Empty sub-kinds (nothing created yet, nothing matched, nothing yet but
     pending) as distinct treatments, never conflated with a failed fetch.
  - [ ] 4. Implement destructive-action affordances: preview with named counts, confirmation proportional to
     consequence, an afterward report — never a default, focused, or single-step destructive action.
  - [ ] 5. Add a lint rule that fails the build if a page branches on an HTTP status code to choose a display
     state instead of reading the state the API returned alongside the data.
- [ ] **Done when:** `I-UX-1`, `I-UX-2`, and `I-UX-3` pass — every data-bearing page renders one of the nine
  states from what the API returned, no page infers a state from response shape or timing, and the
  lint rule catches an attempt to do so.

### Task 1.9 SSE transport
- **Build:** the single multiplexed SSE connection used for job progress and source health, established
  after auth.
- **Where:** `backend/crates/api` (SSE endpoint); `frontend/` (client, reconnection logic).
- **Subtasks:**
  - [ ] 1. Implement one connection per client, multiplexed by topic, established after authentication.
  - [ ] 2. Implement backoff reconnection; on reconnect, refetch state rather than replay missed events.
  - [ ] 3. Implement a small, non-modal disconnection indicator distinct from every other state.
  - [ ] 4. Ensure every SSE-fed surface is correct after a plain page load with no stream connected at all —
     the stream accelerates, it is never the only source of truth.
- [ ] **Done when:** `I-UX-9` passes — with SSE blocked, every surface the stream feeds still renders
  correct data on load and the disconnection is visible; a client that misses events during a
  disconnect and then reconnects ends up identical to a client that loaded the page fresh, verified by
  a reconnect-and-refetch test; the disconnection indicator appears within one missed heartbeat.

### Task 1.10 The i18n catalogue and extraction
- **Build:** the message-catalogue framework, shipping English, with interpolation and plural rules.
- **Where:** `frontend/` (catalogue, lint rule); `backend/crates/core` (locale as a data concept for formatters).
- **Subtasks:**
  - [ ] 1. Wire a message-catalogue library with interpolation and plural-rule support.
  - [ ] 2. Add a lint rule, active from the first commit, that fails on a hard-coded user-facing string.
  - [ ] 3. Thread locale through as a first-class setting the way the formatter registry will expect it in
     Phase 3.
- [ ] **Done when:** `I-UX-7` passes — the lint rule rejects a hard-coded user-facing string at commit time,
  and English ships as a complete catalogue with no untranslated-key fallback needed anywhere in the
  shell.

### Task 1.11 Interface pages this phase ships
- **Build:** the navigation shell — six primary destinations plus Settings — as routed pages, and the
  first-run and login flows that make the shell reachable.
- **Where:** `frontend/src/routes`.
- **Subtasks:**
  - [ ] 1. Route Dashboard, Collections, Design, Home Screen, Lifecycle, and Doctor, plus a Settings area
     with its sub-page navigation, organised around the object the operator is thinking about rather
     than the owning subsystem.
  - [ ] 2. Build the claim page — the token field, the recovery affordance once an admin exists, and the
     Blocked treatment carrying the retry time — against the endpoints from Task 1.12.
  - [ ] 3. Build the first-run admin-account creation page, reachable only with an active claim and only
     when no admin exists.
  - [ ] 4. Build the login page covering both local credentials and the Plex PIN/OAuth flow from Task 1.2.
  - [ ] 5. Render every shell page in the "nothing created yet" Empty treatment, since no page carries real
     data until a later phase populates it.
- [ ] **Done when:** a fresh instance boots directly to the claim page, refuses every other route until
  the instance is claimed and an admin exists, and every one of the six destinations plus Settings
  resolves to a routed page rather than a 404.

### Task 1.12 The bootstrap token and the setup claim
- **Build:** the console-printed bootstrap token, the setup claim leased to one browser, and the
  recovery path that replaces the token once an admin account exists. Specified in PRD §19.6.1;
  decided as D-045 and D-046.
- **Where:** `backend/crates/afisharr` (banner, token state); `backend/crates/api` (claim, recovery, and the claim
  gate); `backend/crates/core` (`setup:claim` lease, derived resume step).
- **Subtasks:**
  - [ ] 1. Implement token generation: three four-character segments from a 36-character lowercase
     alphanumeric alphabet, drawn from the OS CSPRNG with rejection sampling — discard and redraw any
     byte at or above 252 rather than reducing modulo 36, or the 62-bit claim is false.
  - [ ] 2. Hold the token in process memory with a 15-minute expiry, replacing any predecessor. Assert by
     test that it reaches no table, no response body, and no line of `logs/afisharr.log`.
  - [ ] 3. Print the banner from the startup sequence built in Task 0.3, only when
     `instance.setup_completed_at` is `NULL`: the token, the setup URL composed from the configured
     host and port, and the three events that end the token's life.
  - [ ] 4. Implement validation as check-and-keep, not consume: exists, unexpired, length matches, then a
     constant-time comparison. One error response covers wrong, expired, malformed, and empty.
  - [ ] 5. Implement the claim as a `setup:claim` lease whose `owner` is the SHA-256 of the cookie value,
     with a 10-minute expiry, and set `afisharr_setup_claim` with `HttpOnly`, `Secure` over HTTPS,
     `SameSite=Lax`, `Path=/api/setup`, and `Max-Age=600`.
  - [ ] 6. Implement the claim gate as middleware over every setup endpoint: renew on success, refuse with
     the Blocked response and the claim's expiry time otherwise. Renewal moves both the lease expiry
     and the cookie's `Max-Age`.
  - [ ] 7. Order the claim endpoint as: holder renews and succeeds; held-elsewhere returns Blocked before
     the rate limiter is consulted; then the limiter from Task 1.4 at 5 attempts per IP per 15
     minutes; then the token comparison.
  - [ ] 8. Implement recovery: admin credentials mint a claim when setup is incomplete, an admin exists,
     and no claim is active. Verify the password with the Argon2id path from Task 1.2; return the
     same response for an unknown username and a wrong password.
  - [ ] 9. Implement the derived resume step from the table in PRD §7.14, reading `instance.setup_acked_steps`
     for the two acknowledgement-only steps, and reject any client-supplied step index outright.
  - [ ] 10. Append one `job_run_events` row per setup step under a single `Api`-triggered `job_runs` row —
      not the lifecycle audit record, which PRD §21.4.8 reserves for what the engine did.
  - [ ] 11. Implement release: completing setup writes `instance.setup_completed_at`, deletes the lease,
      clears the in-memory token, and expires the cookie. The setup endpoints answer 404 thereafter.
- [ ] **Done when:** `I-SEC-8` passes — every wizard endpoint refuses without a claim on a fresh
  instance; wrong, expired, malformed, and empty tokens are indistinguishable in the response; a
  second cookie's claim attempt against a held claim returns the retry time and changes no state; the
  token appears in no table, no response, and no log file; and a restart with setup incomplete
  invalidates the previous token. `I-UX-10` is claimed in Phase 13, where the wizard the resume step
  serves is built.

---

## Phase 2 — Plex client and the adversarial fake

**Size:** `L`.

**Exit invariants:** `I-ID-5`, plus the fake's own criterion — every behaviour in its fidelity contract
reproducible from its seed, and the contract test passing against a real server.

**Not here:** anything that consumes Plex data.

**This phase is infrastructure and will feel like a detour. It is not.** D-036 exists because a stub
cannot express the failures the invariants from Phase 4 onward are written against.

### Task 2.1 The Plex protocol client
- **Build:** the protocol surface Afisharr actually calls, and no more — authentication, library and
  item listing, collection CRUD, hub reposition, labels, media-stream facts, artwork upload, and
  per-library filter-metadata discovery.
- **Where:** `backend/crates/plex`.
- **Subtasks:**
  - [ ] 1. Implement the `X-Plex-*` header contract and the PIN/OAuth token exchange consumed by Task 1.2.
  - [ ] 2. Implement library listing and item listing, including the filter-query parameter shapes
     (`/library/sections/{key}/all?…`) with operator suffixes (`!=`, `>>=`, `<<=`, `&=`, doubled `=`
     for exact string matching).
  - [ ] 3. Implement collection create, update, and item add/remove/reorder calls, plus hub reposition and
     visibility calls.
  - [ ] 4. Implement label add/remove, media-stream fact retrieval, and artwork upload.
  - [ ] 5. Implement the per-library, per-libtype filter-metadata discovery calls: filtering types, fields
     with type and subtype, per-type legal operators, and enumerated filter choices with their fast
     keys.
  - [ ] 6. Implement machine-identifier retrieval as its own call, since a changed value must be detectable
     without a full library fetch.
- [ ] **Done when:** the crate compiles with no dependency on any other Afisharr crate's domain types, and
  every call in this list is exercised by a unit test against a hand-rolled fixture response before the
  fake in Task 2.2 exists.

### Task 2.2 The adversarial fake, to full fidelity
- **Build:** a test-only Plex implementation satisfying the fidelity contract: silent no-op moves,
  unrecognised artwork URL formats, rating-key churn, partial scan states, independently-controllable
  sort-title value/presence/lock, mid-pass timeouts and 5xx failures at a chosen operation, and a
  changeable machine identifier — every behaviour deterministic from a seed.
- **Where:** a test-support module inside `backend/crates/plex`, compiled only under test, consumed by every
  later phase's test suite.
- **Subtasks:**
  - [ ] 1. Implement a move that reports success over the wire while not actually changing order, triggered
     past a configurable precision budget.
  - [ ] 2. Implement artwork URLs in at least two unrecognised formats.
  - [ ] 3. Implement rating-key churn: the same logical item reappearing under a new key on a later fetch.
  - [ ] 4. Implement partial scan states — an item indexed but not yet complete.
  - [ ] 5. Implement sort titles with independently settable value, presence, and lock state.
  - [ ] 6. Implement timeout and 5xx injection at a caller-chosen operation, mid-pass.
  - [ ] 7. Implement a machine-identifier change, triggerable on demand.
  - [ ] 8. Seed every behaviour from one explicit seed value; assert byte-identical replay from the same
     seed across two runs.
- [ ] **Done when:** every row of the fidelity contract reproduces deterministically from its seed, and
  `I-ID-5` passes against the fake — a changed machine identifier is detected and treated as a possibly
  different server rather than silently reconciled.

### Task 2.3 The release-lane contract test against a real server
- **Build:** a contract test exercising the client from Task 2.1 against a real Plex server, run in the
  release lane, that keeps the fake honest.
- **Where:** `backend/crates/plex/tests`, wired into the release lane from Task 0.1.
- **Subtasks:**
  - [ ] 1. Exercise the same call surface as Task 2.1 against a real server reachable in CI via a
     release-lane credential.
  - [ ] 2. Assert response shapes match what Task 2.1's parsers expect for the happy path on every call.
  - [ ] 3. Fail the release lane, by name, on any call whose real-server shape has drifted from what the fake
     assumes.
- [ ] **Done when:** the contract test passes in the release lane against a real server, and a deliberately
  introduced shape mismatch between the fake and a captured real response fails the test rather than
  passing silently.

### Task 2.4 Interface pages this phase ships
- **Build:** the Plex connectivity indicator in Settings — reachable, unreachable, or wrong-server —
  using the nine-state vocabulary from Task 1.8, without displaying any library content.
- **Where:** `frontend/src/routes` (Settings › Plex connection); `backend/crates/api`.
- **Subtasks:**
  - [ ] 1. Add a lightweight connectivity check hitting the machine-identifier call from Task 2.1.
  - [ ] 2. Render the three connection states (reachable, unreachable, wrong-server per `I-ID-5`'s blocking
     condition) through the shared state components.
  - [ ] 3. Surface the blocking "this is a new server, rebind" or "restore a backup" choice as a Blocked
     state, with no library data behind it.
- [ ] **Done when:** the Settings page shows a live connectivity state sourced from Task 2.1's calls, and a
  simulated machine-identifier change renders as Blocked with both recovery options offered, neither
  auto-resolved.

---

## Spike track — Q-015 then Q-014

**Size:** `M`. Runs in parallel from the end of Phase 2. **Must land before the placement phase
begins** — a placement phase designed against an assumed answer is one built twice.

**Why the spikes run in parallel rather than first:** they need nothing but a real Plex server and a
Plex client — no schema, no engine, no collections — so they can start the moment Phase 2 exists and
run alongside Phases 3 through 6 at almost no cost to the main line.

**Why Q-015 before Q-014:** Q-015 decides whether the home screen is one global ordering sequence or
several per-library sequences merged at render, which determines whether ordering is one planning
problem or several, and it also unblocks the home-screen board design (Q-013). Q-014's measurement
depends on that answer, so it runs second.

### Task S.1 Q-015 — one global sequence, or several merged at render
- **Build:** a direct answer, from observation of a real server, to whether the home screen is one
  global ordering sequence or per-library sequences merged at render.
- **Where:** a spike harness outside the crate boundary, exercising `backend/crates/plex` against a real
  server; not merged into product code.
- **Subtasks:**
  - [ ] 1. Instrument a real server to observe how home-screen hub composition behaves under controlled
     reordering across more than one library.
  - [ ] 2. Test both hypotheses directly against server behaviour rather than against documentation of it.
  - [ ] 3. Record the answer with its supporting measurement.
  - [ ] 4. Amend the placement design document with the resolved answer.
- [ ] **Done when:** the question is answered with a recorded measurement, and the placement design
  document states, with evidence, which of the two shapes ordering takes.

### Task S.2 Q-014 — the real precision budget
- **Build:** a calibrated precision budget for minimal-move planning, measured against a sequence of at
  least 2,500 items — a short sequence measures the wrong thing.
- **Where:** the same spike harness, against `backend/crates/plex` and a real server.
- **Subtasks:**
  - [ ] 1. Build a sequence of at least 2,500 items against a real server, shaped by S.1's answer (global or
     per-library).
  - [ ] 2. Measure actual move-precision behaviour — how many moves silently no-op, and under what
     conditions — at that scale.
  - [ ] 3. Compare the measurement against the placement budget recorded in the PRD's §21.2.2.
  - [ ] 4. Confirm the existing budget or replace it with the measured value.
- [ ] **Done when:** the precision budget in the PRD's §21.2.2 is either confirmed or replaced by a
  recorded measurement from a sequence of at least 2,500 items, and the placement design document is
  amended accordingly.

**Exit (whole track):** both questions answered with recorded measurements, the placement design
document amended, and the placement budget confirmed or replaced.

---

## Phase 3 — Definition engine and registries

**Size:** `L`.

**Exit invariants:** `I-DEF-1`, `I-DEF-2`, `I-DEF-3`, `I-DEF-5`, `I-DEF-6`, `I-DEF-7`.

**Not here:** packs and pack upgrade. Sources.

### Task 3.1 Envelope, kinds, and definition storage
- **Build:** the definition envelope (`kind`, `schemaVersion`, `registryVersion`, ULID, handle, `meta`,
  `spec`), the seven kinds, and the storage and concurrency machinery around them.
- **Where:** `backend/crates/core` (definition module); `backend/crates/api` (`/api/definitions/{kind}` CRUD).
- **Subtasks:**
  - [ ] 1. Implement the envelope type and canonical JSON serialisation with stable key ordering.
  - [ ] 2. Implement the seven kinds — `Collection`, `Playlist`, `Placement`, `OverlayTemplate`,
     `PosterTemplate`, `SmartFilterDef`, `PackManifest` — as typed `spec` variants.
  - [ ] 3. Implement namespaced identifiers: an immutable ULID plus a `namespace/slug` handle, with packs
     owning their namespace and user definitions living under `user/`.
  - [ ] 4. Wire the `definitions` table with its derived columns, `definition_refs`, and
     `definition_libraries`.
  - [ ] 5. Implement optimistic concurrency: the GUI save is a compare-and-swap on `body_hash`; a background
     pass whose read hash has since changed discards its results for that definition and re-queues it,
     never merges.
- [ ] **Done when:** a definition round-trips through export and import byte-for-byte after
  canonicalisation for every kind; a save against a stale `body_hash` affects zero rows and returns the
  current body for a diff rather than overwriting it; and deleting a definition with inbound
  `definition_refs` rows requires an explicit cascade choice rather than succeeding silently.

### Task 3.2 Condition and filter expression trees
- **Build:** the one structured expression language shared by collection filters, overlay conditions,
  and lifecycle rules — leaves, combinators, and scoped quantifiers.
- **Where:** `backend/crates/core` (expression tree, evaluator).
- **Subtasks:**
  - [ ] 1. Implement leaves (`field`/`op`/`value`), combinators (`all`/`any`/`not`), and scoped quantifiers
     (`scope`, `quantifier: any|all|none|{countGte:N}|{countLt:N}`, `tree`).
  - [ ] 2. Implement empty-child semantics exactly: over zero children, `any` is false, `none` is true, `all`
     is vacuously true, `countGte:1` is false.
  - [ ] 3. Cap nesting depth at two scoped levels and require the child data behind a quantifier to be
     batch-loadable rather than evaluated per item.
  - [ ] 4. Implement regex leaves using the linear-time engine, compiled and size-capped at save time.
- [ ] **Done when:** `I-DEF-7` passes — a table-driven test exercises every empty-child case in the
  semantics above and each returns the specified value; the GUI condition builder warns when `all` is
  used without a companion `countGte` guard.

### Task 3.3 The four registries as Rust constants
- **Build:** the field registry (static core plus the server-discovered layer), the operator set, the
  formatter registry, and the source registry entry shape — compiled into the binary as constants, per
  D-016.
- **Where:** `backend/crates/core` (registry constants, generated JSON artifact); `backend/crates/api` (serving the
  artifact to the GUI).
- **Subtasks:**
  - [ ] 1. Encode the static-core field catalogue (`item.*`, `media.*`, `ratings.*`, `lifecycle.*`,
     `show.*`/`season.*`/`episode.*`, `collection.*`) as Rust constants, each with type, cardinality,
     scope, availability class, provenance, nullability, legal operators, legal formatters.
  - [ ] 2. Implement the operator set with its type/cardinality acceptance table and the boolean-field
     restriction to `eq`/`exists` only.
  - [ ] 3. Implement the formatter registry as pure functions, each declared with its accepted input type,
     including locale-dependent formatters taking the instance locale by default with an explicit
     override.
  - [ ] 4. Implement the `discovered_fields`/`discovered_field_choices`/`discovered_sorts` snapshot cache and
     its snapshot-scoped atomic swap, so the field registry is genuinely two-layered.
  - [ ] 5. Implement `registry_versions` as the append-only version-snapshot table, populated from the
     compiled constants, with a generated JSON artifact and a CI drift check comparing the artifact
     against the constants that produced it.
  - [ ] 6. Enforce that static-core keys win on collision with discovered `plex.*` keys.
- [ ] **Done when:** the CI drift check fails if the generated JSON artifact and the compiled Rust constants
  diverge; a condition referencing a discovered field records the library it was authored against and
  falls back to local evaluation with a flag, never a silent drop, when that field is later absent
  (`I-DEF-2`, per D-017's stale-field rule — warn and fall back, never block the save).

### Task 3.4 The validation pipeline
- **Build:** the eleven-step save-time validation sequence as one code path, with structured,
  pointer-addressed errors.
- **Where:** `backend/crates/core` (validation); `backend/crates/api` (surfacing structured errors).
- **Subtasks:**
  - [ ] 1. Implement the eleven checks in order, stopping at the first failure: envelope shape; kind schema;
     source parameters against JSON Schema; field-key existence and scope; operator legality for type
     and cardinality; literal-value type and enum membership; formatter legality and pipeline
     type-checking; ordering-mode compatibility with source capabilities; seed presence for
     non-deterministic sources; reference resolution and cron parsing; regex compile within the size
     cap.
  - [ ] 2. Return every failure as a JSON pointer plus registry key plus expected-versus-actual, matching the
     structured error type from Task 1.1.
  - [ ] 3. Reject a definition combining Plex-native compiled filtering with `sourcePosition` or manual
     ordering — the smart-collection constraint — at save time.
- [ ] **Done when:** `I-DEF-1` passes — no definition body can carry executable code or a string expression
  language, verified by fuzzing the body shape; a definition combining Plex-native filtering with
  manual ordering is rejected at save with a pointer to the offending field, never accepted and
  silently mis-ordered later.

### Task 3.5 Export and import
- **Build:** canonical-JSON export and import with exact round-trip.
- **Where:** `backend/crates/core`; `backend/crates/api` (export/import endpoints).
- **Subtasks:**
  - [ ] 1. Implement pretty-printed canonical JSON export for any definition.
  - [ ] 2. Implement import running the same eleven-step validation as a save.
  - [ ] 3. Implement the missing-dependency prompt for an imported definition referencing a pack asset the
     importing instance does not have installed.
- [ ] **Done when:** `import(export(x)) == x` byte-for-byte after canonicalisation for every kind, and
  importing a definition referencing an uninstalled pack asset produces the missing-dependency prompt
  rather than a silently broken reference.

### Task 3.6 Definition history
- **Build:** the last-20-versions history per definition, with diff and restore.
- **Where:** `backend/crates/core` (`definition_history`); `backend/crates/api`; `frontend/` (history panel, per the
  cross-cutting affordances every definition-backed object carries).
- **Subtasks:**
  - [ ] 1. Write a `definition_history` row on every accepted save, keyed by `(definition_id,
     body_version)`, with no foreign key to `definitions` so history outlives deletion.
  - [ ] 2. Implement a diff view between any two retained versions.
  - [ ] 3. Implement restore: writing an old body back through the normal save path, including
     revalidation.
  - [ ] 4. Enforce retention at 20 versions per definition.
- [ ] **Done when:** restoring a historical version re-runs full save-time validation rather than writing
  the old body directly, and a deleted definition's history remains readable for the retention window.

### Task 3.7 User-defined computed fields
- **Build:** the restricted computed-field capability from CR-1/D-018 — one arithmetic operation over
  two registered numeric fields, in a closed `user.*` namespace.
- **Where:** `backend/crates/core` (`computed_fields` table, registry integration).
- **Subtasks:**
  - [ ] 1. Implement the `computed_fields` table: operation (`add`/`subtract`/`multiply`/`divide`), two
     operand field keys, result type, `null_policy` (`Null` default, `Zero` opt-in), derived
     availability class.
  - [ ] 2. Enforce that both operands must be registered, non-computed numeric fields — no nesting, ever.
  - [ ] 3. Enforce exactly one operation, no constants, no third operand.
  - [ ] 4. Enforce the closed `user.` namespace, colliding with neither the static core nor `plex.*`.
  - [ ] 5. Implement tombstone deletion: `deleted_at` set, the unique index still covering deleted rows so
     the key can never be reused.
  - [ ] 6. Derive `availability` as `integration` if either operand is `integration`, so pack
     `requiresFields` resolution keeps working through a computed field.
- [ ] **Done when:** `I-DEF-6` passes — an attempt to reference a computed field as an operand of another
  computed field is rejected at save time; a division by zero yields null under both null policies; and
  a deleted computed field's key can never be reused by a later create.

### Task 3.8 Interface pages this phase ships
- **Build:** the definition editor shell, the registry-generated condition and expression builder, and
  the cross-cutting affordances every definition-backed object carries.
- **Where:** `frontend/src/routes` (definition editors); `frontend/` (shared condition-builder component).
- **Subtasks:**
  - [ ] 1. Build a condition-tree builder generated from the field and operator registries — adding a field
     to the registry must add a working control with no frontend code change.
  - [ ] 2. Build export, history-with-diff-and-restore, duplicate (including forking a pack-origin
     definition to `user/`), enable/disable, and where-used affordances, available on every
     definition-backed object.
  - [ ] 3. Build the computed-field creation control from Task 3.7's constraints.
- [ ] **Done when:** adding a new field to the static-core registry produces a working condition-builder
  control with zero changes to the frontend beyond the registry constant, verified by adding one test
  field and confirming the control appears.

---

## Phase 4 — Library cache and identity

**Size:** `L`.

**Exit invariants:** `I-ID-1`, `I-ID-2`, `I-ID-3`, `I-ID-4`, `I-EVID-8`, `I-DATA-4`, `I-PERF-1`.

**Not here:** collections.

**`I-PERF-1` lands here because this is the first component that touches all 200,000 items.** The
streaming discipline is set on the first pass that needs it, not retrofitted onto four later ones.

### Task 4.1 Library discovery
- **Build:** discovery and tracking of Plex libraries, filtering to the two representable types.
- **Where:** `backend/crates/core` (`libraries` table); `backend/crates/plex` (the discovery calls from Phase 2).
- **Subtasks:**
  - [ ] 1. Populate `libraries` from the server's section list, keyed on `section_uuid` with an immutable
     `handle` that definitions reference.
  - [ ] 2. Filter `music` and `photo` library types out at discovery — they are never inserted, so an
     unrepresentable state cannot be reached by a bug.
  - [ ] 3. Rebind after a section-key change by matching `section_uuid` first, then `(type, title)` with
     confirmation.
  - [ ] 4. Track `scanned_at`, `cache_refreshed_at`, and `missing_since` per library.
- [ ] **Done when:** a library whose Plex section key changes rebinds to the same `handle` with no
  definition edit required, and a music or photo library never produces a row.

### Task 4.2 The item cache
- **Build:** the per-item cache — movies, shows, seasons, episodes — with soft deletion.
- **Where:** `backend/crates/core` (`library_items`).
- **Subtasks:**
  - [ ] 1. Populate `library_items` with parent linkage (season → show, episode → season), `rating_key`,
     `guid`, and the tracked metadata fields.
  - [ ] 2. Implement `is_placeholder` as a column set only inside the transaction that materialises or
     resolves a lifecycle intent — never derived from filename or Plex label.
  - [ ] 3. Implement soft deletion via `missing_since`: an item absent for one pass is not hard-deleted,
     since it may be mid-scan.
  - [ ] 4. Implement the reaping job that hard-deletes items missing longer than the retention window.
- [ ] **Done when:** `I-DATA-4` passes — an item that disappears from Plex for a single pass and then
  reappears retains its base-poster and lifecycle bindings across the gap, because it was soft-deleted
  rather than hard-deleted on first absence.

### Task 4.3 Metadata change tracking
- **Build:** a hashable projection of item facts, separated from ratings by refresh cadence and
  availability class.
- **Where:** `backend/crates/core` (`library_item_state`).
- **Subtasks:**
  - [ ] 1. Compute `metadata_hash` on `library_items` from the tracked metadata fields, to detect changes
     cheaply on each pass.
  - [ ] 2. Implement `library_item_state` with `facts_json`/`facts_hash` separate from
     `ratings_json`/`ratings_hash`/`ratings_fetched_at`.
  - [ ] 3. Preserve the distinction between a `NULL` `ratings_json` (unavailable — fetch failed or
     unconfigured) and a JSON `null` inside a populated body (known to have no value).
  - [ ] 4. Compute the `state_hash` digest over facts, ratings, and lifecycle as the overlay render key's
     dependency.
- [ ] **Done when:** a failed rating fetch leaves `facts_json` untouched, and a test asserts the
  `NULL`-versus-`null` distinction survives a full write/read cycle rather than collapsing to one
  meaning.

### Task 4.4 The discovered field cache
- **Build:** the persisted half of the two-layer field registry from Task 3.3 — one snapshot per
  library, invalidated on observable events only.
- **Where:** `backend/crates/core` (`discovery_snapshots` and children, `definition_field_uses`).
- **Subtasks:**
  - [ ] 1. Write a new snapshot and flip `is_current` in one transaction, so a failed or partial discovery
     never leaves the cache half-rewritten.
  - [ ] 2. Retain the two most recent non-current snapshots for diagnosis; delete older ones, cascading to
     their fields and choices.
  - [ ] 3. Trigger invalidation on three events only: a library scan advancing `scanned_at`, a Plex version
     change, and an explicit doctor-page refresh — no TTL.
  - [ ] 4. Populate `definition_field_uses` on every definition save, recording the authored library for each
     discovered-field reference.
- [ ] **Done when:** a discovery run that fails partway through leaves the previous snapshot's fields fully
  usable, with no window where `discovered_fields` returns a partial set as if it were complete.

### Task 4.5 Canonical id resolution
- **Build:** resolution of external identifiers (TMDB/TVDB/IMDb triple, Plex GUID) to library items,
  with ambiguity recorded rather than guessed.
- **Where:** `backend/crates/core` (`library_item_ids`, `ambiguous_matches`).
- **Subtasks:**
  - [ ] 1. Populate `library_item_ids` from Plex's own GUID plus agent- and mapping-derived identifiers.
  - [ ] 2. Implement the lookup index resolving `(id_space, id_value)` to a library item within a library.
  - [ ] 3. Detect more than one match within a library and record it in `ambiguous_matches` rather than
     guessing; block every subject action referencing the ambiguous identifier until it resolves.
  - [ ] 4. Implement resolution recording (`resolved_item_id`, `resolved_by`) so a later pass reads the
     pinned choice rather than re-detecting.
- [ ] **Done when:** `I-EVID-8` passes — an unresolved ambiguous match blocks every action on the subjects
  it covers, visibly, until an explicit resolution is recorded; a resolved row is read directly on the
  next pass with no re-detection.

### Task 4.6 Anime id mapping
- **Build:** bulk-imported cross-provider identifier mapping (AniList/MAL to TVDB/TMDB), refreshed
  wholesale on a schedule, kept apart from per-item state.
- **Where:** `backend/crates/core` (`id_mappings`).
- **Subtasks:**
  - [ ] 1. Implement `id_mappings` as a composite-key, `WITHOUT ROWID` table, season-aware where a mapping
     depends on season.
  - [ ] 2. Implement the wholesale-refresh import job for the mapping dataset, separate from
     `library_item_ids` so a dataset refresh never rewrites per-item state.
- [ ] **Done when:** a dataset refresh rewrites `id_mappings` in full without touching a single row of
  `library_item_ids`.

### Task 4.7 Rebinding and self-healing
- **Build:** recovery from Plex-assigned identifier churn without treating the item as new.
- **Where:** `backend/crates/core` (rebinding logic over `library_items` and `library_item_ids`).
- **Subtasks:**
  - [ ] 1. Detect rating-key churn — the same canonical identity reappearing under a new `rating_key` — and
     rebind the existing row via `UPDATE`, never a re-key.
  - [ ] 2. Detect unrecognised artwork URL formats without failing the pass; record and continue.
  - [ ] 3. Feed rebinding into the ambiguity surface (Task 4.5) when a churned key resolves to more than one
     candidate.
- [ ] **Done when:** `I-ID-1` and `I-ID-3` pass against the fake's rating-key-churn scenario — the same
  logical item under a new key rebinds to its existing row rather than creating a duplicate; `I-ID-2`
  passes — an unrecognised artwork URL format is recorded and does not abort the pass.

### Task 4.8 The ambiguity surface
- **Build:** the resolution API and minimal list surface for ambiguous matches, ahead of the full
  doctor page.
- **Where:** `backend/crates/api` (resolution endpoint); `frontend/` (minimal list, feeding the doctor page later).
- **Subtasks:**
  - [ ] 1. Expose an endpoint listing unresolved `ambiguous_matches` rows with their candidates.
  - [ ] 2. Expose a resolution endpoint recording `resolved_item_id` and `resolved_by`.
  - [ ] 3. Surface the list on a minimal page reachable from the collection editor via a deep link, per the
     shape D-013 settles (resolution stored once, applying everywhere it is read).
- [ ] **Done when:** `I-ID-4` passes — resolving an ambiguous match through this surface is read by every
  later pass with no re-detection, and the same resolved state is visible whether reached from the
  editor deep link or the list directly.

### Task 4.9 Streaming discipline over the full library
- **Build:** the batch/stream processing discipline for any pass touching every item in a library, set
  here because this is the first component that does.
- **Where:** `backend/crates/core` (pass execution).
- **Subtasks:**
  - [ ] 1. Process items in bounded batches rather than materialising a full 200,000-item library in memory.
  - [ ] 2. Make every full-library pass resumable at a batch boundary, consistent with the no-transaction-
     across-I/O rule from Task 0.4.
  - [ ] 3. Establish the pattern as a reusable pass-execution helper so later full-library passes (rendering,
     lifecycle) reuse it rather than reinventing it.
- [ ] **Done when:** `I-PERF-1` passes — a full-library pass over a 200,000-item fixture library completes
  within the memory budget in the PRD's *Non-functional requirements*, measured rather than assumed.

### Task 4.10 Interface pages this phase ships
- **Build:** the Libraries settings page and item-cache status indicators.
- **Where:** `frontend/src/routes` (Settings › libraries).
- **Subtasks:**
  - [ ] 1. List discovered libraries with type, item count, last scan, and last cache-refresh time.
  - [ ] 2. Show a library missing from the server (per `missing_since`) as a distinct state, not silently
     dropped from the list.
  - [ ] 3. Surface the minimal ambiguous-match list from Task 4.8 as a page section.
- [ ] **Done when:** the Libraries page reflects `libraries.missing_since` and `cache_refreshed_at` live,
  and every unresolved ambiguous match from Task 4.8 is reachable from this page.

---

## Phase 5 — Sources and the reconciliation pipeline

**Size:** `XL`. The largest phase. Roughly twenty adapters, several with multiple subtypes.

**Exit invariants:** `I-SRC-1`, `I-SRC-2`, `I-SRC-3`, `I-SRC-4`, `I-SRC-5`, `I-SRC-7`, `I-SRC-8`,
`I-EVID-1`, `I-EVID-4`, `I-DATA-12`, `I-DATA-13`, `I-SEC-7`, `I-PERF-4`.

**Not here:** writing anything to Plex.

**Why the infrastructure tasks come before the second adapter, not after the twentieth:** the endpoint
ladder, the circuit breaker, the parser-versioned cache, the volatile-parameter feed, and the bulk
dataset importer are infrastructure every adapter then sits on. Retrofitting a parser version into a
shipped cache means invalidating every entry at once — the traffic shape `I-PERF-4` exists to prevent.
Build them before the second adapter, not after the twentieth.

**The adapters are the tail, and the tail is long.** Build the interface and two adapters, prove the
invariants against those two, then the rest is repetition. If anything here is cut under pressure, it
is adapters — a content decision, not an engine one.

### Task 5.1 The generic source interface
- **Build:** the `SourceBuilder` trait every adapter implements, with a typed client, rate limiter,
  circuit breaker, response validator, and health-status hook built once.
- **Where:** `backend/crates/sources` (trait and shared scaffolding).
- **Subtasks:**
  - [ ] 1. Define the `SourceBuilder` trait: fetch, declared parameters (JSON Schema), declared endpoints
     (the ladder), declared `idSpace`.
  - [ ] 2. Wire every adapter through the shared typed HTTP client, rate limiter, and circuit breaker rather
     than each adapter owning its own.
  - [ ] 3. Implement mandatory response validation ahead of parsing, so a challenge page can never reach a
     parser and be counted as zero items.
- [ ] **Done when:** a minimal reference adapter implementing only `SourceBuilder` compiles and runs
  end-to-end against the shared client, breaker, and validator with no adapter-owned HTTP code.

### Task 5.2 The endpoint ladder with per-rung capabilities
- **Build:** the ordered, per-rung endpoint declaration (`structured` / `embedded` / `markup`), each
  rung carrying its own `parserVersion` and capability flags, per D-040.
- **Where:** `backend/crates/sources` (ladder execution); `backend/crates/core` (registry entry shape from Task 3.3).
- **Subtasks:**
  - [ ] 1. Implement the ladder as an ordered list tried top to bottom, most sources declaring exactly one
     rung.
  - [ ] 2. Implement per-rung `affirmativeEmpty`, `ordered`, `deterministic`, `paginated`, `supportsLimit`.
  - [ ] 3. Apply the capability flags of the rung that actually answered, never the source's best rung.
  - [ ] 4. Reject `order.by: sourcePosition` against a source/rung declaring `ordered: false` at definition
     save time.
- [ ] **Done when:** `I-SRC-8` passes — a source whose top rung declares `affirmativeEmpty: true` and whose
  fallback rung declares `affirmativeEmpty: false` is tested falling through to the fallback, and the
  engine treats the resulting zero-item response as a failure, not an affirmed empty list.

### Task 5.3 The circuit breaker
- **Build:** persisted, per-source circuit-breaker state that survives restart.
- **Where:** `backend/crates/sources` (`source_health`).
- **Subtasks:**
  - [ ] 1. Implement `source_health` keyed on `(source_type, instance_ref)`, tracking consecutive failures,
     open/half-open/closed state, and cooldown.
  - [ ] 2. Classify every failure into `last_error_kind` (`Timeout`/`Http4xx`/`Http5xx`/`Challenge`/
     `Parse`/`Auth`/`RateLimit`), with `Challenge` distinguished from `Parse` specifically.
  - [ ] 3. Persist breaker state to the table on every transition, so a crash-loop or routine restart never
     resets an open breaker into a burst of retries against an already-failing service.
  - [ ] 4. Record a fallthrough to a lower ladder rung as a degradation the breaker/health record surfaces,
     not a silent success.
- [ ] **Done when:** `I-SRC-4` passes — restarting the process mid-cooldown leaves the breaker open rather
  than resetting to closed, verified by a restart test against a source whose breaker is open.

### Task 5.4 Frozen contributions and degraded state
- **Build:** the last-known-good freeze that keeps a failed source from emptying a collection.
- **Where:** `backend/crates/sources` / `backend/crates/core` (`source_contributions`).
- **Subtasks:**
  - [ ] 1. Implement `source_contributions`, retaining at most two rows per `(definition_id, source_index)`:
     most recent, and most recent trustworthy (`status = 'Ok'` and either `item_count > 0` or
     `affirmed_empty = 1`).
  - [ ] 2. Freeze a source's contribution at its last trustworthy value on failure or on an unaffirmed empty
     result, rather than emptying the collection.
  - [ ] 3. Guard the freeze with `params_hash`: a frozen contribution whose parameters have changed since is
     not reused, because it answers a different question now.
- [ ] **Done when:** `I-SRC-1` passes — a source returning zero items without declaring
  `affirmativeEmpty` freezes at its last trustworthy contribution rather than propagating an empty
  result downstream, verified against the fake's mid-pass failure injection from Task 2.2.

### Task 5.5 One instrumented HTTP client with a mandatory per-request timeout
- **Build:** the single shared HTTP client every adapter uses — timeout, retry, backoff, jitter, log
  deduplication.
- **Where:** `backend/crates/sources` (shared client module).
- **Subtasks:**
  - [ ] 1. Enforce a mandatory per-request timeout with no adapter able to opt out.
  - [ ] 2. Implement retry with exponential backoff and jitter, bounded, feeding failures into the breaker
     from Task 5.3.
  - [ ] 3. Implement log deduplication so a failing source does not flood the log at retry cadence.
  - [ ] 4. Detect challenge pages by response validation ahead of the transport ladder's fallback client.
- [ ] **Done when:** a request exceeding the configured timeout is aborted and classified `Timeout` in
  `source_health` rather than hanging the calling pass, verified by a test against an artificially slow
  endpoint.

### Task 5.6 The parser-versioned response cache with spread expiries
- **Build:** `http_cache`, keyed on request plus the interpreting parser's version, with expiries
  spread to avoid a synchronized refetch storm — per D-043.
- **Where:** `backend/crates/sources` (`http_cache`).
- **Subtasks:**
  - [ ] 1. Compute `cache_key` as a digest over method, URL, relevant headers, and `parser_version` — inside
     the key, not stored beside it.
  - [ ] 2. Compute `expires_at` as `fetched_at + ttl - random(0, ttl / 4)` on every write, so entries written
     together do not expire together.
  - [ ] 3. Implement conditional revalidation via `etag`/`last_modified` where a provider supports it.
  - [ ] 4. Wire `parser_version` from each source registry entry's per-rung `parserVersion`, bumped by the
     adapter author in the same commit as any parsing fix.
- [ ] **Done when:** `I-DATA-12` passes — bumping a source's `parserVersion` makes every previously cached
  entry for that source miss and refetch, never returning a response shaped by the old parser;
  `I-PERF-4` passes — a batch of cache writes at the same instant produces a measurably spread
  distribution of `expires_at` values, not a single cluster.

### Task 5.7 The volatile-parameter feed and its signature verification
- **Build:** the out-of-band feed that supplies provider-rotated parameter values, constrained to the
  names the binary ships — per D-041.
- **Where:** `backend/crates/sources` (`volatile_params`, feed fetch and verification).
- **Subtasks:**
  - [ ] 1. Implement `volatile_params`, rejecting any `name` absent from the shipped registry's
     `volatileParams` declarations.
  - [ ] 2. Verify the feed's signature before applying any value; report an unverifiable feed on the doctor
     surface without applying it.
  - [ ] 3. Check every fetched value against its declared syntactic constraint before storing it; on failure,
     increment `reject_count`, record `last_reject_reason`, and keep `last_good_value` in force.
  - [ ] 4. Ensure the feed can change a declared parameter's value and do nothing else — no new parameter,
     no type change, nothing executable.
- [ ] **Done when:** `I-SEC-7` passes — a feed value failing its declared constraint is rejected, the
  previous `last_good_value` remains in effect for the next request, and `reject_count` increments;
  an unsigned or badly signed feed is never applied.

### Task 5.8 The bulk dataset importer
- **Build:** atomic, all-or-nothing import of a provider's whole-dataset file, staged and promoted by
  generation — per D-042.
- **Where:** `backend/crates/sources` (`reference_datasets`, `reference_dataset_rows`).
- **Subtasks:**
  - [ ] 1. Stream-decompress and batch-insert the incoming file at `generation + 1` with
     `import_state = 'Staging'`, never loading the full file into memory.
  - [ ] 2. Verify row count and a spot-check before promotion.
  - [ ] 3. Promote `generation + 1` to `Live` and delete the old generation in a single transaction; on
     failure, leave the previous generation `Live` and record `Failed` with a reason.
  - [ ] 4. Expose imported values as registry fields of availability class `integration` — never as a source
     contributing items, so this importer sits outside the circuit breaker and `affirmativeEmpty`
     entirely.
- [ ] **Done when:** `I-DATA-13` passes — a deliberately truncated import file leaves the previous
  generation live and readable, with the failure recorded and no half-imported generation ever visible
  to a reader.

### Task 5.9 TMDB adapters
- **Build:** charts, franchise, custom lists, random (seeded), Discover with nested filter groups,
  watch providers, and person collections (auto-collections).
- **Where:** `backend/crates/sources/tmdb`.
- **Subtasks:**
  - [ ] 1. Implement `tmdb.chart` (popular/topRated/trending, window, mediaType, limit).
  - [ ] 2. Implement `tmdb.franchise` (franchise id or seed title, includeParts) and `tmdb.list` (list url or
     id).
  - [ ] 3. Implement `tmdb.discover` with nested and/or filter groups and `sortBy`.
  - [ ] 4. Implement `tmdb.watchProvider` (providerId, region, mediaType).
  - [ ] 5. Implement `tmdb.person` (role, minItems, separator options) as an auto-collection source, distinct
     from the Tier-1 people-browse source.
  - [ ] 6. Implement `tmdb.random` (pool params, explicit seed) and reject its absence of a seed at save
     time.
- [ ] **Done when:** all six TMDB source types validate against their published JSON Schema, declare
  `affirmativeEmpty: true` on their structured rung, and each produces a deterministic result set from
  the fake or a recorded fixture.

### Task 5.10 Trakt adapters
- **Build:** charts, custom lists, recommendations.
- **Where:** `backend/crates/sources/trakt`.
- **Subtasks:**
  - [ ] 1. Implement `trakt.chart` (chart, period, mediaType).
  - [ ] 2. Implement `trakt.list` (list url).
  - [ ] 3. Implement `trakt.recommendations` (mediaType, limit).
- [ ] **Done when:** all three validate against their JSON Schema and pass the shared adapter contract test
  from Task 5.1.

### Task 5.11 IMDb adapters
- **Build:** charts and custom lists, declared on two rungs per CR-3/D-040 — a structured,
  cursor-paginated, typed-error endpoint, and an embedded-payload fallback that cannot distinguish
  empty from broken.
- **Where:** `backend/crates/sources/imdb`.
- **Subtasks:**
  - [ ] 1. Implement `imdb.chart` and `imdb.list` against the structured rung, declaring
     `affirmativeEmpty: true` there because its typed errors distinguish not-found from forbidden from
     empty.
  - [ ] 2. Implement the embedded-payload fallback rung, declaring `affirmativeEmpty: false`.
  - [ ] 3. Declare the structured rung's authenticating hash as a `volatileParams` entry, supplied by
     Task 5.7's feed.
  - [ ] 4. Scope both adapters to charts and custom lists only — no ratings, which arrive through Task 5.8's
     bulk importer on a separate cadence.
- [ ] **Done when:** the fallthrough from structured to embedded rung is exercised by a forced-failure test
  and the engine applies `affirmativeEmpty: false` for that run, per `I-SRC-8`.

### Task 5.12 Letterboxd lists adapter
- **Build:** the scraped-tier list source.
- **Where:** `backend/crates/sources/letterboxd`.
- **Subtasks:**
  - [ ] 1. Implement `letterboxd.list` (list url, random) audited against the endpoint ladder before any
     parser is written.
  - [ ] 2. Declare `affirmativeEmpty: false` by default, behind challenge detection and the shared circuit
     breaker.
- [ ] **Done when:** a challenge-page fixture response is rejected by response validation before reaching
  the parser and is never counted as zero items.

### Task 5.13 MDBList adapter
- **Build:** `mdblist.list` (list url).
- **Where:** `backend/crates/sources/mdblist`.
- **Subtasks:**
  - [ ] 1. Audit against the endpoint ladder and implement at the highest rung reached.
  - [ ] 2. Declare capabilities per rung.
- [ ] **Done when:** the adapter passes the shared adapter contract test from Task 5.1.

### Task 5.14 AniList adapter
- **Build:** `anilist.*` (chart or list url).
- **Where:** `backend/crates/sources/anilist`.
- **Subtasks:**
  - [ ] 1. Implement chart and list variants against AniList's structured API.
  - [ ] 2. Resolve results through the anime id mapping from Task 4.6 where the target library speaks
     TVDB/TMDB.
- [ ] **Done when:** an AniList result resolves to the correct library item via `id_mappings`, verified
  against a fixture with a known AniList-to-TVDB mapping.

### Task 5.15 MyAnimeList adapter
- **Build:** `mal.*` (chart or list).
- **Where:** `backend/crates/sources/mal`.
- **Subtasks:**
  - [ ] 1. Implement chart and list variants.
  - [ ] 2. Resolve through the anime id mapping as in Task 5.14.
- [ ] **Done when:** the adapter passes the shared adapter contract test and resolves through
  `id_mappings` identically to Task 5.14.

### Task 5.16 FlixPatrol adapters
- **Build:** networks and originals, scraped tier.
- **Where:** `backend/crates/sources/flixpatrol`.
- **Subtasks:**
  - [ ] 1. Implement `flixpatrol.networks` (country, platform) and `flixpatrol.originals` (platform).
  - [ ] 2. Declare `affirmativeEmpty: false`, behind challenge detection and the circuit breaker.
- [ ] **Done when:** both adapters pass the shared adapter contract test with `affirmativeEmpty: false`
  enforced.

### Task 5.17 Overseerr adapter
- **Build:** `overseerr.requests` (scope global/perUser, status).
- **Where:** `backend/crates/sources/overseerr`.
- **Subtasks:**
  - [ ] 1. Implement the requests query against Overseerr's API.
  - [ ] 2. Declare `affirmativeEmpty: true` on its structured rung.
- [ ] **Done when:** the adapter passes the shared adapter contract test.

### Task 5.18 Tautulli adapter
- **Build:** `tautulli.stats` (metric × unit, days, minPlays).
- **Where:** `backend/crates/sources/tautulli`.
- **Subtasks:**
  - [ ] 1. Implement the popular/watched metric crossed with plays/duration unit.
  - [ ] 2. Support the `days`/`minPlays` parameters as JSON-Schema-validated inputs.
- [ ] **Done when:** the adapter passes the shared adapter contract test.

### Task 5.19 Radarr/Sonarr tag adapters
- **Build:** `radarr.tag` and `sonarr.tag` (instanceId, tagId).
- **Where:** `backend/crates/sources/radarr`, `backend/crates/sources/sonarr`.
- **Subtasks:**
  - [ ] 1. Implement both against multi-instance configuration.
  - [ ] 2. Declare `ordered: false`.
- [ ] **Done when:** both adapters pass the shared adapter contract test and reject
  `order.by: sourcePosition` at save time per Task 5.2.

### Task 5.20 Plex library modes adapter
- **Build:** `plex.library` (mode: recentlyAdded / recentlyReleased / recentlyReleasedEpisodes, limit),
  reading through the Phase 2 client — no writes.
- **Where:** `backend/crates/sources/plex_library`, consuming `backend/crates/plex`.
- **Subtasks:**
  - [ ] 1. Implement all three modes as read-only queries against the library-listing calls from
     Task 2.1.
  - [ ] 2. Declare `ordered: true`, `affirmativeEmpty: true`.
- [ ] **Done when:** the adapter passes the shared adapter contract test using the fake from Task 2.2 with
  no write call ever issued.

### Task 5.21 Lifecycle Coming Soon adapter
- **Build:** `lifecycle.comingSoon` (window, monitored scope, instance and tag filters), shaped now so
  the source interface is complete even though the lifecycle state machine it reads lands later.
- **Where:** `backend/crates/sources/lifecycle`.
- **Subtasks:**
  - [ ] 1. Implement the source's parameter schema and query shape against the lifecycle fields the registry
     already declares (Task 3.3's `lifecycle.*` catalogue).
  - [ ] 2. Stub the underlying data query against a fixture until the lifecycle state machine exists,
     documented as a known forward dependency, not a silent gap.
- [ ] **Done when:** the adapter validates against its JSON Schema and passes the shared adapter contract
  test against fixture data; its live query is revisited when the lifecycle phase lands.

### Task 5.22 Multi-source composition
- **Build:** the two meta sources — `multi` (N children with per-source priority and caps) and
  `hubReplacement` (shadows a native Plex hub, excluding placeholder items).
- **Where:** `backend/crates/sources/meta`.
- **Subtasks:**
  - [ ] 1. Implement `multi`, mapping its combine modes onto the merge strategy and order stage from
     Task 5.23.
  - [ ] 2. Implement `hubReplacement`, filtering on `library_items.is_placeholder`.
- [ ] **Done when:** a `multi` source composed of two children with different priorities produces a
  deterministic merged result, and `hubReplacement` never includes a placeholder item.

### Task 5.23 Merge, filter, and order — one pipeline
- **Build:** the single pipeline stage set used by every collection and every mode: merge (union/
  intersect/subtract, per-source cap, canonical-id dedupe), filter (exclusions, thresholds, attribute
  filters, mutual exclusion, time restrictions), order (deterministic by source position, release date,
  rating, or seeded random; franchise parts by release date).
- **Where:** `backend/crates/core` (pipeline).
- **Subtasks:**
  - [ ] 1. Implement merge with the three strategies, per-source caps, and canonical-identifier
     deduplication.
  - [ ] 2. Implement the `exclusions` table (global and per-definition scope) as the filter stage's
     exclusion source.
  - [ ] 3. Implement deterministic ordering with a mandatory seed for any non-deterministic mode, rotated on
     schedule, never per run.
  - [ ] 4. Route every collection and every mode — smart or manual — through this one pipeline, with no
     divergent quick and full paths.
- [ ] **Done when:** `I-SRC-7` passes — a definition combining any two source types produces the same
  output whether run as a "quick preview" or a full sync, because both paths call the same pipeline
  function; `I-SRC-2`, `I-SRC-3`, and `I-SRC-5` pass against their respective merge, filter, and order
  fixtures.

### Task 5.24 Interface pages this phase ships
- **Build:** source health and circuit-breaker status on the collection editor, a per-collection source
  list, and the preview panel showing partial results with per-source attribution.
- **Where:** `frontend/src/routes` (collection editor, preview panel); `backend/crates/api` (exposing
  `source_health` and `source_contributions`).
- **Subtasks:**
  - [ ] 1. Render each source's health state (closed/open/half-open, frozen, degraded) inline in the
     collection editor.
  - [ ] 2. Build the preview panel: resolved items with per-source attribution and counts, showing partial
     results with the failing source named rather than replacing the whole panel with an error.
  - [ ] 3. Generate source parameter forms from each source's published JSON Schema, including
     `x-control`-driven live-lookup pickers (an *arr tag selector populating from the configured
     instance) and `x-dependsOn` conditional fields.
- [ ] **Done when:** `I-EVID-1` and `I-EVID-4` pass — a preview where one of several sources fails still
  shows every successful source's results, with the failure named rather than the panel replaced by an
  error, verified against the fake's mid-pass-failure injection.

---

## Phase 6 — Collections in Plex

**Size:** `M`. The first phase where Afisharr changes the user's library.

**Exit invariants:** `I-SRC-6`, `I-IDEM-1`, `I-REV-5`, `I-REV-6`.

**Not here:** ordering or placement.

**`I-SRC-6` is the invariant to watch:** membership reconciliation must never destroy a collection to
change it. Deleting and recreating takes the rating key with it, and with it hub placement, adoption
state, artwork, and sort-title records.

### Task 6.1 Create and update managed collections
- **Build:** the `managed_collections` binding between a definition and the Plex collection(s) it
  produces, one row per `(definition, library, variant)`.
- **Where:** `backend/crates/core` (`managed_collections`); `backend/crates/plex` (create/update calls from Task 2.1).
- **Subtasks:**
  - [ ] 1. Implement `managed_collections` with `variant_key` for the multi-collection modes (per-franchise,
     per-person), keyed uniquely on `(definition_id, library_id, variant_key)`.
  - [ ] 2. Map `collection_mode` and `collection_sort` to Plex's integer values (`-1`/`0`/`1`/`2` and
     `0`/`1`/`2` respectively), documented at the point they are written.
  - [ ] 3. Enforce that `collection_sort` is never written on a smart collection, matching the save-time
     constraint from Task 3.4.
  - [ ] 4. Create the Plex collection on first sync; update its Plex-visible attributes on later syncs
     without deleting it.
- [ ] **Done when:** a collection's title or smart-filter attributes change across a resync while its
  `rating_key` stays identical, verified against the fake.

### Task 6.2 Reconcile membership
- **Build:** membership reconciliation as a diff against `managed_collection_items`, never a full
  rewrite.
- **Where:** `backend/crates/core` (reconciliation); `backend/crates/plex` (item add/remove/reorder calls).
- **Subtasks:**
  - [ ] 1. Diff the desired item set against `managed_collection_items`, the last reconciled membership.
  - [ ] 2. Issue only the add/remove/reorder calls the diff requires — no unconditional clear-and-rewrite.
  - [ ] 3. Self-heal by label and canonical identity when a rating key has churned (using Task 4.7's
     rebinding), rather than treating a churned item as departed.
  - [ ] 4. Never act on a failed or unaffirmed-empty source fetch — read the frozen-contribution state from
     Task 5.4 before writing.
- [ ] **Done when:** `I-SRC-6` passes — updating a collection's membership never deletes and recreates the
  Plex collection object, verified by asserting `rating_key` is unchanged across a membership change in
  the fake; `I-IDEM-1` passes — a second reconciliation run with unchanged inputs issues zero write
  calls.

### Task 6.3 Mutual exclusion
- **Build:** enforcement of `reconcile.mutualExclusionGroup` across collections sharing a group.
- **Where:** `backend/crates/core` (reconciliation, using `ix_managed_collection_items__item`).
- **Subtasks:**
  - [ ] 1. Before adding an item to a collection with a mutual-exclusion group, check existing membership
     across every other collection in that group via the item-indexed lookup.
  - [ ] 2. Resolve a conflict deterministically (first-writer-wins by definition order, or the rule the
     engine's pipeline already establishes) rather than allowing both memberships to stand.
- [ ] **Done when:** an item that qualifies for two collections sharing a mutual-exclusion group ends up in
  exactly one of them after reconciliation, deterministically reproducible across runs.

### Task 6.4 Self-healing for a vanished collection
- **Build:** recovery when a managed collection disappears from Plex between passes.
- **Where:** `backend/crates/core` (`managed_collections.missing_since`, `heal_count`).
- **Subtasks:**
  - [ ] 1. Detect a managed collection's absence via `missing_since` rather than assuming deletion means
     intent.
  - [ ] 2. Recreate it on the next pass, incrementing `heal_count` rather than silently resetting it.
  - [ ] 3. Surface a rising `heal_count` as a doctor-facing signal that something is fighting the
     reconciliation, not a healthy pattern.
- [ ] **Done when:** a collection deleted out-of-band in the fake is recreated on the next pass with its
  membership restored from `managed_collection_items`, and `heal_count` increments rather than resets.

### Task 6.5 Adoption of user-made collections, with consent
- **Build:** consent-gated adoption of collections the user created, per D-014.
- **Where:** `backend/crates/core` (adoption state, sort-title consent); `frontend/` (consent controls).
- **Subtasks:**
  - [ ] 1. Implement the per-library adoption-consent control, with a per-collection override for
     exceptions, and no global control at launch.
  - [ ] 2. Gate any sort-title modification of an adopted collection on `sortTitleConsent`, recording the
     original value before the first write.
  - [ ] 3. Restore the original sort title (value, presence, and lock state, not merely the value) on
     demotion or teardown.
- [ ] **Done when:** `I-REV-6` passes — no adopted collection's sort title is modified without recorded
  per-library or per-collection consent; `I-REV-5` passes — restoring a demoted adopted collection
  returns its original sort-title value, presence, and lock state exactly, not merely the value.

### Task 6.6 Interface pages this phase ships
- **Build:** the Collections list page and adoption-consent controls in Settings.
- **Where:** `frontend/src/routes` (Collections list; Settings › libraries adoption consent).
- **Subtasks:**
  - [ ] 1. List every collection with name, target libraries, item count, last-run outcome, next run, and
     current state (frozen/degraded/never-run/disabled/mid-sync), each legible without opening the
     collection.
  - [ ] 2. Build the per-library adoption-consent toggle and the per-collection override.
  - [ ] 3. Surface `heal_count` on the collection's row when it is rising, linking to the doctor surface this
     phase does not otherwise build.
- [ ] **Done when:** every collection's state in the list matches its `managed_collections` row with no
  need to open the collection to confirm it, verified by a test asserting list-state matches
  backend-recorded state across all five listed states.
## Phase 7 — Placement

Size `XL`. **Blocked on the spike track:** Q-015 (is the home surface one global sequence, or
per-library sequences merged at render?) and Q-014 (the real precision budget, calibrated against a
sequence of at least 2,500) must both land, with the PRD's placement design amended accordingly,
before this phase's tasks are authoritative. If Q-015 answers "per-library sequences merged at
render," the ordering model in Task 7.1 and the home-screen board in Task 7.9 both grow, and Task
7.9's downstream board work in Phase 13 grows with them. This is the highest-risk subsystem in the
product, and the scale target (D-030 — 200,000 items, 2,000 collections) is what makes it so: more
participants per surface means more precision spent per pass.

**Exit invariants:** I-CONV-1 through I-CONV-8, I-IDEM-2, I-IDEM-3, I-UX-4, I-UX-6.

**Not here:** rendering the posters that sit in the sequence — that is Phase 8.

**Why no ad-hoc retry loop is permitted anywhere in this phase:** I-CONV-3 and I-CONV-6 exist
specifically to forbid it. Rebalancing must be a planned step derived from gap-budget accounting, and
non-convergence must be a visible, recorded state — never a loop that silently keeps trying until it
stops.

### Task 7.1 Desired sequence and deterministic ordering

- **Build:** compute the desired sequence per surface from stored `position` values, tie-broken by
  participant ULID, deduplicated by identifier before any planning runs.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Read `placement_desired` rows into an ordered sequence per (surface, library).
  - [ ] 2. Sort by `(position, participant ULID)` ascending.
  - [ ] 3. Deduplicate by participant identifier before planning; collapse to the first occurrence and
     report the duplication as a doctor finding rather than silently dropping it.
  - [ ] 4. Property test: many participants sharing one position produce a byte-identical desired sequence
     across repeated computations.
- [ ] **Done when:** I-CONV-7 passes — the desired sequence is byte-identical across repeated
  computations for a fixture with duplicate positions, and a second pass over unchanged desired
  input emits zero moves.

### Task 7.2 Minimal-move planner

- **Build:** given the actual sequence read from Plex and the desired sequence, compute the longest
  subsequence of actual already in desired relative order (an LIS over desired-rank) and emit
  exactly `n − LIS` moves, each item moved once, `after` its already-correct predecessor.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement LIS-over-desired-rank on the actual sequence.
  - [ ] 2. Emit the move list: everything outside the LIS, in desired order, each placed `after` its
     correct predecessor.
  - [ ] 3. Property test over random permutation pairs: assert the emitted move count is never more than
     `n − LIS`, and that fewer is never observed either — `n − LIS` is the provable minimum.
- [ ] **Done when:** I-CONV-1 passes — the property test over generated permutation pairs finds no case
  where the planner emits more than `n − LIS` moves.

### Task 7.3 Gap-budget accounting

- **Build:** track subdivision depth per adjacent participant pair, per surface and library, in
  `placement_gaps`. An insertion destroys the parent pair and creates two child pairs, each
  inheriting `depth = parent.depth + 1`; depth resets to zero for a pair whose right-hand
  participant was just re-promoted with fresh spacing.
- **Where:** backend/crates/core (accounting), backend/crates/plex (the position observations that seed the estimate)
- **Subtasks:**
  - [ ] 1. Implement the `placement_gaps` split-on-insertion update: two child rows with incremented
     depth, parent row removed.
  - [ ] 2. Implement the `gapBudget` threshold check (default 8): a planned insertion into a gap whose
     depth exceeds the budget schedules a rebalance instead of attempting the move and handling a
     failure.
  - [ ] 3. Track `insertions` as a diagnostic counter alongside `depth`, without using it for the budget
     decision — a pair with high `insertions` and low `depth` is a different, interesting signal,
     not a trigger.
  - [ ] 4. Simulate deep, repeated subdivision of one region; assert the depth-based check fires while a
     naive raw-insertion-count check would not, because every "fresh" child pair would read a low
     count forever.
- [ ] **Done when:** a simulated precision-exhaustion fixture shows the rebalance scheduled by
  accounting strictly before any move is attempted against the exhausted gap.

### Task 7.4 The escalation ladder and anchor preference

- **Build:** the four-rung ladder — apply-and-verify, bounded per-item re-promotion, one full
  library rebalance scoped to re-promotable participants, and a terminal non-convergent state —
  bounded and deterministic, with every rung recorded before it executes and anchors never
  unpromoted at any rung.
- **Where:** backend/crates/core (ladder logic), backend/crates/plex (unpromote / promote / move calls)
- **Subtasks:**
  - [ ] 1. Rung 0: apply the minimal move plan from Task 7.2, one attempt, verify by read-back (Task 7.5).
  - [ ] 2. Rung 1: for each item still misplaced and re-promotable, unpromote and re-promote with fresh
     spacing, then re-plan the remainder; bound to `rebalanceLimit` items per pass (default 5).
  - [ ] 3. Rung 2: full rebalance of one library — unpromote every re-promotable participant, re-promote
     in desired order, then position anchors around them; scoped to non-anchor participants only,
     idempotent, and recorded before execution; bound to one library per pass.
  - [ ] 4. Rung 3: stop; mark the surface `non-convergent` with the specific unsettled items; surface it;
     the next pass does not re-enter rung 0 without new inputs.
  - [ ] 5. Anchor preference: when a plan has freedom between moving A after B or B before A, prefer
     moving the re-promotable participant over the anchor.
  - [ ] 6. Explicitly forbid the "reset all hub management and rebuild" fallback as an automated rung-2
     action; it exists only as an explicit, previewed operator action reserved for the doctor page
     (Phase 13).
- [ ] **Done when:** I-CONV-3, I-CONV-4, and I-CONV-6 all pass — a library whose participants are
  majority anchors completes every rung with zero unpromote calls against any participant the
  server reports as non-deletable; an unconvergeable fixture terminates at rung 3 with a recorded
  reason at every rung entered, and the next pass does not silently re-enter rung 0.

### Task 7.5 Verification and idempotency

- **Build:** every applied plan is verified by reading back the actual order; a pass compares a
  hash of (desired sequence, visibility set) against the last verified state and, on a match,
  issues zero API calls beyond the cheap verification read.
- **Where:** backend/crates/core, backend/crates/plex
- **Subtasks:**
  - [ ] 1. Compute `desired_hash` before planning; compare against `placement_surface_state.verified_hash`.
  - [ ] 2. On a hash match, skip planning and writes entirely — only the cheap verification read runs.
  - [ ] 3. On every applied move, read back the resulting order and compare to the plan; treat a mismatch
     as detected on the same pass, never deferred to the next.
  - [ ] 4. Record `placement_passes` rows: participant count, moves planned/applied, rebalances, rung
     reached, verification result, gap pressure.
- [ ] **Done when:** I-CONV-2 passes — a fake whose moves silently no-op past a gap budget has the
  mismatch detected on the same pass; and a second pass with unchanged desired sequence and
  visibility set issues zero writes and zero reads beyond the verification read.

### Task 7.6 Visibility as a principal set

- **Build:** apply visibility changes (owner home, shared-user home, library recommended) before
  ordering within a pass, against `placement_visibility` rows scoped to the seeded whole-audience
  principals at Tier 0.
- **Where:** backend/crates/core, backend/crates/plex
- **Subtasks:**
  - [ ] 1. Resolve each participant's visibility set from `placement_visibility` per surface.
  - [ ] 2. Apply visibility writes strictly before the ordering pass for that surface — a newly visible
     item must exist in the ordering space before its position is set; a newly hidden item consumes
     no move.
  - [ ] 3. Confirm the write path accepts a per-user principal row without a migration, even though only
     the `everyone` principal is ever written by the Tier 0 interface.
- [ ] **Done when:** a fixture pass shows visibility changes applied strictly before positioning writes
  for the same surface, and inserting a non-`everyone` principal row against the shipped schema
  succeeds with no migration.

### Task 7.7 Sort-title policy: capture, consent, and the round trip

- **Build:** the single function that computes and strips sort-title prefixes; capture of the
  original value, presence, and lock state from the raw Plex attribute before the first mutation;
  consent enforcement for adopted collections at library scope with a per-collection override;
  idempotent prefix application.
- **Where:** backend/crates/core (policy, the one function), backend/crates/plex (raw-attribute read, locked
  edit-endpoint write)
- **Subtasks:**
  - [ ] 1. Implement the single prefix compute/strip function; no second implementation exists anywhere
     in the codebase.
  - [ ] 2. Before the first mutation of any item's sort title, read the raw attribute — never the parsed,
     title-defaulted value — and record `was_present`, `was_locked`, `original_value` (as bytes),
     and `original_sha256` into `sort_title_originals`; refuse the mutation outright if capture
     fails.
  - [ ] 3. Resolve consent via `adoption_consents` (most-specific-wins: participant, then library, then
     global, defaulting to not-granted) before any write to a collection Afisharr did not create;
     refuse and raise a finding when unresolved.
  - [ ] 4. Implement promote/demote as a pair that restores the captured value, presence, and lock flag
     together, writing the field's value and its lock flag in the same edit call.
  - [ ] 5. Assert applying the prefix twice yields an identical string.
- [ ] **Done when:** the capture-before-mutate rule holds for every fixture — a failed capture blocks
  the mutation and leaves the item untouched — and consent resolution refuses a write with no
  resolvable row. (This capture rule is the same one Phase 8 claims as I-REV-1 once an equivalent
  path exists for base posters; the full byte-exact round trip this task enables is claimed as
  I-REV-3 in Phase 11, where it is exercised end to end.)

### Task 7.8 Self-healing and randomized ordering

- **Build:** the self-healing behaviour table — recreate a missing managed collection, rebind a
  re-keyed one, drop and report a user-deleted adoption, drop and report an absent native hub, and
  leave unrecognised participants alone and recorded — plus epoch-seeded randomization confined to
  a participant's own occupied positions.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement each row of the self-healing table as an explicit branch, not a generic catch-all.
  - [ ] 2. Implement `randomization_epochs`: the shuffle is seeded by `(epoch, surface)`; the epoch
     advances only on schedule or an explicit re-roll, never per pass.
  - [ ] 3. Constrain the shuffle to the position set already occupied by flagged participants.
  - [ ] 4. Fixture: inject unrecognised participants at random positions across every rung; assert
     survival and that their presence is recorded.
- [ ] **Done when:** I-CONV-5, I-IDEM-2, and I-IDEM-3 all pass — unrecognised participants survive
  every rung with their presence recorded; three passes within one epoch produce zero moves;
  advancing the epoch produces a different, reproducible order; and the multiset of positions
  occupied by randomized participants is unchanged before and after a shuffle, with no unflagged
  participant moved.

### Task 7.9 Interface: the home-screen board

- **Build:** ordering and visibility across the home surface and each library surface, shaped by
  the spike track's answers to Q-015 and Q-013; every applicable engine-specific state rendered
  explicitly, with a full keyboard path for every drag operation.
- **Where:** backend/crates/api (routes, SSE for pass status), frontend/
- **Subtasks:**
  - [ ] 1. Confirm Q-014 and Q-015 have landed with recorded measurements, and the PRD's placement design
     amended accordingly, before building against their answer.
  - [ ] 2. Render the applicable states explicitly: move pending verification, move failed verification,
     library non-convergent, rebalance scheduled or in progress, adopted collection lacking
     consent, anchor row, unrecognised participant present.
  - [ ] 3. A reordered row shows pending until read-back confirms it; it is never shown as settled before
     that.
  - [ ] 4. Never offer an operation on an anchor row that anchors cannot support (no unpromote /
     re-promote affordance).
  - [ ] 5. Implement drag-and-drop reordering and an equivalent keyboard path (layer list, move-up,
     move-down, move-to-position) that produce an identical resulting definition.
- [ ] **Done when:** I-UX-4 and I-UX-6 pass — a reorder against a fake with delayed verification shows
  pending and settles only after read-back returns, and the same reorder performed by drag and by
  keyboard produces an identical definition. I-CONV-8 also passes here: a reorder made through the
  board is written, verified, and still in place after the next scheduled sync runs against
  unchanged inputs, with that pass emitting no compensating moves.

---

## Phase 8 — Rendering

Size `XL`. Recommended before this phase begins: a further prior-art pass (Q-012) on poster and
overlay renderers, since the renderer is next. Two data-model amendments are owed before this
phase starts: asset-store sizing, so the render cache's cap (Task 8.4) is set against a real number
rather than a guess.

**Exit invariants:** I-RENDER-1 through I-RENDER-7, I-REV-1, I-REV-2, I-PERF-2, I-DATA-9.

**Not here:** lifecycle status overlays — they need Phase 9's state machine and land there.

**Why I-REV-1 and I-REV-2 must not be deferred within this phase:** the original poster is captured
before the first modification, byte-exactly, and restoring must return those exact bytes rather
than a re-render. Base posters are the only irreplaceable bytes in the product — once an overlay is
applied, Plex holds the overlaid version and the pristine original exists in exactly one place.
Getting capture wrong here is unrecoverable in a way nothing else in this phase is. Phases 8 and 9
are both `XL` and both touch the user's library; they are sequenced apart deliberately, and the
temptation to interleave lifecycle overlays into this phase should be resisted until Phase 9's
state machine passes its own tests.

### Task 8.1 Base-poster capture and provenance

- **Build:** capture the original poster once, before any modification, byte-exactly and
  content-addressed; quarantine any capture whose provenance is uncertain.
- **Where:** backend/crates/render (capture pipeline), backend/crates/plex (thumbnail read)
- **Subtasks:**
  - [ ] 1. On first touch of a library item, read the poster bytes via the item's recorded thumbnail key
     and hash to `sha256`; insert into the content-addressed asset store (deduplicated on the
     unique digest index), then into `base_posters` keyed by `library_item_id` — the primary key
     enforces that there is never a second base poster for an item.
  - [ ] 2. Refuse capture, and refuse the pending modification, if the read or hash fails.
  - [ ] 3. Mark a capture `is_suspect` when it occurs after Afisharr already wrote a poster for that item,
     or when the thumbnail key does not match the one previously recorded; exclude suspect bases
     from compositing and raise a doctor finding.
  - [ ] 4. Fixture every provenance-uncertain scenario: a restored backup, a reinstall, an item touched by
     another tool.
- [ ] **Done when:** I-REV-1 and I-RENDER-2 pass — a fault-injected capture failure blocks the
  modification entirely with the item left untouched, and every constructed provenance-uncertain
  scenario is quarantined, excluded from compositing, and raised as a finding.

### Task 8.2 Overlay element model and formatters

- **Build:** the element model (layers, positioned elements, per-element conditions), formatters as
  pure functions of value, locale, and arguments, and the null-skip rule for unresolved variables.
- **Where:** backend/crates/render
- **Subtasks:**
  - [ ] 1. Implement the element model: layers, positioned elements, per-element conditions bound to
     registry fields.
  - [ ] 2. Implement formatters as pure functions — no clock, no I/O, no randomness; an architectural
     test asserts none reach the clock or the network.
  - [ ] 3. An element whose bound value is null or unavailable is skipped entirely — never drawn blank,
     never drawn with a placeholder string; the render audit records why.
  - [ ] 4. A value-to-asset mapping table must declare a fallback at save time or is rejected.
- [ ] **Done when:** I-RENDER-4, I-RENDER-5, and I-RENDER-7 pass — every element type renders absent
  (not blank) under null and unavailable inputs, with the reason recorded; every formatter is
  idempotent under repeated identical calls and none reach the clock or network; and saving a
  mapping without a fallback is a structured save-time error.

### Task 8.3 Renderer and the compositing guarantee

- **Build:** one renderer implementation compositing pristine base plus current template plus
  current state, serving both the editor preview and the applied output; an overlay is never
  applied over an overlay.
- **Where:** backend/crates/render
- **Subtasks:**
  - [ ] 1. Implement a single render entry point used by both the preview endpoint and the apply job — no
     second renderer.
  - [ ] 2. Every render reads the base poster from the base-poster store, never from a previously
     rendered output.
  - [ ] 3. Architectural test: assert Afisharr's own render output is never read back as an input to a
     subsequent render.
- [ ] **Done when:** I-RENDER-1 and I-RENDER-3 pass — applying overlays, changing the template, and
  applying again yields a second render whose input digest equals the stored base digest and whose
  output is byte-identical to rendering the new template once from a clean library; rendering a
  template corpus through both entry points yields byte-identical output.

### Task 8.4 Render cache: key, cap, and eviction

- **Build:** the content-addressed render cache keyed by a hash of (base digest, template id,
  template version, state snapshot, renderer version), with a configured size cap and
  `last_used_at` eviction that never evicts an entry a database constraint protects.
- **Where:** backend/crates/render
- **Subtasks:**
  - [ ] 1. Implement the five-term key composition; mutating any one term must change the key.
  - [ ] 2. On an unchanged key, skip the upload entirely — a cache hit, no write.
  - [ ] 3. Implement eviction by `last_used_at` once the cache exceeds its configured cap; the eviction
     scan must skip rows a `RESTRICT` constraint protects (an output still bound to a live Plex
     upload).
  - [ ] 4. Drive renders past the cap in a fixture and assert the cap holds under sustained load.
- [ ] **Done when:** I-RENDER-6 and I-PERF-2 pass — mutating each of the five key components
  independently changes the key and forces a re-render, changing nothing produces a cache hit with
  no upload, the cache never exceeds its configured cap under load, and a render still bound to a
  live upload survives eviction.

### Task 8.5 Asset garbage collection

- **Build:** mark-and-sweep garbage collection across every table that can reference an asset, with
  a grace window before deletion and a filesystem reconciliation pass for unreferenced files.
- **Where:** backend/crates/render, backend/crates/core (nightly job wiring)
- **Subtasks:**
  - [ ] 1. Mark: walk every table that can reference an asset — base posters, the render cache, pack
     assets, local asset files, and definition-body asset references — and touch `last_used_at` on
     every asset reached.
  - [ ] 2. Sweep: delete asset rows older than the grace window (default 7 days) whose deletion is not
     blocked by a `RESTRICT` constraint; unlink the files.
  - [ ] 3. Reconcile: sample the filesystem for files with no asset row and delete them after the same
     grace window.
  - [ ] 4. Run the sweep against a database where every asset appears unreferenced; assert the
     constraint-protected classes survive because of the constraint, not the sweep's own logic —
     reference counting was rejected precisely because a drifting counter cannot be told apart from
     a correct one without the same full walk the mark phase already performs.
- [ ] **Done when:** I-DATA-9 passes — a full-unreferenced fixture leaves every pristine base poster,
  every render bound to a live upload, and every pack-required asset intact after the sweep runs.

### Task 8.6 Restoration: byte-exact, not re-rendered

- **Build:** removing overlays uploads the stored base poster exactly as captured — no resize,
  re-encode, format conversion, or crop.
- **Where:** backend/crates/render, backend/crates/plex
- **Subtasks:**
  - [ ] 1. Implement the reset path as a direct upload of the stored base asset's bytes; it never passes
     through the renderer.
  - [ ] 2. Fixture a base poster of an unusual aspect ratio and a non-default format; apply overlays, then
     reset.
- [ ] **Done when:** I-REV-2 passes — the bytes Plex serves after reset hash-equal the captured
  original, for both the unusual-aspect and non-default-format fixtures.

### Task 8.7 Interface: the template editor (overlay and poster)

- **Build:** the layered canvas editor — layer list, canvas, element inspector, per-element
  conditions, live preview — sharing Task 8.3's renderer so preview and applied output cannot
  drift.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Layer list, canvas, element inspector, and per-element condition editing bound to the field
     registry.
  - [ ] 2. Live preview calling the same render entry point as production.
  - [ ] 3. Handle the states beyond the universal ones: a referenced font or icon asset missing; an
     element's bound field unavailable on this server; a preview item with no media (so `media.*`
     resolves null); a pack-origin template read-only until forked.
- [ ] **Done when:** I-RENDER-3's byte-equality property holds through this page specifically — a
  corpus of templates rendered via the editor preview and via the applied-output path, both reached
  through this page's API calls, are byte-identical.

---

## Phase 9 — Lifecycle

Size `XL`. The differentiator, and the component with the strictest correctness obligations,
because it writes files into the user's library and deletes them again. Two open items gate this
phase: Q-005 (retention-window calibration) is promoted to "before this phase ships" by the PRD's
precision-budget section, and a second data-model amendment — the retention cap — is owed before
this phase starts, alongside the asset-store sizing amendment owed before Phase 8.

**Exit invariants:** I-LIFE-1 through I-LIFE-4, I-EVID-2, I-EVID-3, I-EVID-5, I-EVID-6, I-EVID-7,
I-DATA-1, I-SEC-4, I-PERF-3.

**Not here:** acquisition — the download-stack write path is Phase 10.

**Why the evidence group concentrates here:** this milestone writes files into the user's library,
which is why I-SEC-4 and the evidence invariants sit in this phase rather than being spread thin
across the ones that merely read. I-EVID-2 is the load-bearing one of the group: a placeholder is
deleted only under an allowlisted trigger carrying its evidence into the audit record. Phases 8 and
9 are both `XL` and both touch the user's library; resist interleaving lifecycle overlays into
Phase 8 until this phase's state machine passes its own tests, in isolation, first.

### Task 9.1 The subject and the four-axis state machine

- **Build:** the lifecycle subject model — phase, acquisition, presence, and (for shows) production
  as independent axes; the fixed six-step evaluation pass; the seven guards; bidirectional phase
  transitions that never destroy on a release date moving backwards.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement the subject model against the lifecycle-subjects table, including its identity
     uniqueness (library, id space, id value, season) and the whole-title/season split from D-025.
  - [ ] 2. Implement phase resolution: bidirectional transitions, the release-date priority ladder per
     media type (digital, then physical, then a theatrical estimate for movies; first-air or
     next-episode for shows; a season's own air date for season subjects, never inherited from the
     show), and recording of which basis resolved the date.
  - [ ] 3. Implement the fixed six-step pass: refresh evidence, assess staleness, re-evaluate references,
     compute target axes, emit transitions subject to guards, execute side effects.
  - [ ] 4. Implement the seven guards exactly: a stale subject transitions on no axis; presence reaches
     `RemovalPending` only through the allowlist; `Real` is set only from positive Plex
     confirmation; a destructive transition justified solely by a theatrical-estimate date is
     refused under `strictDates`; a referenced subject is never removed for departure; two
     transitions of one axis in one pass fails loudly rather than picking one; an ambiguous
     canonical match enters `Ambiguous` and is never acted on until a human resolves it.
  - [ ] 5. Table-test the full product of axes and triggers: every legal `(from, to, trigger)` triple
     succeeds; every triple outside that set is refused with a named reason.
  - [ ] 6. Generate date sequences moving forward and backward across every phase boundary; assert zero
     destructive actions arise from a backwards move, and that a delayed title keeps its
     placeholder.
- [ ] **Done when:** I-LIFE-1 and I-LIFE-3 pass — the table test over the full product of axes and
  triggers shows the legal set closed and enumerable, with every legal triple exercised and every
  off-list triple refused; and the generated forward/backward date sequences produce zero
  destructive actions from any backwards move.

### Task 9.2 Both granularities and placeholder ownership

- **Build:** opt-in per-show season subjects, a whole-title subject that always exists for a tracked
  show, and the ownership-by-absence rule that stops the two granularities from ever contending for
  one placeholder path.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement `seasonGranularity` resolution (off by default, instance/definition default, per-show
     override) and season-subject creation only for seasons whose air date falls inside
     `countdownWindow`.
  - [ ] 2. Implement placeholder ownership: the whole-title subject owns the placeholder while the show is
     absent; a season subject owns its own placeholder only once the show is `Real` and that season
     is absent; while the show is absent and seasons are tracked, only the whole-title subject
     writes anything.
  - [ ] 3. Implement production as a show-level fact a season subject reads from its parent rather than
     recomputing.
  - [ ] 4. Table-test the product of (show presence, season presence, `seasonGranularity`), including
     running the season subject's evaluation both before and after the show subject's, within the
     same pass.
- [ ] **Done when:** I-LIFE-4 passes — across the full product table, at most one subject holds a
  placeholder path per pass, the path sets of the two granularities are disjoint, and the ordering
  variant produces the same outcome both ways.

### Task 9.3 Derived status

- **Build:** the pure mapping from (phase, acquisition, presence, production) to the single status
  label overlay packs render, exposed under a `lifecycle.*` field-registry namespace.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement the status table as a pure function — no I/O, no clock reads beyond the evaluation
     clock.
  - [ ] 2. Register `lifecycle.status`, `lifecycle.phase`, `lifecycle.acquisition`, `lifecycle.presence`,
     `lifecycle.production`, `lifecycle.releaseDate`, `lifecycle.releaseDateBasis`,
     `lifecycle.daysUntilRelease`, `lifecycle.daysSinceRelease`, `lifecycle.isStale`,
     `lifecycle.isPlaceholder`, and `lifecycle.seasonNumber` as field-registry entries with declared
     types and legal operators.
  - [ ] 3. Table-test every reachable composite.
- [ ] **Done when:** I-LIFE-2 passes — the table test over every reachable composite shows the mapping
  total and single-valued: no composite maps to none, none maps to two.

### Task 9.4 Placeholder materialisation, discovery, and title repair

- **Build:** the intend/execute/confirm sequence for creating and removing placeholder files, the
  three-layer marker scheme that lets hub replacement and sweeps identify a placeholder without
  parsing filenames, and title repair for a placeholder whose display title has drifted from
  evidence.
- **Where:** backend/crates/core (intent orchestration, filesystem writes, marker application), backend/crates/plex
  (label write, scan trigger, rating-key discovery)
- **Subtasks:**
  - [ ] 1. Implement the intent lifecycle: Intend (transactional, moves presence to
     `PlaceholderPending` / `RemovalPending`) → Execute (file operation plus Plex scan or refresh) →
     Confirm (verify the observable result, settle presence).
  - [ ] 2. Implement the placeholder marker as three layers: a Plex label as the authoritative runtime
     marker, the rating-key-to-subject database binding as the authoritative durable marker, and an
     edition filename tag as a hint only, never load-bearing for correctness.
  - [ ] 3. Implement discovery: after a scan, bind the newly indexed placeholder's rating key to its
     subject by label and canonical identity.
  - [ ] 4. Implement title repair: when evidence's resolved title diverges from the placeholder's current
     title, rewrite it through the same intend/execute/confirm sequence.
  - [ ] 5. Enforce writes only under configured placeholder roots: canonicalise and symlink-resolve the
     target path before the containment check, refuse anything resolving outside an enabled root,
     and settle the subject to a reported error rather than `Placeholder` on refusal.
  - [ ] 6. Both create and remove operations are idempotent: creating an existing placeholder is a no-op;
     deleting an absent one is a no-op.
  - [ ] 7. Crash-inject between every pair of intend/execute/confirm steps, for every intent kind, and
     assert one startup pass — re-driving every open intent from the execute step — reaches a
     consistent state.
- [ ] **Done when:** I-DATA-1 and I-SEC-4 pass — crash injection between every step pair, for every
  intent kind, converges to a consistent state after one startup pass; and a traversal-and-symlink
  corpus applied to placeholder writes shows no file created outside a configured root, with the
  subject settling to a reported error rather than `Placeholder`.

### Task 9.5 The destructive-action allowlist and the audit record

- **Build:** the seven-trigger allowlist as the only path that can delete a placeholder, each
  carrying its evidence into an append-only transition log; the reference-counting rule under which
  only a count reaching zero departs a subject.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement the seven triggers (`Materialized`, `Departed`, `Retired`, `FilteredOut`, `Disabled`,
     `Manual`, `Reaped`), each writing a transition row with `is_destructive` set and a complete
     evidence array — relying on the schema constraint (built in Phase 0) that makes an off-list
     trigger unrepresentable at the database level.
  - [ ] 2. Recompute the reference count each pass from the collections that currently resolve to the
     subject; never increment or decrement it ad hoc. The reason a reference was dropped — filter
     fail, removed from source, definition deleted — is written directly into the transition's
     evidence.
  - [ ] 3. Implement `Real` as reachable only from a positive Plex media-part confirmation, never from
     provider or `*arr` data alone.
  - [ ] 4. Implement staleness as a full stop: a subject whose evidence could not be refreshed keeps every
     axis unchanged this pass.
  - [ ] 5. Implement the not-evidence rule for upstream absence: a provider that stops serving a
     previously resolved subject marks it stale, never `Departed`.
- [ ] **Done when:** I-EVID-2, I-EVID-3, I-EVID-5, I-EVID-6, and I-EVID-7 all pass — a property test
  over generated evidence sequences shows no placeholder deleted without an allowlisted trigger and
  complete evidence, backed by a database-level test that an off-list destructive insert is
  rejected by the schema; `Real` is unreachable from any evidence set lacking Plex media-part
  confirmation; N collections referencing one subject in interleaved add/remove order leave exactly
  one file existing throughout, removed only at zero references; a provider fake that stops serving
  a previously resolved subject leaves state unchanged and marks it stale with no intent created;
  and every single-provider-failure fixture leaves all four axes unchanged.

### Task 9.6 Acquisition policy scaffolding (evaluation only, no writes)

- **Build:** the eligibility-gate and routing computation the lifecycle pass evaluates alongside the
  state machine, stopping short of any write to a download stack — that write path belongs to
  Phase 10.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Implement the eligibility gates (position cap, minimum year, minimum ratings, genre / country /
     language / keyword filters, collection enablement) as pure evaluators over evidence.
  - [ ] 2. Resolve TV shaping (maximum seasons, per-show cap, grab order) to a concrete season list,
     without submitting it anywhere.
  - [ ] 3. Compute and record the routing decision (request versus direct-to-`*arr`) without executing it;
     Phase 10 adds the client calls that act on it.
- [ ] **Done when:** gate evaluation is deterministic and reproducible from evidence alone across a
  fixture corpus. (The full reproducibility guarantee is claimed as an exit invariant in Phase 10,
  once the write path exists to reproduce against.)

### Task 9.7 Scale validation: the pass stays inside the memory ceiling

- **Build:** a nightly-lane scale test running a full reconciliation-through-lifecycle pass over the
  200,000-item, 2,000-collection scale target (D-030), asserting process RSS never exceeds 1 GB.
- **Where:** backend/crates/core (streaming discipline across the full pass), the nightly CI lane (D-035)
- **Subtasks:**
  - [ ] 1. Assemble scale fixtures at 200,000 items across several libraries and 2,000 collections,
     reusing the streaming batch discipline established for the item cache earlier in the plan.
  - [ ] 2. Run a full pass — sources through lifecycle — under a memory-bounded harness; assert peak RSS
     stays under 1 GB. Elapsed time is recorded for trend, not asserted.
  - [ ] 3. Wire the run into the nightly lane.
- [ ] **Done when:** I-PERF-3 passes — the memory-bounded nightly run over the scale fixtures completes
  without process RSS exceeding 1 GB.

### Task 9.8 Interface: the Lifecycle page

- **Build:** the upcoming-by-date view, the materialised-placeholders view, and the
  stale-placeholder surface (D-011) with bulk removal; the acquisition-activity view ships empty
  here and is completed in Phase 10.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Upcoming-by-date view ordered by each subject's resolved release date.
  - [ ] 2. Placeholders-currently-in-the-library view; each entry shows why it exists (its transitions)
     and which definitions reference it.
  - [ ] 3. Stale-placeholder view: every placeholder past its retirement window under `retirePolicy`, with
     bulk removal — the surface that turns "keep" into a managed choice per D-011, rather than
     silent accumulation.
  - [ ] 4. Handle the states beyond the universal ones: a stale subject (evidence could not refresh), an
     ambiguous match blocking action, an intent pending, a retire window expired.
- [ ] **Done when:** every placeholder currently in the library is reachable from this page, each entry
  legibly shows why it exists and which definitions want it, and the stale-placeholder view's
  bulk-removal count matches the set defined by the retirement-window query on the same fixture.

---

## Phase 10 — Acquisition

Size `M`.

**Exit invariants:** I-ACQ-1 through I-ACQ-5.

**Why this is not merged into Phase 9:** I-ACQ-1 is the reason. Acquisition must never act on
unverifiable state, and an unreachable download stack makes every item in every collection look
absent. That deserves its own milestone and its own failure fixtures, separate from the lifecycle
state machine's own correctness obligations.

### Task 10.1 Radarr and Sonarr write clients

- **Build:** typed write clients for Radarr and Sonarr — add movie or series, quality profile, root
  folder, tags, monitor mode, search-on-add, season folder — reusing the instrumented HTTP client
  and circuit breaker already built for these same products as read-only sources.
- **Where:** backend/crates/sources
- **Subtasks:**
  - [ ] 1. Extend each adapter with a write-capable client surface distinct from its read-only
     collection-membership surface.
  - [ ] 2. Implement per-subject override resolution: instance, quality profile, root folder, tags,
     monitor mode, search-on-add, season folder.
  - [ ] 3. Reuse the circuit breaker and its persisted state for the write path — a broken instance must
     not receive write attempts either.
- [ ] **Done when:** a fixture write to each client, against the adversarial fake extended with write
  endpoints, produces the expected instance-side call with the resolved overrides recorded.

### Task 10.2 Overseerr request client

- **Build:** a typed client for creating Overseerr requests, distinct from the per-user / global
  read-only source built earlier.
- **Where:** backend/crates/sources
- **Subtasks:**
  - [ ] 1. Implement the request-creation call and response handling.
  - [ ] 2. Route it through the same instrumented-client and breaker infrastructure as the read path.
- [ ] **Done when:** a fixture request against the fake Overseerr endpoint succeeds and is recorded with
  its external reference id.

### Task 10.3 Acquisition policy: gates, routing, and reproducibility

- **Build:** complete the eligibility gates and routing decision scaffolded in Phase 9 into an
  executable policy — request-or-direct routing, never both for one subject — with a decision
  record complete enough to recompute the decision from the record alone.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Evaluate all eligibility gates and record every gate's verdict, including the ones that
     passed.
  - [ ] 2. Resolve routing to exactly one of request or direct-to-`*arr` per subject.
  - [ ] 3. Resolve TV shaping to a concrete season list and record it.
  - [ ] 4. Write one acquisition-decision row per decision: inputs, every gate and its verdict, resolved
     overrides, the season list, policy version, outcome, and the external reference of the created
     record.
  - [ ] 5. Implement the partial-availability rule: "already present" resolves to what is specifically
     missing — a series with 1 of 60 episodes present is not a series that is present, and the
     missing seasons are requested while the present one is not.
  - [ ] 6. Implement the missing-configured-target rule: a definition naming an instance, quality
     profile, root folder, or library that no longer exists fails with a message naming both the
     configured and the found value; the default is never substituted.
  - [ ] 7. Build a replay harness that loads one decision row with no access to live state and recomputes
     the same decision.
- [ ] **Done when:** I-ACQ-2, I-ACQ-3, I-ACQ-4, and I-ACQ-5 all pass — a series fixture with 1 of 60
  episodes present requests the missing seasons and not the present one; removing each configured
  target in turn produces refusal naming both values with zero calls to any substitute; replaying
  every recorded decision in a generated corpus from the record alone reproduces identical outcomes;
  and the recorded gate set equals the configured gate set for every decision.

### Task 10.4 Unverifiable-state freeze

- **Build:** the rule that an unreachable download stack freezes the acquisition axis, rather than
  being treated as permission to add again.
- **Where:** backend/crates/core
- **Subtasks:**
  - [ ] 1. Detect download-stack unreachability per instance before evaluating gates for any subject
     targeting it.
  - [ ] 2. On unreachability, freeze the acquisition axis for affected subjects, emit zero add/request
     calls, and record the degradation.
  - [ ] 3. Fixture: make the download stack unreachable mid-pass; assert the outbound acquisition call
     count is exactly zero.
- [ ] **Done when:** I-ACQ-1 passes — the mid-pass unreachability fixture shows zero outbound
  acquisition calls, asserted at the HTTP client boundary, the acquisition axis unchanged, and a
  recorded degradation.

### Task 10.5 Interface: acquisition activity

- **Build:** complete the Lifecycle page's acquisition-activity view left empty in Phase 9, and add
  the acquisition-policy section (eligibility gates, routing, TV shaping) to the collection editor's
  lifecycle section.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Acquisition-activity view: recent decisions, their outcome, and their route.
  - [ ] 2. Collection editor's lifecycle section gains the gate, routing, and TV-shaping controls,
     validated against the definition layer.
- [ ] **Done when:** every decision visible in the acquisition-activity view links to the collection
  that produced it and to the Task 10.3 replay for that row.

---

## Phase 11 — Teardown

Size `L`. The acceptance test for reversibility (D-022), built at launch rather than promised for
later.

**Exit invariants:** I-REV-3, I-REV-4.

**Why I-REV-4 is the single most important test in the product:** it is placed in the nightly lane
(D-035) because it needs a fully populated fixture library and a complete apply-then-reverse cycle.
Those fixtures are worth building earlier than this phase and reusing here — they have been in use
since Phase 6, extended for placement, rendering, and lifecycle as each landed.

**Why I-REV-3 catches what a naive teardown misses:** a sort title has three properties — value,
presence, and lock state — and restoring only the value fails silently and permanently.

### Task 11.1 Teardown fixtures

- **Build:** confirm the fully populated fake-Plex fixture library, built incrementally from
  Phase 6 onward, covers every subsystem this phase must restore before the orchestrator (Task 11.2)
  is exercised against it.
- **Where:** shared test harness
- **Subtasks:**
  - [ ] 1. Confirm the fixture library carries collections, overlays, placeholders, and placement state
     together, not just each in isolation.
  - [ ] 2. Snapshot server state before any Afisharr write, as the baseline the teardown result is
     compared against.
- [ ] **Done when:** the fixture snapshot captures every field Task 11.2's allowlist comparison will
  need.

### Task 11.2 The teardown orchestrator

- **Build:** a first-class, resumable teardown operation restoring every base poster, sort-title
  prefix with its lock state, applied label, managed collection and placeholder, and native hub
  placement; resumable after a crash or a cancel; reports everything it could not restore, by name.
- **Where:** backend/crates/core (orchestration, resumability), backend/crates/plex (restore calls), backend/crates/render
  (base-poster restore, reusing Task 8.6)
- **Subtasks:**
  - [ ] 1. Restore every base poster via the byte-exact reset path built in Phase 8.
  - [ ] 2. Strip every applied sort-title prefix and restore value, presence, and lock state together, via
     the round trip built in Phase 7.
  - [ ] 3. Remove every applied label; a failed removal is reported, never silently ignored, and leaves
     the item out of "reset" bookkeeping so the next pass retries it.
  - [ ] 4. Delete every managed collection and placeholder Afisharr created; restore native hub placement
     to its pre-Afisharr state where recoverable.
  - [ ] 5. Persist teardown progress so the whole operation is resumable: a crash or an explicit cancel
     mid-run leaves a state a resumed run finishes correctly, without redoing completed restores or
     skipping unfinished ones.
  - [ ] 6. Produce a final report naming, by object, everything that could not be restored.
- [ ] **Done when:** I-REV-4 passes — an integration test against the fake Plex applies a full sync
  cycle (overlays, placeholders, placement), runs teardown, and the resulting snapshot matches the
  pre-sync snapshot under an explicit allowlist of legitimately changed fields that is empty for
  artwork bytes, sort titles, lock states, and labels; a second variant kills the process
  mid-teardown and asserts a resumed run reaches the same end state.

### Task 11.3 The sort-title round trip, exactly

- **Build:** the specific regression I-REV-3 exists for — value, presence, and lock state restored
  independently and correctly for items that never had a sort title, had a locked one, and had an
  unlocked one.
- **Where:** backend/crates/core, backend/crates/plex
- **Subtasks:**
  - [ ] 1. Build three fixtures: no sort title, a locked sort title, an unlocked sort title.
  - [ ] 2. Round-trip each through promote, then teardown's demote.
  - [ ] 3. Assert value, presence, and lock flag independently — a value-only comparison must not pass
     this test even where it would look correct.
- [ ] **Done when:** I-REV-3 passes on all three fixtures independently for value, presence, and lock
  state.

### Task 11.4 Label-removal failure reporting, verified inside teardown

- **Build:** confirm the label-removal failure reporting built earlier is not bypassed by the
  teardown path specifically.
- **Where:** backend/crates/core, backend/crates/plex
- **Subtasks:**
  - [ ] 1. Fault-inject a label-removal failure during a teardown run.
  - [ ] 2. Assert a doctor finding is raised, the item is not recorded as reset, and the next pass retries
     the removal.
- [ ] **Done when:** the fault-injected fixture, run through teardown specifically, shows a finding
  raised and a retry on the next pass, with the item excluded from "reset" bookkeeping until it
  succeeds.

### Task 11.5 Interface: the Teardown page

- **Build:** preview, typed confirmation, resumable progress display, and a final report — the
  preview's counts matching what teardown actually does.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Preview: compute and display the exact named objects and counts teardown will affect, before
     any write.
  - [ ] 2. Typed confirmation gate before execution.
  - [ ] 3. Live resumable progress over SSE; a page reload mid-run reflects the actual in-progress state,
     not a restarted one.
  - [ ] 4. Final report listing anything that could not be restored, by name.
- [ ] **Done when:** the preview's counts and named objects match teardown's actual effect against the
  same fixture (the general preview-equals-effect property is claimed as I-UX-5 in Phase 13, applied
  here specifically to teardown); and interrupting a run mid-teardown leaves a library that a
  resumed run, driven from this page, finishes correctly — the observable behaviour I-REV-4 requires.

---

## Phase 12 — Backup, restore, and upgrade

Size `M`.

**Exit invariants:** I-SEC-5, I-SEC-6.

**Why the credential-less restore path is the common case, not the edge case:** excluding
`secrets.key` from the backup is the default (D-033). It must lose credentials and nothing else.

### Task 12.1 Scheduled backup via SQLite's online backup API

- **Build:** nightly backup to D-033's scope — `afisharr.db` always, base-poster assets always,
  `secrets.key` opt-in and default off, render cache / HTTP cache / placeholder stubs / logs never —
  captured using SQLite's online backup API, never a file copy, retaining seven daily and four
  weekly archives.
- **Where:** backend/crates/afisharr (scheduler wiring, backup command), backend/crates/core (scheduling)
- **Subtasks:**
  - [ ] 1. Implement the online-backup-API capture path for the database, reusing the mechanism already
     required for the pre-migration backup built in Phase 0.
  - [ ] 2. Implement incremental asset backup: sync new digests only, since base-poster assets are
     content-addressed and immutable — a digest never changes meaning, which is what makes the
     incremental sync safe.
  - [ ] 3. Implement the retention schedule: seven daily, four weekly; exclude the render cache, HTTP
     cache, placeholder stubs, and logs from the archive entirely — excluding the render cache alone
     removes roughly half the bytes and all of the churn.
  - [ ] 4. Make `secrets.key` inclusion an explicit opt-in, default off.
- [ ] **Done when:** a scheduled run produces an archive matching D-033's scope exactly, verified by
  enumerating archive contents against the scope table.

### Task 12.2 Backup self-verification

- **Build:** the nightly job verifies the archive it just wrote — integrity check on the database
  copy, digest sample against asset files — and raises a doctor finding on failure. A backup that
  has never been restored is a hypothesis, not a guarantee.
- **Where:** backend/crates/afisharr, backend/crates/api (finding surface)
- **Subtasks:**
  - [ ] 1. Run an integrity check against the backed-up database copy immediately after capture.
  - [ ] 2. Sample asset digests in the archive against their recorded values.
  - [ ] 3. Record the verification timestamp on success; raise a doctor finding on any failure.
- [ ] **Done when:** I-SEC-6 passes — a corrupted written archive fails verification and raises a
  finding; a valid archive passes and records its verification timestamp.

### Task 12.3 Restore as a first-class operation

- **Build:** restore that refuses to run against a live instance, verifies the archive before
  touching anything, refuses a schema version newer than the binary, restores the database then the
  assets, reconciles asset rows against the filesystem, and reports what could not be restored.
- **Where:** backend/crates/afisharr
- **Subtasks:**
  - [ ] 1. Refuse to restore into a running instance; require a stop first.
  - [ ] 2. Verify the archive before any write: database integrity, schema version, a sample of asset
     digests against their files.
  - [ ] 3. Refuse a backup whose schema version is newer than the binary, naming both versions — reusing
     the same refusal the startup sequence already applies to migrations.
  - [ ] 4. Restore the database, then the assets.
  - [ ] 5. Reconcile: every asset row is checked against the filesystem; a row with no file is marked
     missing rather than deleted — an asset row with a missing file is recoverable (recapture from
     Plex); a deleted row is not.
  - [ ] 6. Report what could not be restored, by name and count.
- [ ] **Done when:** the restore path exercises all six steps against a corpus of archives (valid,
  wrong-schema, simulated mid-write invalid) with the correct refusal or completion in each case.

### Task 12.4 The credential-less restore path

- **Build:** the case that will actually happen — restore without `secrets.key` — proven graceful:
  every definition, collection, placement record, and base poster restores; the credential rows are
  present and marked undecryptable; nothing is deleted on the assumption that an unreadable
  credential is an absent one; the interface walks the operator through re-authenticating each
  affected integration.
- **Where:** backend/crates/afisharr, backend/crates/api, frontend/ (integration re-authentication flow)
- **Subtasks:**
  - [ ] 1. Implement the undecryptable-credential marking on restore, rather than deletion or silent
     substitution.
  - [ ] 2. Surface each affected integration as needing re-authentication, with a guided flow.
  - [ ] 3. Assert full row counts across every table after a credential-less restore, compared against the
     pre-backup state.
- [ ] **Done when:** I-SEC-5 passes — back up, restore without the key, and assert full row counts
  across every table, every integration reporting a need to re-authenticate, and nothing deleted.

---

## Phase 13 — Onboarding, packs, doctor, observability

Size `L`.

**Exit invariants:** I-UX-5, I-UX-8, I-UX-10, I-DEF-4, I-DEF-8.

**Why I-UX-8 is a timed test, not a review:** onboarding reaches a populated library within the
target the PRD states, or it fails — asserted on step count and blocking calls, not on a reviewer's
impression.

**Why I-UX-10 lands here and not in Phase 1:** Task 1.12 builds the derived resume step, but a
derivation is only testable against the steps it derives, and those ship in this phase. The claim
gate itself is claimed as I-SEC-8 in Phase 1, where it guards the first-run page.

### Task 13.1 The setup wizard, including report-and-adopt-nothing

- **Build:** the resumable, re-runnable eight-step wizard journey on top of the claim mechanism from
  Task 1.12, including D-026's report-and-adopt-nothing step — it lists existing collections per
  library, explains what adoption is, states plainly that Afisharr leaves those collections alone,
  and links to where adoption happens, with no bulk-adopt control anywhere in the wizard.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Build the eight wizard steps against the onboarding journey — Claim, Admin, Plex, Libraries,
     Integrations, Packs, Report, Review — resumable mid-flow and re-runnable later without
     destroying existing configuration.
  - [ ] 2. Wire every step to the derived resume endpoint from Task 1.12 rather than to any client-held
     step index, and write `packs` and `existingCollections` into `instance.setup_acked_steps` as
     their steps complete — both complete by acknowledgement, so nothing else marks them done.
  - [ ] 3. Build the Blocked treatment for the claimed-elsewhere case: the shared Blocked component from
     Task 1.8, carrying the claim's expiry as a local time. No gate is relaxed to produce it.
  - [ ] 4. Build the re-run mode reached from Settings: no token, no claim, current values shown as current
     values, and completion that destroys nothing the operator did not change.
  - [ ] 5. Implement the report-and-adopt-nothing step per D-026: enumerate existing collections per
     library, explain adoption in plain language, link to the per-library / per-collection adoption
     control built in Phase 6 and Phase 7, and provide no bulk-adopt affordance anywhere in the
     wizard — the operator with many hand-made collections is the one for whom one click is most
     tempting and most alarming, and a first impression is the worst moment to spend that trust.
  - [ ] 6. Script the first-run journey against a fixture server end to end: claim with the console token,
     create the admin, connect Plex, select libraries, complete the wizard, run the first sync.
  - [ ] 7. Script the two interruption paths against the same fixture: close the tab mid-wizard and resume
     at the same step, and restart the process mid-wizard and resume through admin recovery with the
     console token already dead.
- [ ] **Done when:** I-UX-8 passes — the scripted first-run journey against a fixture server completes
  the wizard and the first sync inside the target stated in the PRD, asserted on step count and
  blocking calls rather than wall time; and I-UX-10 passes — for each step, a database seeded at that
  step's evidence level derives that step, and a client requesting any other step index is answered
  with the derived one.

### Task 13.2 Packs: install, upgrade, fork

- **Build:** the pack installer over the manifest format (definitions, assets, declared variables),
  upgrade that replaces pack-origin documents while leaving forks untouched and reported as behind
  upstream, and forking a pack-origin definition into the user's own namespace.
- **Where:** backend/crates/packs
- **Subtasks:**
  - [ ] 1. Implement manifest parsing and asset/definition installation from file, URL, or repository.
  - [ ] 2. Implement variable resolution and template expansion at install time (D-044): a parameterized
     template plus declared variables materializes into concrete, unparameterized definition
     documents — one row per enumeration member for an expansion over a registry enumeration, with
     no substitution syntax surviving into storage.
  - [ ] 3. Store the resolved variable values so a later pack upgrade can re-materialize the definitions
     the user has not forked.
  - [ ] 4. Implement fork: duplicate a pack-origin document into the user's own namespace; forked
     documents are thereafter untouched by upgrade.
  - [ ] 5. Implement upgrade: replace every non-forked pack-origin document; leave forks alone; report
     forks as behind upstream.
- [ ] **Done when:** I-DEF-4 and I-DEF-8 pass — forking a pack definition then upgrading the pack
  leaves the fork unchanged, updates the non-forked pack document, and reports the drift; and
  installing a pack whose manifest declares variables and an expansion over a registry enumeration
  produces one row per member, with no stored body containing substitution syntax and every stored
  body passing full definition-layer validation unaided — then upgrading re-materializes the
  unforked definitions while forks stay untouched.

### Task 13.3 The doctor page

- **Build:** the consolidated surface for everything that needs a decision — open findings by
  severity, configuration and connectivity checks, ambiguous-match resolution (the authoritative
  surface per D-013), suspect base posters, orphan-sweep candidates, non-convergent libraries,
  asset-store reconciliation, and the explicitly dangerous operator actions (full hub reset, forced
  re-discovery, cache rebuild), each behind a preview of what will be lost.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Aggregate findings by severity, deduplicated per check and subject, with first-seen and
     last-seen timestamps and an acknowledge action that suppresses without resolving.
  - [ ] 2. Ambiguous-match resolution: list every `Ambiguous` subject and canonical-id collision; a
     resolution recorded here persists and unblocks the subject on the next pass without
     re-detection.
  - [ ] 3. Suspect base posters, orphan-sweep candidates (default resolution: reported, never
     auto-deleted), non-convergent libraries, and asset-store reconciliation, each reading the
     surfaces built in Phases 7 through 9.
  - [ ] 4. The three explicitly dangerous actions, each computing and displaying a preview of the specific
     objects it will affect before any typed confirmation is accepted.
- [ ] **Done when:** I-UX-5 passes — for each of the three dangerous actions, the preview's counts and
  named objects are compared against the operation's actual effect on the same fixture and match
  exactly.

### Task 13.4 Jobs, schedules, and the scheduling engine

- **Build:** the scheduler that drives every recurring pass — cron parsing, jitter, due-job
  selection, backoff on consecutive failure, manual triggering — and the jobs/schedules page.
- **Where:** backend/crates/core (scheduler), backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Implement jitter and next-run-time computation feeding the due-jobs index, on top of the cron
     parsing already validated at save time in Phase 3.
  - [ ] 2. Implement durable run and event records for every job execution, live over SSE and queryable
     for the logs page.
  - [ ] 3. Implement backoff via a consecutive-failure counter, and startup marking of crash-residue
     running rows as cancelled.
  - [ ] 4. Build the jobs/schedules page: what runs, when — a concrete next-run time, not only the cron
     expression — what happened, manual triggering, and the running / overdue /
     repeatedly-failing-with-backoff / disabled states.
- [ ] **Done when:** every job's next run displays as a concrete time on the page, a manually triggered
  job produces a run record visible over SSE within the same page load, and a job crashed mid-run at
  startup shows as cancelled rather than perpetually running.

### Task 13.5 Logs

- **Build:** the structured logs page reading job-run events directly, filterable by run,
  definition, library, source, and level.
- **Where:** backend/crates/api, frontend/
- **Subtasks:**
  - [ ] 1. Implement filter parameters against the run-events index and each event's scope field.
  - [ ] 2. Confirm "everything that happened to this collection during last night's run" resolves as one
     filter combination, not a text search.
- [ ] **Done when:** the filter-combination query above returns the correct event set against a seeded
  fixture, sourced entirely from the durable event record rather than a log file.

### Task 13.6 Close remaining open questions this phase's pages depend on

- **Build:** resolve whatever the editor-disclosure and dashboard-content open questions (Q-002,
  Q-003) still owe the Design and Dashboard pages, now that both are being finalized, and confirm
  Q-013's board-layout answer needed nothing further once Phase 7 shipped.
- **Where:** frontend/
- **Subtasks:**
  - [ ] 1. Confirm Q-013 is fully closed by the Phase 7 board; extend it here only if a page in this
     phase surfaces placement content that still depends on it.
  - [ ] 2. Resolve Q-002 (editor disclosure) and Q-003 (dashboard content) against the Design and
     Dashboard pages as built.
- [ ] **Done when:** no open item in the plan's open-question register blocks a page shipped in this
  phase.

---

## Phase 14 — Release engineering

Size `M`.

**Exit:** no new invariants. The release lane (D-035) passes in full, on every supported platform.

### Task 14.1 The platform matrix and its images

- **Build:** images for every supported target (D-037) — `linux/amd64` Docker as primary,
  `linux/arm64` Docker supported, native binaries best-effort for `linux/amd64`, `linux/arm64`,
  `darwin/arm64`, and `windows/amd64`; `linux/armv7` explicitly unsupported and excluded from the
  build matrix.
- **Where:** backend/crates/afisharr, repository build and release tooling
- **Subtasks:**
  - [ ] 1. Build and publish `linux/amd64` and `linux/arm64` Docker images; treat `linux/amd64` as the
     tested target every resource budget in the PRD is measured against.
  - [ ] 2. Publish best-effort native binaries for `linux/amd64`, `linux/arm64`, `darwin/arm64`, and
     `windows/amd64`, without per-release manual testing gating the release.
  - [ ] 3. Exclude `linux/armv7` from the build matrix entirely, with the reason recorded: a 32-bit
     address space against a 1 GB memory ceiling and a 50 GB asset store.
  - [ ] 4. Bundle SQLite rather than linking the system copy, on every target, because the schema depends
     on features a system copy may predate.
  - [ ] 5. State the minimum supported Plex version per release and wire it into the release-lane contract
     test.
- [ ] **Done when:** the build matrix produces an image or binary for every supported target and none
  for `linux/armv7`, and the `linux/amd64` Docker image is the one the release lane's scale and
  precision budgets run against.

### Task 14.2 The release CI lane

- **Build:** the release lane from D-035 — merge and nightly lanes already exist by this phase;
  release adds the restore path and the Plex version matrix, gating a tagged release rather than
  every merge.
- **Where:** repository CI configuration
- **Subtasks:**
  - [ ] 1. Add the restore-path test (Phase 12) to the release lane specifically, since it needs a fully
     populated fixture and is too slow for merge or nightly.
  - [ ] 2. Add the Plex-version contract test across the stated minimum-and-current version matrix.
  - [ ] 3. Wire the release lane to run on tag, gating publication of the platform matrix's images and
     binaries.
- [ ] **Done when:** the release lane passes in full — merge and nightly invariants, the restore path,
  and the Plex version matrix — before any image or binary is published for a tagged release.

### Task 14.3 Licence compliance: dependency check, source link, attribution

- **Build:** machine-checked dependency licences, the runtime source-link obligation wired to the
  version stamp, and generated third-party attribution.
- **Where:** backend/crates/afisharr (version stamp, attribution generation), frontend/ (About panel, footer
  link), repository tooling (dependency-licence check)
- **Subtasks:**
  - [ ] 1. Wire a dependency-licence check with an explicit allow-list (MIT, Apache-2.0, BSD-2/3, ISC,
     Zlib, MPL-2.0, Unicode-3.0) into the pre-commit and CI gates, refusing everything else by
     default — explicitly including BSL, SSPL, Elastic, and CC-BY-NC.
  - [ ] 2. Wire the About panel's Source link, scaffolded earlier as part of the interface shell and
     Settings, to resolve from the running binary's version stamp rather than a branch, with the
     target field editable so a fork can retarget it; repeat the link in the footer.
  - [ ] 3. Generate the third-party licence file at build time from the dependency tree; ship it in the
     image and surface it from the About panel; separately credit the protocol reference this
     project builds against, retaining its notice.
- [ ] **Done when:** the dependency-licence check fails the build on an unlisted licence in a fixture
  dependency; the About panel's Source link, checked against two different version-stamped builds,
  resolves to two different targets; and the generated attribution file is present in the built
  image.

### Task 14.4 CONTRIBUTING.md, SECURITY.md, issue templates

- **Build:** the contribution and security-reporting documents the DCO and disclosure process
  require.
- **Where:** repository root
- **Subtasks:**
  - [ ] 1. Write `CONTRIBUTING.md` carrying the DCO 1.1 text verbatim, plus the contribution workflow.
  - [ ] 2. Write `SECURITY.md` with the vulnerability-reporting process.
  - [ ] 3. Add issue templates.
- [ ] **Done when:** `CONTRIBUTING.md` contains the DCO 1.1 text verbatim, and `SECURITY.md` and the
  issue templates exist at the paths the hosting platform expects.

### Task 14.5 Interface: the About panel's licence content

- **Build:** complete the About panel scaffolded in Phase 13's Settings with the licence name, the
  exact running version, and the source link for that version, together in one place.
- **Where:** frontend/
- **Subtasks:**
  - [ ] 1. Render the licence name (AGPL-3.0-or-later), the version stamp, and the source link together
     on the About panel.
  - [ ] 2. Confirm the panel and the footer link stay in sync when the source-link target is edited in
     Settings.
- [ ] **Done when:** the About panel and the footer both reflect an edited source-link target within the
  same session, with no mismatch between the two locations.
---

## Invariant coverage

Every invariant in *Invariants* in the PRD, assigned exactly once. This table is the completeness
check: 97 invariants, 97 assignments, no orphans and no duplicates. An invariant added to the PRD
without a row here goes silently unassigned, which looks exactly like being covered — so add the row
in the same change that adds the invariant.

**This table carries no checkbox, deliberately.** It tracks assignment — every invariant belongs to
exactly one phase — and not whether an invariant currently passes. Pass and fail are the build's
answer to a question this document cannot hold, and a table that reported both would be read as
reporting one.

| Phase | Invariants | Count |
| --- | --- | --- |
| 0 | `I-DATA-2`, `I-DATA-3`, `I-DATA-5`, `I-DATA-6`, `I-DATA-7`, `I-DATA-8`, `I-DATA-10`, `I-DATA-11` | 8 |
| 1 | `I-SEC-1`, `I-SEC-2`, `I-SEC-3`, `I-SEC-8`, `I-UX-1`, `I-UX-2`, `I-UX-3`, `I-UX-7`, `I-UX-9` | 9 |
| 2 | `I-ID-5` | 1 |
| 3 | `I-DEF-1`, `I-DEF-2`, `I-DEF-3`, `I-DEF-5`, `I-DEF-6`, `I-DEF-7` | 6 |
| 4 | `I-ID-1`, `I-ID-2`, `I-ID-3`, `I-ID-4`, `I-EVID-8`, `I-DATA-4`, `I-PERF-1` | 7 |
| 5 | `I-SRC-1` to `I-SRC-5`, `I-SRC-7`, `I-SRC-8`, `I-EVID-1`, `I-EVID-4`, `I-DATA-12`, `I-DATA-13`, `I-SEC-7`, `I-PERF-4` | 13 |
| 6 | `I-SRC-6`, `I-IDEM-1`, `I-REV-5`, `I-REV-6` | 4 |
| 7 | `I-CONV-1` to `I-CONV-8`, `I-IDEM-2`, `I-IDEM-3`, `I-UX-4`, `I-UX-6` | 12 |
| 8 | `I-RENDER-1` to `I-RENDER-7`, `I-REV-1`, `I-REV-2`, `I-PERF-2`, `I-DATA-9` | 11 |
| 9 | `I-LIFE-1` to `I-LIFE-4`, `I-EVID-2`, `I-EVID-3`, `I-EVID-5`, `I-EVID-6`, `I-EVID-7`, `I-DATA-1`, `I-SEC-4`, `I-PERF-3` | 12 |
| 10 | `I-ACQ-1` to `I-ACQ-5` | 5 |
| 11 | `I-REV-3`, `I-REV-4` | 2 |
| 12 | `I-SEC-5`, `I-SEC-6` | 2 |
| 13 | `I-UX-5`, `I-UX-8`, `I-UX-10`, `I-DEF-4`, `I-DEF-8` | 5 |
| 14 | — (the release lane in full) | 0 |
| | **Total** | **97** |

**A mechanism may be built in one phase and its invariant claimed in a later one.** The split is
deliberate and is preserved above. Sort-title capture is built in Phase 7, but `I-REV-1` is claimed
in Phase 8 and `I-REV-3` in Phase 11, because only a complete apply-then-reverse cycle can test them.
Acquisition-gate evaluation is scaffolded in Phase 9, but `I-ACQ-4` and `I-ACQ-5` are claimed in
Phase 10. Do not move an invariant to the phase that builds its mechanism.

---

## What the plan rebuild had to discharge

D-034 recorded why the previous plan was deleted. Each requirement, and where it landed.

| Requirement from D-034 | Discharged by |
| --- | --- |
| Tier 0 grew by roughly a factor of two | The plan is fifteen phases, four of them `XL`. It does not attempt the old shape |
| Placement had no phase of its own and is now the highest risk | Phase 7, blocked on a spike track that exists to de-risk it |
| Lifecycle needs persisted state, an append-only audit, and crash-safe intents | Phase 9, with the evidence group as its exit criteria |
| Teardown crosses four subsystems and needs its own fixtures | Phase 11, with fixtures built earlier and reused |
| Theme music, local assets, and i18n appeared in no phase | i18n in Phase 1; local assets in Phase 8; theme music and extras in Phase 13 |
| The invariants replaced the external references the old plan relied on | Every phase's exit criterion is invariants. All 97 assigned, *Invariant coverage* |

**Two constraints D-034 named explicitly.** The teardown fixtures are worth building before Phase 11
and are — they are needed from Phase 6 onward to test reversibility of anything. And the plan is not
compressed to fit the old twenty-week shape, because it carries no weeks at all.

---

## Where this plan is most likely to be wrong

Stated because a plan nobody doubts is a plan nobody corrects.

1. **Phase 5 and the adapter tail.** Twenty adapters is an estimate of repetition, and the repetition
   will not be uniform. The scraped and semi-documented sources will each surprise. This is the phase
   most likely to be larger than it looks, and the one where cutting is cheapest.

   It grew when CR-3 through CR-6 landed in it, taking it from eight exit invariants to thirteen.
   Four of the five additions are infrastructure rather than adapters, so the *tail* did not lengthen
   — the head did. That changes what is safe to cut. Adapters remain the cheap cut; the endpoint
   ladder, the parser-versioned cache, the dataset importer, and the parameter feed are not, because
   each one is far more expensive to retrofit than to build, and three of them are correctness rather
   than convenience.
2. **Phase 7 depends on answers nobody has yet.** Its size is `XL` on the assumption that the spikes
   return workable answers. If Q-015 returns "per-library sequences merged at render", Phase 7 grows
   and Phase 13's board page grows with it.
3. **Phase 8 and Phase 9 are both `XL` and both touch the user's library.** They are sequenced apart
   deliberately, and the temptation to interleave them — because lifecycle overlays need both —
   should be resisted until Phase 9's state machine passes its own tests.
4. **The interface is distributed across every phase and is therefore hardest to see.** If anything
   silently slips, it will be pages, and the symptom will be a backend that works and a product
   nobody can use.

---

## What is still open

| Item | Effect on this plan |
| --- | --- |
| The two spikes (Q-014, Q-015) | Block Phase 7. The spike track exists for them |
| The home-screen board (Q-013) | Blocked on Q-015. Lands in Phase 7 or Phase 13 depending on the answer |
| Editor disclosure and dashboard content (Q-002, Q-003) | Shape Phase 3's and Phase 13's pages. Neither blocks a phase |
| Retention windows (Q-005) | Promoted to "before Phase 9 ships" |
| A further prior-art pass (Q-012) | Recommended immediately before Phase 8, when the renderer is next |
| The two amendments the data model owes | Asset-store sizing before Phase 8; the retention cap before Phase 9 |

---
## Appendix A — Coding standards as build gates

This appendix is the build-facing form of the coding guidelines. The PRD states the same rules as
obligations the code must satisfy; here they are commands to run and boxes to tick.

**The boxes in this appendix are a template, not a ledger.** Every `- [ ]` below is worked through
fresh for each task, by each author and each reviewer, and the copy in this document stays unticked
permanently. The progress boxes are the ones in the phase bodies (*How to read this*), and those are
the only checkboxes in this plan that are ever marked in place.

**Reference convention for this appendix.** A reference of the form `§A.n` points at a subsection
below. Every other numbered reference — `§24.2.6`, `§24.4`, and so on — points at the PRD. The
guidelines live in the PRD's §24, and this appendix does not restate them; it says how to check them.

An implementer reading only this appendix knows what to run and what must pass. An implementer who
needs to know *why* a rule exists reads the PRD's §24.

**Neither this appendix nor §24 replaces the stack rule files.** `.augment/rules/backend-rust-dev-pro.md`
and `.augment/rules/frontend-dev-pro.md` are normative, and you read the one for your surface before
you write code for it (PRD §24.1, D-048). The commands below cannot substitute for that: every rule
in this appendix is one a script or a reviewer can catch, and the rule files exist because most of
what makes code wrong on either stack is neither.

### A.1 The gates every phase must pass

Every phase of work — a task, a PR, a milestone — passes only when all of the following commands succeed. This section is the single command list; §A.2–§A.5 explain how each command's configuration is derived and how to work through a task so the gates pass on the first CI run rather than the third.

**Rust gate (run from `backend/`, the workspace root — cargo and rustup discover
`.cargo/config.toml` and `rust-toolchain.toml` by walking up, never down):**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --all-features
cargo test --doc                # nextest doesn't run doctests
cargo deny check                # licenses + advisories + bans
cargo machete                   # unused dependencies
cargo semver-checks             # (for libs) catch accidental breaking changes
```

Any crate that touches `unsafe` additionally requires, at least before merge:

```bash
cargo +nightly miri test
```

**Frontend gate (run from `frontend/`, the package root, always with `bun`, never `npm`):**

```bash
bun install --frozen-lockfile   # fail if bun.lock would change
biome ci .                      # format + lint + assist, writes nothing
bun run check                   # svelte-kit sync && svelte-check --tsconfig ./tsconfig.json
bun test                        # unit tests — every *.test.ts except *.svelte.test.ts
bun run test:browser            # vitest run — *.svelte.test.ts in chromium
bun run build                   # vite build via adapter-static; must complete with no server-only code reachable
```

**Structure gate (run from the repository root — it spans both surfaces).** This is the
script-checkable half of PRD §24.6.
Each command must print nothing. A line of output names a file over its soft limit (§24.6.4), which
is either split in this change or justified in one sentence in the PR. A file over its hard limit is
split, or it carries a `// STRUCTURE:` header comment naming why the split is worse, agreed by a
reviewer who is not the author — two signatures, recorded in the file.

```bash
# Rust, non-test: soft 400, hard 700
find backend/crates -name '*.rs' -not -path '*/target/*' -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 400' | sort -rn

# Svelte components: soft 250, hard 400
find frontend/src -name '*.svelte' -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 250' | sort -rn

# TypeScript and rune modules: soft 300, hard 500
find frontend/src \( -name '*.ts' -o -name '*.svelte.ts' \) -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 300' | sort -rn
```

Generated code, the four registry constant tables (§13.2–§13.6 in the PRD), and SQL migrations are
exempt from the limits — exclude them from the paths above rather than raising a threshold to
accommodate them. The other four rules of §24.6 — division by domain, one file one thing, no god
files, narrow public surfaces — carry no command, because no linter for either surface can see
whether two responsibilities are related. They are enforced in §A.2, §A.3, and §A.4 instead.

**Local hooks:** the repository uses `prek` (a pre-commit-compatible hook runner) to run a fast subset of the above on every commit, and the full set on push/CI. `prek` reads the same `.pre-commit-config.yaml` shape as its predecessor tooling; do not write a second, competing hook mechanism (a custom `git commit` wrapper, a `husky`-style npm hook, etc.) alongside it.

```bash
prek install                    # install the git hook once, locally
prek run --all-files            # run every configured hook against the whole tree
prek run --hook-stage push      # run only the push-stage hooks (heavier: tests, deny, semver-checks)
```

Nothing merges with a red gate. A gate that is red because of a pre-existing, unrelated failure still blocks — fix it or explicitly scope it out with the reviewer before proceeding, never merge past it silently.

### A.2 Rust checklist per task

Work through this list for every Rust task before requesting review. Each line names the concrete thing to check and where the rule that backs it lives in the requirements document (§24.2, and §24.6 for the four structure lines that open the list).

The first line is the only one you tick *before* writing code rather than after.

- [ ] **Rules read**: you read `.augment/rules/backend-rust-dev-pro.md` before writing this task's code, not after. If the task touches a construct the file covers — async, error types, trait design, `serde`, SQLx, concurrency — you re-read that part of it (PRD §24.1, D-048).
- [ ] **Placement**: every new file sits in a subfolder named after a domain, not a layer. No file was added to a `utils`/`helpers`/`common`/`types`/`models` catch-all, and no such module was created (§24.6.1, §24.6.3).
- [ ] **Single purpose**: you can name each new or changed file's job in one sentence with no "and". Where the sentence needed an "and", the file was split at that seam (§24.6.2).
- [ ] **File size**: the structure gate in §A.1 prints nothing for this diff. A file over its soft limit is split, or the PR carries the one sentence saying why not. A file over its hard limit is split, or it carries a `// STRUCTURE:` comment naming the category — and you have asked a reviewer to sign it, not assumed they would (§24.6.4).
- [ ] **Module surface**: children are declared `mod x;` and the intended surface re-exported with `pub use`; `pub(crate)` used for anything shared only inside the crate; no new `pub` that exists so one caller could reach three levels in. `mod.rs` declares and re-exports, and holds no logic (§24.6.5, §24.6.3).
- [ ] **Ownership**: parameters accept the least-owned type that works (`&str`/`&[T]`/`impl AsRef<Path>`/`Cow`), not `&String`/`&Vec<T>`/forced-owned `String` (§24.2.1).
- [ ] **Shared state**: no new `Arc<Mutex<HashMap<…>>>` introduced for a read-mostly structure; `RwLock`/`dashmap` used instead where reads dominate (§24.2.1).
- [ ] **Typing**: any new domain concept that can be in an illegal state is a newtype, not a raw `u64`/`String`/`i32` passed around by convention (§24.2.2).
- [ ] **Error handling**: library/crate code returns typed `thiserror` errors; binary/application code uses `anyhow` with `.context()`. Zero `unwrap()`/`expect()` introduced in a non-test path, except a justified `expect("reason that proves it cannot fail")` (§24.2.3).
- [ ] **Async**: no blocking call (`std::fs`, `std::sync::Mutex::lock` held across `.await`) inside an `async fn` reachable from the Tokio runtime; blocking/CPU-bound work goes through `spawn_blocking` (§24.2.4).
- [ ] **`select!` branches**: every branch of a new `tokio::select!` is cancellation-safe, or the mutable state has been hoisted out of the branch (§24.2.4).
- [ ] **Concurrency**: new fork-join code over borrowed data uses scoped threads or `rayon`, not `Arc`-and-clone (§24.2.5).
- [ ] **Handlers**: new Axum handlers use the shared `AppError`/`IntoResponse` pattern for error responses, not ad hoc status-code tuples scattered per handler (§24.2.6).
- [ ] **OpenAPI**: every new/changed public handler and DTO carries the utoipa annotations needed to keep the generated schema — and the generated TypeScript client — correct. Regenerate the client and confirm it compiles against the frontend before requesting review (§24.2.6, §24.5).
- [ ] **Database**: every new query goes through `sqlx::query!`/`query_as!` against the SQLite pool (never a hand-built SQL string, never string-formatted user input into a query) (§24.2.7).
- [ ] **DTOs**: new config/DTO structs carry `#[serde(deny_unknown_fields)]` unless there's a specific reason to accept unknown fields (§24.2.8).
- [ ] **Unsafe**: `unsafe_code` remains `forbid`ed at the crate root unless this task is specifically introducing a justified `unsafe` block; if so, every block has a `// SAFETY:` comment and the crate has been run under `cargo +nightly miri test` (§24.2.9).
- [ ] **Performance**: collections with a known size are pre-sized (`with_capacity`); no unnecessary `Vec<T>` where `Box<[T]>`/`&[T]` would do (§24.2.10).
- [ ] **Logging**: new async entry points that matter operationally carry `#[instrument]`, and log at the right level (`info!` for normal operation, `warn!`/`error!` for anomalies) (§24.2.11).
- [ ] **Tests**: new logic has a unit test in-module; new public behavior crossing a module boundary has an integration test under `tests/`; new doc examples compile under `cargo test --doc`. `cargo nextest run --all-features` is green (§24.2.12).
- [ ] **Dependencies**: any new dependency is added via `cargo add` (not hand-edited into `Cargo.toml`), does not appear on the discontinued/superseded list (`async-std`, `lazy_static`, `once_cell`, `structopt`, `failure`, `error-chain`, `#[bench]`), and `cargo machete`/`cargo deny check` stay green after the change (§24.2.13).
- [ ] **Lints**: the crate's `[lints] workspace = true` is intact (no per-crate override added to the same table); any necessary exception is a crate-level `#![allow(...)]` attribute with the reason stated (§24.2.13, §24.2.14).
- [ ] **Docs**: every new public item has a doc comment; `cargo doc --no-deps` builds clean with no `missing_docs` warnings (§24.2.15).
- [ ] **Formatting/lints ran locally, not just imagined**: `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` both pass before the PR is opened.

**Exact configuration these checks are measured against** (copy-ready; keep these files in sync with the workspace root):

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy", "rust-src"]
profile = "default"
```

`rustfmt.toml`:

```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Default"
imports_granularity = "Crate"     # nightly-gated formatting options
group_imports = "StdExternalCrate"
```

`clippy.toml`:

```toml
msrv = "1.97"
avoid-breaking-exported-api = false
```

Workspace `Cargo.toml` lint table (every member crate opts in via `[lints] workspace = true`):

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

`deny.toml`:

```toml
[advisories]
yanked = "deny"
[bans]
multiple-versions = "warn"
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-3.0"]
```

`Cargo.lock` is committed (this is a binary/application, not a library).

### A.3 Frontend checklist per task

Work through this list for every frontend task before requesting review. Each line names the concrete thing to check and where the rule that backs it lives in the requirements document (§24.3, §24.4, and §24.6 for the four structure lines that open the list).

The first line is the only one you tick *before* writing code rather than after.

- [ ] **Rules read**: you read `.augment/rules/frontend-dev-pro.md` before writing this task's code, not after. If the task touches a construct the file covers — runes, props, effects, the SvelteKit file conventions, UnoCSS, shadcn-svelte, Bun tooling — you re-read that part of it. Read §24.4 alongside it: the rule file describes the full SvelteKit stack, and this project has no JavaScript server runtime (PRD §24.1, D-048).
- [ ] **Placement**: new domain code sits in `src/lib/features/<domain>/` beside that domain's state and API calls; nothing domain-specific was added to `src/lib/components/ui/`; no `utils.ts`/`helpers.ts`/`types.ts` catch-all was created or grown (§24.6.1, §24.6.3).
- [ ] **Single purpose**: each new component renders one thing, and each new `.svelte.ts` module owns one piece of state. A component that both fetches a list and edits a row is two components (§24.6.2).
- [ ] **File size**: the structure gate in §A.1 prints nothing for this diff — 250 lines soft and 400 hard for `.svelte`, 300 soft and 500 hard for `.ts`/`.svelte.ts`. Past the hard limit, either split the component or carry a `// STRUCTURE:` comment a reviewer signs; "the sub-parts would share a dozen `$bindable` values" is a real reason, and it is written down (§24.6.4).
- [ ] **Feature surface**: the feature's `index.ts` names its exports, and every cross-feature import goes through that barrel rather than a deep path. Components that serve one parent are not exported from it (§24.6.5).
- [ ] **Runes only**: no `export let`, `$:`, `on:click`, `<slot />`, `createEventDispatcher`, or `new Component({ target })` introduced anywhere in the diff — grep the diff for these before opening the PR (§24.3.2, §24.3.13).
- [ ] **Derived vs effect**: any new value computed from other state is `$derived`/`$derived.by`, not an `$effect` that writes to another `$state` (§24.3.2).
- [ ] **Reactive collections**: any new `Map`/`Set`/`Date` that needs to be reactive is `SvelteMap`/`SvelteSet`/`SvelteDate` from `svelte/reactivity`, not the plain built-in (§24.3.3).
- [ ] **Page/nav state**: `$app/state`, never `$app/stores` (`$page`) (§24.3.3, §24.3.13).
- [ ] **Static-SPA compliance**: no new `+page.server.ts`/`+layout.server.ts` load depending on per-request data, no new form `actions`, no new code in `src/hooks.server.ts`, no new `bun:sqlite`/`Bun.sql`/`$lib/server/db` access, no new `$env/dynamic/*` or `$env/static/private` read, no new `.remote.ts` file. Every new data read/write goes through the generated OpenAPI TypeScript client called client-side (§24.4).
- [ ] **Component authoring**: new `$lib/components/ui/*` files follow the house idiom — `ref = $bindable(null)`, renamed `class`, spread `...restProps`, `bind:ref`/`bind:this`, `data-slot` attribute, variants via `tailwind-variants` (`tv`), not `class-variance-authority` (§24.3.1).
- [ ] **Bits UI wrapping**: new interactive widgets wrap a Bits UI primitive rather than hand-rolling keyboard/focus/ARIA behavior on plain elements; trigger customization uses the `child` snippet, not an `asChild` prop (§24.3.1, §24.3.4).
- [ ] **Accessibility**: every new label/input pair uses `$props.id()` for the `for`/`id` association, not a hand-written string id (§24.3.4).
- [ ] **Keyed each**: every new `{#each}` over data that can reorder or be removed is keyed with `(item.id)` (§24.3.1, §24.3.9).
- [ ] **Styling**: only `presetWind4` UnoCSS classes/shortcuts used; no `presetUno`/`presetWind3`, no `tailwind.config.js`, no `@tailwind` directive, no `@apply`/`@screen` (prefer `shortcuts`) (§24.3.5, §24.3.13).
- [ ] **Icons**: `@lucide/svelte` only; confirm no `lucide-svelte` (unscoped) import slipped in (§24.3.1, §24.3.13).
- [ ] **Typing**: every new/changed component has an explicit `Props` type (or inline prop type); class props typed `ClassValue`; snippet props typed `Snippet`/`Snippet<[T]>` (§24.3.11).
- [ ] **Bun-native**: no `dotenv`, `ts-node`/`tsx`, `jest`, `bcrypt`, or `nodemon` introduced — use `Bun.env`/`$env`, native `.ts` execution, `bun:test`, `Bun.password`, `bun --hot` respectively (§24.3.12, §24.3.13).
- [ ] **Tests placed correctly**: new pure logic / `.svelte.ts` state has a `bun:test` test; new `.svelte` component behavior has a Vitest browser-mode test — never the other tool for the other target (§24.3.10).
- [ ] **Format/lint/typecheck ran locally, not just imagined**: `biome ci .` and `bun run check` both pass before the PR is opened.

**Exact configuration these checks are measured against** (copy-ready; keep these files in sync with the frontend package root):

`biome.json`:

```jsonc
{
  "$schema": "https://biomejs.dev/schemas/2.5.7/schema.json",
  "vcs": { "enabled": true, "clientKind": "git", "useIgnoreFile": true },
  "files": { "includes": ["**", "!build", "!.svelte-kit", "!dist"] },
  "formatter": { "enabled": true, "indentStyle": "tab" },
  "linter": { "enabled": true, "rules": { "preset": "recommended" } },
  "assist": { "actions": { "source": { "organizeImports": "on" } } },
  "javascript": {
    "formatter": { "quoteStyle": "single", "trailingCommas": "all" }
  },
  // Formats the Svelte template markup, not just <script> and <style>.
  // Experimental — drop this line if the markup output surprises you.
  "html": { "experimentalFullSupportEnabled": true }
}
```

`tsconfig.json`:

```jsonc
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

`package.json` scripts (the canonical entry points every gate command below maps to):

```jsonc
{
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "test:unit": "bun test",
    "test:browser": "vitest run",
    "lint": "biome ci .",
    "format": "biome format --write ."
  }
}
```

`bunfig.toml` (test config the coverage gate is measured against):

```toml
[install]
registry = "https://registry.npmjs.org/"
exact = true

[test]
preload = ["./test-setup.ts"]
coverage = true
coverageReporter = ["text", "lcov"]
coverageThreshold = { lines = 0.85, functions = 0.90, statements = 0.80 }
```

Biome's version is pinned exactly in `package.json` (`bun add -D --exact @biomejs/biome`) — never a floating range.

### A.4 Review checklist

A reviewer works through this list against the diff, independent of what the author already checked. Anything found here is a request-changes, not a nit.

**Grep the diff for prohibited patterns before reading a single line of logic:**

Rust:
```bash
git diff --unified=0 -- '*.rs' | grep -nE '\.unwrap\(\)|\.expect\('   # outside #[cfg(test)]?
git diff --unified=0 -- '*.rs' | grep -nE 'Arc<Mutex<.*HashMap'
git diff --unified=0 -- '*.rs' | grep -nE '&Vec<|&String\b'
git diff --unified=0 -- '*.rs' | grep -nE 'async-std|lazy_static|once_cell::|structopt|error-chain'
git diff --unified=0 -- '*.rs' | grep -nE '\bunsafe\b' # every hit needs an adjacent SAFETY: comment
```

Frontend:
```bash
git diff --unified=0 -- '*.svelte' '*.ts' | grep -nE 'export let |on:click|<slot|createEventDispatcher|new [A-Z][A-Za-z]*\('
git diff --unified=0 -- '*.svelte' '*.ts' | grep -nE '\$app/stores'
git diff --unified=0 -- '*' | grep -nE 'presetUno|presetWind3|tailwind\.config|@tailwind|class-variance-authority|lucide-svelte'
git diff --unified=0 -- '*' | grep -nE '\+page\.server\.ts|\+layout\.server\.ts|hooks\.server\.ts|\.remote\.ts|bun:sqlite|env/dynamic'
git diff --unified=0 -- '*' | grep -nE '\bdotenv\b|\bts-node\b|\bbcrypt\b|\bjest\b'
```

Structure, both surfaces (§24.6):
```bash
# Catch-all module names, created or grown
git diff --name-only | grep -nE '(^|/)(utils|helpers|common|misc|shared|types|models)\.(rs|ts)$|(^|/)(utils|helpers|common|misc|shared)/'

# Size of every file the diff touches, against the limits in §A.1
git diff --name-only --diff-filter=d | xargs wc -l | sort -rn | head -20
```

Any hit in the Rust `unsafe`/db/db-string block or the frontend static-SPA block is not automatically wrong, but it must be explained in the PR description with the §24.4/§24.2.9 rationale for why the exception applies — an unexplained hit is a request-changes.

**Beyond grep, the reviewer confirms:**

- [ ] **Structure, before anything else.** Every new file is in a domain folder, states one thing, and does not add a responsibility to a module that already had a different one. A file that grew past its soft limit was split, or the PR says in one sentence why not (§24.6.1–§24.6.4). This is a request-changes on its own — a correct change in the wrong file is still the wrong file, and it is cheaper to move now than after the next phase builds on it.
- [ ] **The hard-limit exception, if this PR takes one.** You are the second signature (§24.6.4), so decide rather than wave it through. The `// STRUCTURE:` comment names a category — one state machine, one exhaustive `match`, one editor whose parts would share a dozen `$bindable` values — and not a schedule. Confirm the file is one thing under §24.6.2 first: the exception exists for a file that is big, never for a file that is two files.
- [ ] **Public surface.** Every new `pub`/`pub use`/barrel export is there because a caller outside the boundary needs it, not because a symbol was needed somewhere and `pub` was the shortest fix. Ask whether the caller belongs inside the boundary instead (§24.6.5).
- [ ] **Stack idiom, checked against the rule file rather than from memory.** Where the diff uses a construct the surface's rule file covers, open that part of `.augment/rules/backend-rust-dev-pro.md` or `.augment/rules/frontend-dev-pro.md` and compare. Each file pairs the current idiom with the wrong-but-plausible alternative; the grep block above catches only the alternatives common enough to write as a regex, and the rest are found by reading. A diff that is green on every command and contradicts its rule file is a request-changes (PRD §24.1, D-048).
- [ ] The OpenAPI schema/generated client were regenerated in the same PR as any handler/DTO change, and the frontend diff, if any, uses the regenerated client rather than a hand-typed shape (§24.2.6, §24.5).
- [ ] Error responses use the shared `AppError` pattern; no new bespoke error shape was introduced for one endpoint (§24.2.6).
- [ ] New public Rust items have doc comments; `cargo doc --no-deps` was actually run, not assumed clean (§24.2.15).
- [ ] New components have explicit prop types; nothing relies on inferred `any` without a `biome-ignore` and a reason (§24.3.11).
- [ ] Test placement matches the target: unit logic in `bun:test`/`#[cfg(test)]`, component behavior in Vitest browser mode, cross-module Rust behavior in `tests/` (§24.2.12, §24.3.10).
- [ ] No dependency was added that appears on either discontinued/superseded list (§24.2.13) or duplicates functionality Bun/Biome already provide (§24.3.12).
- [ ] `cargo deny check`, `cargo machete`, `bun install --frozen-lockfile`, and `biome ci .` output was actually pasted or linked in the PR, not just claimed.
- [ ] Every SAFETY comment on a new `unsafe` block actually states an invariant, not a restatement of what the code does.
- [ ] The reviewer is not the author of the change under review (§A.1's "nothing merges with a red gate" is enforced by CI; this line is enforced by process — no self-approval).

### A.5 CI lane configuration

CI runs as independent lanes so a failure in one stack does not block investigating the other, and so the fast lanes (format/lint) fail loudly before the slow lanes (tests, supply-chain, browser) even start.

**Lane order and exact commands:**

1. **`rust-fmt`** — `cargo fmt --check`. Fails fast; no compilation needed.
2. **`rust-lint`** — `cargo clippy --all-targets --all-features -- -D warnings`. Depends on nothing but the toolchain from `rust-toolchain.toml`.
3. **`rust-test`** — `cargo nextest run --all-features` followed by `cargo test --doc`.
4. **`rust-supply-chain`** — `cargo deny check` and `cargo machete`, run in parallel with each other, after `rust-lint` passes (both need a resolved dependency graph, neither needs `rust-test` to have finished).
5. **`rust-miri`** — `cargo +nightly miri test`, scoped to crates that contain `unsafe` (skip entirely for crates with `unsafe_code = "forbid"` and no exceptions). Runs in parallel with `rust-test`.
6. **`frontend-install`** — `bun install --frozen-lockfile`. Every later frontend lane depends on this one; it is the single point where a lockfile drift is caught.
7. **`frontend-format-lint`** — `biome ci .`. Fails fast; no build needed.
8. **`frontend-typecheck`** — `bun run check` (`svelte-kit sync && svelte-check --tsconfig ./tsconfig.json`).
9. **`frontend-test-unit`** — `bun test`.
10. **`frontend-test-browser`** — `bun run test:browser` (`vitest run`, Playwright-backed browser mode). Slowest frontend lane; runs after `frontend-typecheck` and `frontend-test-unit` pass, not before, since a typecheck or unit failure is cheaper to report first.
11. **`frontend-build`** — `bun run build`. Exercises `adapter-static`; a page or component that reaches for a server-only primitive covered by §24.4 fails the build here, which is the backstop for the review-time grep in §A.4.
12. **`contract-check`** — regenerate the OpenAPI client from the current `utoipa` schema and diff it against the committed generated client; a non-empty diff fails the lane. This is what makes "the client was regenerated in this PR" (§A.4) a machine-checked fact instead of a reviewer's trust.

**Merge gate:** every lane above must be green. `rust-miri` and `frontend-test-browser` may be configured as required-but-parallel rather than sequential dependencies of everything else, to keep total wall-clock time down, but "parallel" never means "optional" — the merge gate still waits on their result.

**Local parity with CI:** `prek run --hook-stage push` runs the same fast lanes (`rust-fmt`, `rust-lint`, `frontend-format-lint`, `frontend-typecheck`) locally before a push, so lane failures are caught before CI minutes are spent on them. The heavier lanes (`rust-test`, `frontend-test-browser`, `rust-miri`, `contract-check`) are not run on every commit locally — they run in CI and, optionally, via `prek run --all-files` when explicitly requested.

```yaml
# .pre-commit-config.yaml (read by prek)
repos:
  - repo: local
    hooks:
      # prek runs every hook from the repository root, so the cargo entries cd
      # into backend/ themselves. A `files` filter alone would leave them
      # running against the default toolchain with no workspace in sight.
      - id: rust-fmt
        name: cargo fmt --check
        entry: sh -c 'cd backend && cargo fmt --check'
        language: system
        pass_filenames: false
      - id: rust-clippy
        name: cargo clippy
        entry: sh -c 'cd backend && cargo clippy --all-targets --all-features -- -D warnings'
        language: system
        pass_filenames: false
      - id: biome-ci
        name: biome ci
        entry: biome ci .
        language: system
        pass_filenames: false
      - id: svelte-check
        name: svelte-check
        entry: bun run check
        language: system
        pass_filenames: false
        stages: [push]
      - id: file-size
        name: file size limits (PRD 24.6.4)
        entry: scripts/check-file-size.sh
        language: system
        pass_filenames: false
```

`scripts/check-file-size.sh` holds the three commands from §A.1, plus the exempt-path exclusions, and exits non-zero on any output. It also skips any file whose first ten lines carry a `// STRUCTURE:` comment, so a signed hard-limit exception (§24.6.4) does not block every later commit that touches the file. That is the whole enforcement of the exception in the script — a comment is cheap to write, and the control that makes it expensive is the reviewer who has to sign it (§A.4), not the grep. It is a commit-stage hook rather than a push-stage one on purpose: the point of the limit is to stop a file crossing it, and a check that runs after five commits reports a split that is now five commits deep.

Never use `npm`/`npx` in any lane or hook — every frontend command in this document is a `bun`/`bunx` invocation, and a stray `npm install` in CI config is itself a gate failure to catch in review (§A.4).
