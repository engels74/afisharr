---
type: "agent_requested"
description: "Rust coding guidelines"
---
# Rust 1.97 / Edition 2024 — Production Coding Reference

Rust's current stable posture (1.97.1, edition 2024) is a language that has finished absorbing most of the "hard" ergonomics work: `async fn` works in traits, `let` chains and async closures are stable, RPIT captures lifetimes the way you expect, and `std` now ships `LazyLock`, scoped threads, and `OnceLock` so you rarely reach for a crate to get a lazy global or structured parallelism. Optimize for **ownership-first design** (borrow, don't clone; move, don't `Arc`), **errors as values** (typed errors in libraries, `?` everywhere, `unwrap` never in library code), and **zero-cost abstraction** (iterators and generics that monomorphize, not `dyn` and boxing by reflex). This stack is exceptional at fearless concurrency, predictable performance without a GC, and compile-time-enforced correctness — lean on all three.

The biggest way an agent writes wrong-but-plausible Rust is by importing habits from an adjacent ecosystem. From Go/Java: wrapping everything in `Arc<Mutex<…>>` and cloning to dodge the borrow checker. From Python/TypeScript: `unwrap()`/`expect()` as normal control flow, stringly-typed errors, and deep `if let` nesting. From C++: inheritance-style trait-object hierarchies and reaching for `unsafe` to "go fast." From dynamic languages: `.clone()` sprinkled to silence lifetime errors. Idiomatic Rust is almost always the opposite: take `&str`/`&[T]`/`impl AsRef<Path>`, return owned data only when you must, model errors with enums, prefer generics + `impl Trait`, and treat `unsafe` as a last resort with a written safety proof.

- **Research date:** August 5, 2026
- **Research basis:** current official docs, release notes, specifications, changelogs, and primary repositories.
## Stack snapshot: current versions and status

| Component | Current stable | Notes |
|---|---|---|
| rustc / cargo | **1.97.1** (1.97.0 released July 9, 2026) | Six-week cadence; only latest stable is patched. v0 symbol mangling is now default. |
| Edition | **2024** (stabilized in 1.85.0, February 20, 2025) | Use `edition = "2024"` for all new crates. |
| tokio | **1.52.x** | Default async runtime. Ignore blog posts claiming a "Tokio 2.0"; tokio is still 1.x. |
| axum | **0.8.9** | Path params changed to `/{id}` syntax. `axum 0.9` in development. |
| serde / serde_json | **1.0.229 / 1.0.151** | `serde_core` split exists for faster parallel builds. |
| thiserror / anyhow | **2.0.x / 1.0.x** | thiserror 2 for libs, anyhow for bins. |
| clap | **4.6.x** (derive) | `structopt` is obsolete — clap derive replaced it. |
| sqlx | **0.9.0** (2026-05-21) | Compile-time-checked queries. |
| tracing / tracing-subscriber | **0.1.x / 0.3.23** | Structured logging default. |
| reqwest | **0.13** | Defaults to rustls. |
| rayon | **1.12.0** | Data parallelism. |
| criterion / divan | **0.8.2 / current** | criterion for statistical rigor; divan for ergonomics + CI. |
| rand | **0.9.x / 0.10.x** | New API: `rand::rng()`, `random_range()`, `rand::distr`. |
| jiff / chrono / time | **jiff current** | Prefer `jiff` for new datetime code. |
| cargo-nextest | **0.9.140** | Faster test runner. |
| cargo-deny / machete / udeps / semver-checks | **0.20.2 / 0.9.2 / 0.1.61 / 0.47.0** | Supply-chain + hygiene. |

**Discontinued / superseded — do not reach for these:**
- **`async-std`** — officially discontinued (v1.13.1, March 15, 2025; advisory **RUSTSEC-2025-0052** published August 24, 2025). Use `tokio` (default) or `smol` (lightweight, same author lineage). Never start a new project on it.
- **`lazy_static` / `once_cell`** — replaced by `std::sync::LazyLock` / `LazyCell` (stable 1.80, July 25, 2024, which "completes the stabilization of functionality adopted into the standard library from the popular lazy_static and once_cell crates") and `OnceLock` (stable 1.70). No crate needed.
- **`structopt`** — folded into `clap` v4 derive. Use `#[derive(Parser)]`.
- **`failure` / `error-chain`** — dead. Use `thiserror` + `anyhow`.
- **`#[bench]` / `test::Bencher`** — fully de-stabilized (a hard error since 1.88 without nightly `custom_test_frameworks`; it had been a deny-by-default future-incompatibility lint since 1.77). Use `criterion` or `divan` on stable; use `std::hint::black_box` (stable 1.66).
## Project and crate layout

