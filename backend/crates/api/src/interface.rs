// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Serving the prerendered SPA the binary carries.
//!
//! The assets themselves are embedded by the binary crate, which is where
//! `include`-at-compile-time belongs; this module holds the seam
//! ([`AssetSource`]), the route that serves through it, and the digest of the
//! one inline script the shell contains, which the CSP admits by hash rather
//! than by `'unsafe-inline'`.

mod assets;
mod route;
mod script_digest;

pub use assets::{Asset, AssetSource, NoAssets};
pub use route::spa;
pub use script_digest::inline_script_digests;
