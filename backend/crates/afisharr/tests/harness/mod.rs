// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared scaffolding for the binary's integration tests.
//!
//! Compiled into every test binary that declares it, so an item only one of
//! them needs looks dead to the others.
#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::struct_field_names
)]

mod fixtures;
mod plex_attempt;
mod plex_tv_stub;
mod running_instance;
mod temp_instance;
mod wizard;

pub use fixtures::{
    InsertIdMapping, InsertLifecycleSubject, InsertPlexPrincipal, InsertVisibility, seed_library,
};
pub use plex_attempt::{Attempt, CSRF_HEADER, browser};
pub use plex_tv_stub::PlexTvStub;
pub use running_instance::RunningInstance;
pub use temp_instance::TempInstance;
pub use wizard::{Wizard, csrf_from};