Prefer a **workspace** from day one, even for a single binary — it makes adding crates, sharing dependency versions, and centralizing lints trivial.

```
myapp/
├── Cargo.toml                 # workspace root
├── Cargo.lock                 # COMMIT for binaries; for libs, do not commit
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── crates/
│   ├── app/                   # binary crate
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── core/                  # library crate (domain logic)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── api/                   # library crate (axum handlers)
│       ├── Cargo.toml
│       └── src/lib.rs
```

Workspace root `Cargo.toml` using **workspace inheritance** (dependency + lint versions defined once):

```toml
[workspace]
members = ["crates/*"]
resolver = "3"                 # edition 2024 default resolver

[workspace.package]
edition = "2024"
rust-version = "1.97"          # MSRV, checked by the resolver
license = "MIT OR Apache-2.0"

[workspace.dependencies]
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0"
anyhow = "1.0"
axum = "0.8.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

Member crate `Cargo.toml` — inherits everything:

```toml
[package]
name = "core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true               # opt into all workspace lints
```

Critical insight: a crate that sets `[lints] workspace = true` **cannot also override lints in the same `[lints]` table** — that is a hard error. Do per-crate exceptions with crate-level attributes like `#![allow(clippy::missing_errors_doc)]` in `lib.rs` instead.

### Feature flags: additive only

Features must be **additive** — enabling one must never remove APIs or break another consumer. Never model mutually-exclusive modes as features (that breaks under Cargo's feature unification when two dependents pick different modes).

```toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]          # `dep:` hides the implicit feature
postgres = ["dep:sqlx", "sqlx/postgres"]

[dependencies]
serde = { version = "1.0.229", optional = true }
sqlx = { version = "0.9", optional = true }
```

### Profile tuning

```toml
[profile.release]
opt-level = 3
lto = "thin"                   # "fat" for max, slower link; "thin" is the sweet spot
codegen-units = 1              # better optimization; slower compile
panic = "abort"                # smaller binaries, no unwinding — only if you never catch_unwind
strip = "symbols"

[profile.dev]
opt-level = 0

# Optimize heavy dependencies even in dev builds (e.g. crypto, parsing):
[profile.dev.package."*"]
opt-level = 2
```

## Edition 2024: what changes in daily code

Edition 2024 is stabilized (with Rust 1.85.0 on February 20, 2025) and is the correct floor. The changes that actually bite:

**RPIT lifetime capture** — `impl Trait` in return position now captures *all* in-scope generic and lifetime parameters automatically, matching `async fn`. The old "Captures trick" and `+ '_` workaround are gone. To *restrict* capture, use `use<..>`:

```rust
// Rust 2024: 'a is captured automatically — this now compiles and is correct
fn first_word<'a>(s: &'a str) -> impl Iterator<Item = &'a str> {
    s.split_whitespace()
}

// Restrict capture explicitly (stable since 1.82) when you must NOT borrow:
fn make_counter<T>(_seed: T) -> impl Iterator<Item = u32> + use<> {
    0..10   // captures nothing; independent of T's lifetime
}
```

**`if let` temporary scope** — temporaries in the `if let $pat = $expr` scrutinee now drop *before* the `else` branch, not at the end of the whole statement. This fixes a classic deadlock where a `MutexGuard` in the condition stayed locked in the `else`:

```rust
// 2024: the lock temporary is dropped before entering `else` — no deadlock
if let Some(v) = shared.lock().unwrap().get(&key).copied() {
    use_value(v);
} else {
    // lock is already released here
    shared.lock().unwrap().insert(key, default());
}
```

**`unsafe_op_in_unsafe_fn`** — inside an `unsafe fn`, unsafe operations now require their own `unsafe {}` block (warn-by-default). **Unsafe attributes** must be wrapped: `#[unsafe(no_mangle)]`, `#[unsafe(export_name = "…")]`. **`extern` blocks** must be written `unsafe extern "C" { … }`. **`static mut` references** are hard-denied — use `&raw const`/`&raw mut` or an atomic/`OnceLock`.

**`gen` is a reserved keyword** — any identifier named `gen` must be written `r#gen`. (This is why `rand` renamed `gen_range` → `random_range`.)

## Ownership, borrowing, and interior mutability

The single highest-value skill: **accept the least-owned type that works, return the most-owned type you must.**

| Want to accept… | Use | Not |
|---|---|---|
| Read-only text | `&str` | `&String`, `String` |
| Read-only slice | `&[T]` | `&Vec<T>` |
| A path | `impl AsRef<Path>` | `&str`, `PathBuf` |
| Maybe-owned text | `Cow<'_, str>` | always-clone `String` |
| Generic bytes | `impl AsRef<[u8]>` | `Vec<u8>` |

```rust
use std::borrow::Cow;
use std::path::Path;

// Accepts &str, String, Box<str>, ... with zero forced allocation.
fn shout(s: &str) -> String {
    s.to_uppercase()
}

// Cow: only allocate if we actually change something.
fn normalize(input: &str) -> Cow<'_, str> {
    if input.contains(' ') {
        Cow::Owned(input.replace(' ', "_"))
    } else {
        Cow::Borrowed(input)   // no allocation on the common path
    }
}

// impl AsRef<Path>: caller passes "foo.txt", String, PathBuf, Path — all work.
fn read_config(path: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(path.as_ref())
}
```

### When you actually need shared ownership

Reach for `Rc`/`Arc` only when ownership is genuinely shared and lifetimes cannot express it (graphs, shared caches, spawned tasks). Reach for interior mutability only when you need to mutate through a shared reference.

| Need | Single-thread | Multi-thread |
|---|---|---|
| Shared ownership | `Rc<T>` | `Arc<T>` |
| Mutate one value | `Cell<T>` (Copy) / `RefCell<T>` | `Mutex<T>` |
| Many readers, rare writes | `RefCell<T>` | `RwLock<T>` |
| Init once, read forever | `OnceCell` | `OnceLock<T>` (1.70) |
| Lazy global | `LazyCell` (1.80) | `LazyLock<T>` (1.80) |

```rust
use std::sync::LazyLock;
use std::collections::HashMap;

// Lazy global config — no lazy_static!, no once_cell. (stable since 1.80)
static SETTINGS: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    HashMap::from([("retries", 3), ("timeout_ms", 500)])
});

fn retries() -> i32 { SETTINGS["retries"] }
```

Anti-reflex: `Arc<Mutex<HashMap<K, V>>>` is a code smell if the map is read-mostly (use `RwLock`, or a sharded/concurrent map like `dashmap`), or if the lock is only held to hand data to a task (pass an owned clone or a channel instead).

## Error handling

**Libraries define typed errors with `thiserror` (2.0). Binaries use `anyhow` (1.0)** for a boxed, context-rich error. Never `unwrap()`/`expect()` in library code paths; `expect("reason")` is acceptable only for genuine invariants that cannot fail (e.g. a regex literal that is known-valid), and the message should state *why* it can't fail.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]                       // allow adding variants without a breaking change
pub enum StoreError {
    #[error("key not found: {0}")]
    NotFound(String),

    #[error("connection failed")]
    Connect(#[from] std::io::Error),    // auto From<io::Error>, keeps source chain

    #[error("invalid record for {key}: {reason}")]
    Invalid { key: String, reason: String },
}

pub fn load(key: &str) -> Result<Vec<u8>, StoreError> {
    let bytes = std::fs::read(key)?;    // io::Error auto-converts via #[from]
    if bytes.is_empty() {
        return Err(StoreError::Invalid { key: key.into(), reason: "empty".into() });
    }
    Ok(bytes)
}
```

Application/binary code — `anyhow` with `.context()`:

```rust
use anyhow::{Context, Result, bail};

fn run(path: &str) -> Result<()> {
    let cfg = std::fs::read_to_string(path)
        .with_context(|| format!("reading config at {path}"))?;
    if cfg.is_empty() {
        bail!("config {path} is empty");
    }
    Ok(())
}

fn main() -> Result<()> {
    run("app.toml")   // returning Result from main prints the full source chain
}
```

Guidance: use `Box<dyn std::error::Error + Send + Sync>` as a return type only for the simplest binaries or trait objects where you don't want the `anyhow` dependency; `anyhow::Error` is strictly better ergonomically (backtraces, context, downcasting). The `Error` trait now lives in `core` (since 1.81), so `no_std` libraries can implement it too.

## API design and traits

**Newtypes** for domain invariants; **builders** for many-optional-field construction; **`From`/`TryFrom`** for conversions; generics + `impl Trait` by default, `dyn Trait` only when you need heterogeneous collections or want to cut monomorphization/compile-time.

```rust
// Newtype: makes illegal states unrepresentable and adds no runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl TryFrom<i64> for UserId {
    type Error = &'static str;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        u64::try_from(v).map(UserId).map_err(|_| "user id must be non-negative")
    }
}
```

### async fn in traits (stable 1.75) vs `async-trait`

Native `async fn` in traits is stable (return-position `impl Trait` in traits, RPITIT, landed in Rust 1.75). Use it directly for application traits. You still need the `async-trait` crate only when the trait must be **`dyn`-compatible** (object-safe as a trait object), because native async trait methods return an anonymous `impl Future` that isn't object-safe.

```rust
// Static dispatch: native async fn in trait — no crate needed (stable 1.75)
trait Fetcher {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, std::io::Error>;
}

struct Http;
impl Fetcher for Http {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, std::io::Error> {
        Ok(b"data".to_vec())
    }
}

// Need `Box<dyn Fetcher>`? Then use async_trait, OR return a boxed future:
trait DynFetcher {
    fn fetch<'a>(&'a self, url: &'a str)
        -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<u8>> + Send + 'a>>;
}
```

### Sealed traits (prevent downstream impls)

```rust
mod sealed { pub trait Sealed {} }

pub trait Command: sealed::Sealed {
    fn run(&self);
}

pub struct Ping;
impl sealed::Sealed for Ping {}
impl Command for Ping { fn run(&self) { println!("pong"); } }
// Downstream crates can call Command but cannot implement it.
```

### `let` chains (stable 1.88, edition 2024) and async closures (stable 1.85)

```rust
// let chains: flatten nested if-let + boolean conditions with &&
fn describe(v: &Option<Result<i32, String>>) -> &'static str {
    if let Some(Ok(n)) = v && *n > 0 && *n < 100 {
        "small positive"
    } else {
        "other"
    }
}

// async closures: capture environment and return a future (AsyncFn traits)
async fn retry<F>(mut op: F) -> Result<(), String>
where
    F: AsyncFnMut() -> Result<(), String>,
{
    for _ in 0..3 {
        if op().await.is_ok() { return Ok(()); }
    }
    Err("exhausted retries".into())
}
```

## Iterators, pattern matching, and zero-cost idioms

Prefer iterator chains and combinators; they compile to the same code as hand-written loops. Use early-return `?` for fallible flows and combinators for transformations.

```rust
use std::collections::HashMap;

// Idiomatic: build a frequency map in one pass, no manual indexing.
fn word_counts(text: &str) -> HashMap<&str, usize> {
    text.split_whitespace().fold(HashMap::new(), |mut acc, w| {
        *acc.entry(w).or_insert(0) += 1;
        acc
    })
}

// let-else (stable 1.65): bind-or-diverge, keeps the happy path unindented.
fn parse_port(s: &str) -> Option<u16> {
    let Ok(p) = s.parse::<u16>() else { return None };
    (p >= 1024).then_some(p)
}

// matches! and slice patterns
fn classify(xs: &[i32]) -> &'static str {
    match xs {
        [] => "empty",
        [_] => "one",
        [first, .., last] if first == last => "palindromic ends",
        _ => "many",
    }
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}
```

Critical insight: prefer combinator style (`.map().filter().collect()`) when it reads cleanly, but switch to a `for` loop with `?` the moment you have fallible steps or side effects — forcing `Result` through `collect::<Result<Vec<_>, _>>()` is fine, but nested combinators with early exit become unreadable.

## Concurrency (non-async)

**Scoped threads (stable 1.63)** let you borrow local data across threads without `Arc` — use them for fork-join over borrowed slices. **`rayon` (1.12)** for data parallelism. Reach for channels (`std::sync::mpsc`, or `crossbeam` for MPMC/select) to pass ownership between threads instead of sharing mutable state.

```rust
use std::thread;

// Scoped threads: borrow `data` directly, no Arc, no clone. Joined at scope end.
fn parallel_sum(data: &[u64]) -> u64 {
    let mid = data.len() / 2;
    let (a, b) = data.split_at(mid);
    thread::scope(|s| {
        let h = s.spawn(|| a.iter().sum::<u64>());
        let right: u64 = b.iter().sum();
        h.join().unwrap() + right
    })
}
```

```rust
use rayon::prelude::*;

// Data parallelism: swap .iter() for .par_iter(). Guaranteed data-race free.
fn sum_squares(v: &[f64]) -> f64 {
    v.par_iter().map(|x| x * x).sum()
}
```

`parking_lot` vs `std`: std's `Mutex`/`RwLock` are now good (small, no unavoidable poisoning overhead). Use `parking_lot` only if you profile a hot lock and need its smaller/faster primitives or features like fair unlocking. For atomics, default to `Ordering::Relaxed` for counters and `Acquire`/`Release` for lock-free handoff; only use `SeqCst` when you genuinely need a single total order.

## Async with tokio

**`tokio` (1.52) is the default runtime.** Use `#[tokio::main]` for binaries, `#[tokio::test]` for async tests. Use `tokio::sync` primitives inside async (never a blocking `std::sync::Mutex` held across `.await`). Offload CPU-bound or blocking work with `spawn_blocking`.

```rust
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Structured concurrency: JoinSet owns tasks and cleans them up on drop.
    let mut set = JoinSet::new();
    for id in 0..8u32 {
        set.spawn(async move { work(id).await });
    }
    let mut total = 0u32;
    while let Some(res) = set.join_next().await {
        total += res??;   // first `?` = JoinError, second `?` = task's Result
    }
    println!("total = {total}");
    Ok(())
}

async fn work(id: u32) -> anyhow::Result<u32> { Ok(id * 2) }
```

### Cancellation and `select!` safety

`tokio::select!` drops the losing futures — so every branch must be **cancellation-safe** (dropping it mid-flight must not lose committed data). Reading with `AsyncReadExt::read` is cancel-safe; a multi-step "read then write" is not — hoist such state out of the branch. Use `CancellationToken` (from `tokio-util` 0.7) for cooperative shutdown.

```rust
use tokio_util::sync::CancellationToken;
use tokio::time::{sleep, Duration};

async fn worker(cancel: CancellationToken) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                // clean shutdown path
                break;
            }
            _ = sleep(Duration::from_millis(100)) => {
                // do a unit of cancel-safe work
            }
        }
    }
}
```

```rust
// Blocking or CPU-bound work must NOT run on the async runtime threads.
async fn hash_file(path: String) -> std::io::Result<u64> {
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path)?;          // blocking IO — fine here
        Ok(bytes.iter().map(|&b| b as u64).sum())
    })
    .await
    .expect("spawn_blocking panicked")
}
```

When to avoid async entirely: CLIs, batch tools, and CPU-bound programs are simpler and often faster with plain threads + `rayon`. Don't pull in tokio for a program that makes three HTTP calls — use `reqwest::blocking` or just threads.

## Web services: axum + tower + tracing

**`axum` (0.8.9)** is the mainstream choice (built on `hyper` + `tower`); prefer it over `actix-web` for new work unless you specifically need actor patterns. Path parameters use **`/{id}`** syntax (changed from `/:id` in 0.8 — the old syntax now panics at router construction). Serve with `axum::serve` + a `tokio::net::TcpListener` (the old `axum::Server` is gone). Native async traits mean `#[async_trait]` is no longer needed for extractors.

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState { greeting: String }

#[derive(Serialize)]
struct User { id: u64, name: String }

#[derive(Deserialize)]
struct CreateUser { name: String }

// One error type for the whole API, rendered via IntoResponse.
enum AppError {
    NotFound,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

async fn get_user(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<User>, AppError> {
    if id == 0 { return Err(AppError::NotFound); }
    Ok(Json(User { id, name: format!("{}#{id}", state.greeting) }))
}

async fn create_user(Json(body): Json<CreateUser>) -> (StatusCode, Json<User>) {
    (StatusCode::CREATED, Json(User { id: 1337, name: body.name }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = Arc::new(AppState { greeting: "user".into() });
    let app = Router::new()
        .route("/users/{id}", get(get_user))   // 0.8 path syntax
        .route("/users", post(create_user))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

`State<T>` requires the state type be `Clone` (commonly `Arc<AppState>`); `Json<T>` works both as an extractor (request body → `Deserialize`) and as a response (`Serialize` → JSON). Middleware comes from **`tower` / `tower-http`** (timeouts, compression, CORS, tracing) — layer it with `.layer(...)`. Because axum uses `tower::Service`, this middleware is shared with `tonic` and raw hyper.

### Structured logging with tracing

```rust
use tracing::{info, instrument, warn};

#[instrument(skip(db))]                 // auto-creates a span with the args
async fn handle_order(db: &Db, order_id: u64) -> anyhow::Result<()> {
    info!(order_id, "processing order");
    if order_id == 0 {
        warn!("suspicious order id");
    }
    Ok(())
}
```

Init once at startup with `tracing_subscriber::fmt().with_env_filter(...)` (human logs) or `.json()` (production/aggregation). `tracing` replaces `log` for anything with async or spans; the two interoperate via `tracing-log`.

### Database: sqlx vs diesel vs sea-orm

| Crate | Model | Choose when |
|---|---|---|
| **sqlx** (0.9) | Async, compile-time-checked raw SQL, no DSL | You want real SQL + async + compile-time verification. Default. |
| **diesel** | Sync, type-safe DSL + macros | You want a full ORM/query builder and sync is fine. |
| **sea-orm** | Async ORM on top of sqlx | You want an active-record-style ORM with async. |

```rust
use sqlx::postgres::PgPoolOptions;

#[derive(sqlx::FromRow)]
struct User { id: i64, name: String }

async fn find(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<Option<User>> {
    // query_as! verifies columns/types against the live DB at compile time.
    sqlx::query_as!(User, "SELECT id, name FROM users WHERE id = $1", id)
        .fetch_optional(pool)
        .await
}

async fn make_pool(url: &str) -> sqlx::Result<sqlx::PgPool> {
    PgPoolOptions::new().max_connections(10).connect(url).await
}
```

Use `sqlx migrate` (via `sqlx-cli`, 0.9) for versioned migrations; pool once at startup and share the `PgPool` (it's `Clone` and cheap — internally `Arc`) via axum `State`.

## Serialization with serde

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    listen_port: u16,
    #[serde(default)]                       // missing → Default
    verbose: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(flatten)]                        // merge nested fields inline
    extra: Extra,
}

#[derive(Serialize, Deserialize)]
struct Extra { region: String }

// Tagged enum: {"type":"create","id":1}  — great for message protocols.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Create { id: u64 },
    Delete { id: u64 },
}

// Zero-copy: borrow &str straight out of the input buffer (no allocation).
#[derive(Deserialize)]
struct Ref<'a> {
    #[serde(borrow)]
    name: &'a str,
}
```

Use `serde_json` (1.0.151) by default; reach for `simd-json` only when JSON parsing is a proven hot path. Prefer `#[serde(deny_unknown_fields)]` on config/DTO types to catch typos. When implementing a data format (not deriving), you may depend on `serde_core` for faster parallel builds — but for normal derive users, always just depend on `serde` with `features = ["derive"]`.

## Testing

Unit tests live in-module under `#[cfg(test)]`; integration tests go in `tests/`; doc examples in `///` blocks compile and run under `cargo test`. Use **`cargo nextest`** (0.9.140) as the day-to-day runner (faster, better output; note it does not run doctests — run those with `cargo test --doc`).

```rust
pub fn add(a: i32, b: i32) -> i32 { a + b }

/// Adds two numbers.
///
/// ```
/// assert_eq!(mycrate::add(2, 3), 5);   // doc test — compiled and run
/// ```
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn panics_on_bad_input() {
        panic!("overflow");
    }

    #[tokio::test]
    async fn async_case() {
        assert_eq!(tokio::spawn(async { 21 * 2 }).await.unwrap(), 42);
    }
}
```

| Tool | Use for | Current |
|---|---|---|
| **cargo-nextest** | Fast parallel test runner | 0.9.140 |
| **insta** | Snapshot testing (assert against reviewed snapshots) | 1.48.0 |
| **proptest** | Property-based / generative testing | 1.9.0 |
| **mockall** | Mocking traits | 0.13.1 |
| **criterion** | Statistical benchmarks + HTML reports | 0.8.2 |
| **divan** | Ergonomic benchmarks, easy CI | current |

```rust
// proptest: assert a property over many generated inputs.
proptest::proptest! {
    #[test]
    fn round_trips(s in ".*") {
        let encoded = s.as_bytes().to_vec();
        proptest::prop_assert_eq!(String::from_utf8(encoded).unwrap(), s);
    }
}
```

```rust
// criterion benchmark in benches/bench.rs (with harness = false in Cargo.toml)
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;      // std::hint::black_box, stable since 1.66

fn bench(c: &mut Criterion) {
    c.bench_function("fib20", |b| b.iter(|| fib(black_box(20))));
}
fn fib(n: u64) -> u64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
criterion_group!(benches, bench);
criterion_main!(benches);
```

Prefer trait-based fakes over heavy mocking where possible: define a trait for the dependency, implement a real and a test version. Use `assert_matches!` (stable) for asserting on enum variants.

## Unsafe code and FFI discipline

Default to `#![forbid(unsafe_code)]` at the crate root. When `unsafe` is genuinely required (FFI, a proven-necessary optimization, or building a safe abstraction over raw memory), every `unsafe` block gets a `// SAFETY:` comment stating the invariants that make it sound. Under edition 2024, `unsafe fn` bodies still need inner `unsafe {}` blocks (`unsafe_op_in_unsafe_fn`).

```rust
use std::mem::MaybeUninit;
use std::ptr::NonNull;

/// Fills a buffer via a C function that writes exactly `len` bytes.
///
/// # Safety
/// `ptr` must be valid for writes of `len` bytes.
unsafe fn fill(ptr: NonNull<u8>, len: usize) {
    // SAFETY: caller guarantees ptr is valid for `len` writes; we write within bounds.
    unsafe {
        std::ptr::write_bytes(ptr.as_ptr(), 0, len);
    }
}

fn zeroed_page() -> [u8; 4096] {
    let mut buf = MaybeUninit::<[u8; 4096]>::uninit();
    // SAFETY: we initialize all 4096 bytes before assuming init.
    unsafe {
        std::ptr::write_bytes(buf.as_mut_ptr() as *mut u8, 0, 4096);
        buf.assume_init()
    }
}
```

FFI: use `#[repr(C)]` on types crossing the boundary, `unsafe extern "C" { … }` blocks (edition 2024), and `#[unsafe(no_mangle)]` on exported symbols. Generate bindings with `bindgen` (C → Rust) or `cbindgen` (Rust → C headers). **Run `cargo +nightly miri test`** on any crate with `unsafe` — Miri catches UB (out-of-bounds, use-after-free, invalid alignment) that normal tests miss.

## Performance

- **Pre-size collections:** `Vec::with_capacity(n)` / `HashMap::with_capacity(n)` when the size is known — avoids reallocation churn.
- **Pass slices, not owned containers:** `&[T]` over `&Vec<T>`, `&str` over `&String` (works for more callers, one less indirection).
- **`Box<[T]>` over `Vec<T>`** when you never resize — saves a `usize` and signals intent.
- **`SmallVec`** (from `smallvec`) for collections that are usually tiny — keeps them on the stack.
- **`#[inline]` discipline:** don't sprinkle it; the compiler inlines within a crate already. Use `#[inline]` on small cross-crate hot functions (generic code is inlinable regardless).
- **Faster hashing:** the default `HashMap` uses SipHash (DoS-resistant). For internal, non-adversarial maps, swap the hasher: `ahash`, `FxHashMap` (from `rustc-hash`), or `hashbrown` with a fast hasher — often a large speedup on small keys.
- **`dyn` to cut compile time:** heavily-monomorphized generics bloat binaries and compile time. Using `&dyn Trait` at a few call sites can shrink both — a real tradeoff worth making in large codebases.
```rust
use std::collections::HashMap;

fn dedup_sorted(mut v: Vec<u32>) -> Box<[u32]> {
    v.sort_unstable();          // sort_unstable is faster and allocation-free
    v.dedup();
    v.into_boxed_slice()        // no spare capacity retained
}

// Pre-sized, fast-hasher map for internal counting (not attacker-controlled keys).
fn counts(words: &[&str]) -> HashMap<&str, u32> {
    let mut m = HashMap::with_capacity(words.len());
    for &w in words { *m.entry(w).or_insert(0) += 1; }
    m
}
```

Release profile config (LTO, one codegen unit, `panic=abort`) from the Profile section above is where most real speedups come from for shipping binaries. Profile with `cargo flamegraph` before optimizing — never guess.

## Modules, visibility, and documentation

Use the **no-`mod.rs`** layout: a module `foo` with children lives in `foo.rs` plus a `foo/` directory (not `foo/mod.rs`). Keep internals `pub(crate)`; expose a curated public surface via re-exports (the facade pattern).

```rust
// src/lib.rs
#![warn(missing_docs)]
//! # mycrate
//! A one-line crate summary that appears on docs.rs.

mod store;      // -> src/store.rs
mod api;        // -> src/api.rs (+ src/api/ for children)

// Facade: re-export the public API so users write `mycrate::Store`.
pub use store::Store;

/// A key-value store.
pub struct Store { /* … */ }
```

```rust
// Feature-gated item, documented as gated on docs.rs:
#[cfg(feature = "postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "postgres")))]
pub fn connect_pg() { /* … */ }
```

Use `#[doc(hidden)]` for public-but-not-really items (macro helpers). Every public item should have a doc comment; enable `#![warn(missing_docs)]` so the compiler enforces it. Build docs locally with `cargo doc --no-deps --open`.

## `const` and `no_std` (brief)

`const fn` has grown substantially — much of arithmetic, slicing, and control flow works in `const` context now; `char::is_control` became const in 1.97. **Const generics** parameterize types by values:

```rust
// Const generic: a fixed-size ring buffer with no heap allocation.
struct Ring<const N: usize> { buf: [u8; N], head: usize }

impl<const N: usize> Ring<N> {
    const fn new() -> Self { Self { buf: [0; N], head: 0 } }
}

const LOOKUP: [u32; 4] = { let mut a = [0; 4]; a[1] = 1; a };  // const block
```

For `no_std` (embedded), add `#![no_std]` and rely on `core` + `alloc`. The `Error` trait is in `core` (since 1.81), and many crates (`serde`, `heapless`) support `no_std` via features. This is niche — only go `no_std` when targeting bare metal or WASM without an allocator.

## Tooling configuration (copy-ready)

`rust-toolchain.toml` — pins the toolchain for reproducible builds:

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

`deny.toml` (for `cargo deny` — license + advisory + source policy):

```toml
[advisories]
yanked = "deny"
[bans]
multiple-versions = "warn"
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-3.0"]
```

CI gate — the commands that must pass on every PR:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --all-features
cargo test --doc                # nextest doesn't run doctests
cargo deny check                # licenses + advisories + bans
cargo machete                   # unused dependencies (0.9.2)
cargo semver-checks             # (for libs) catch accidental breaking changes (0.47.0)
```

`Cargo.lock` policy: **commit it for binaries/applications** (reproducible builds); **do not commit it for libraries** (let downstream resolve). Manage deps with `cargo add`/`cargo update`; audit with `cargo audit` (or `cargo deny check advisories`). Use `cargo build --timings` and `sccache` to diagnose/speed up compile times. `cargo udeps` (0.1.61, nightly) finds unused deps that `machete` may miss.

## Anti-patterns: wrong vs right

**1. Cloning to dodge the borrow checker**
```rust
// WRONG: needless allocation to satisfy lifetimes
fn greet(name: String) -> String { format!("hi {name}") }
let n = String::from("ada");
greet(n.clone());  // clone just to keep `n`

// RIGHT: borrow
fn greet(name: &str) -> String { format!("hi {name}") }
greet(&n);         // n still usable
```

**2. `Arc<Mutex<…>>` reflex for read-mostly shared state**
```rust
// WRONG: serializes all readers behind a Mutex
let cache: Arc<Mutex<HashMap<K, V>>> = ...;

// RIGHT: many concurrent readers with RwLock (or dashmap for high contention)
let cache: Arc<RwLock<HashMap<K, V>>> = ...;
```

**3. `unwrap()` in library code**
```rust
// WRONG: panics on the caller's behalf
pub fn parse(s: &str) -> Config { serde_json::from_str(s).unwrap() }

// RIGHT: return a typed error
pub fn parse(s: &str) -> Result<Config, serde_json::Error> { serde_json::from_str(s) }
```

**4. Inheritance-style trait-object trees**
```rust
// WRONG (C++ habit): Box<dyn Base> hierarchy for behavior you could monomorphize
fn run(items: Vec<Box<dyn Animal>>) { /* virtual dispatch everywhere */ }

// RIGHT: generics + impl Trait for static dispatch; dyn only for true heterogeneity
fn run<A: Animal>(a: &A) { a.speak(); }
```

**5. Blocking inside async**
```rust
// WRONG: blocks a runtime worker thread, starves other tasks
async fn load() -> Vec<u8> { std::fs::read("big.bin").unwrap() }

// RIGHT: async IO, or spawn_blocking for unavoidable blocking work
async fn load() -> std::io::Result<Vec<u8>> { tokio::fs::read("big.bin").await }
```

**6. Holding a `std::sync::Mutex` guard across `.await`**
```rust
// WRONG: guard is not Send-safe to hold across await points → deadlocks/!Send future
let g = std_mutex.lock().unwrap();
do_async(&*g).await;

// RIGHT: use tokio::sync::Mutex, or drop the guard before awaiting
let data = { let g = std_mutex.lock().unwrap(); g.clone() };
do_async(&data).await;
```

**7. `String`/`&Vec<T>` parameters instead of borrows/slices**
```rust
// WRONG
fn total(v: &Vec<i32>) -> i32 { v.iter().sum() }

// RIGHT: accepts arrays, slices, Vec — everything
fn total(v: &[i32]) -> i32 { v.iter().sum() }
```

**8. Old `rand` / discontinued crates**
```rust
// WRONG: deprecated names + reserved keyword collision
let x = rand::thread_rng().gen_range(0..10);

// RIGHT: current rand API (0.9+): rng(), random_range()
let x = rand::rng().random_range(0..10);
```

## Quick reference: feature → version floor

| Feature | Stable since |
|---|---|
| Scoped threads (`thread::scope`) | 1.63 |
| `std::hint::black_box` | 1.66 |
| `OnceLock` | 1.70 |
| `[lints]` table in Cargo.toml | 1.74 |
| `async fn` in traits (RPITIT) | 1.75 |
| `LazyLock` / `LazyCell` | 1.80 |
| `Error` trait in `core` | 1.81 |
| `use<..>` precise capture; `&raw` operator | 1.82 |
| Async closures (`async \|\|`) | 1.85 |
| Rust 2024 edition | 1.85 |
| Trait upcasting | 1.86 |
| `let` chains | 1.88 (edition 2024) |
| v0 symbol mangling default; `build.warnings` config | 1.97 |
