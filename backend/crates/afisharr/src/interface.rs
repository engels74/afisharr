// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The prerendered SPA, embedded in this binary.
//!
//! One file is the shipped artefact (PRD §3), so the interface travels inside
//! the executable rather than beside it. Embedding lives in the binary crate
//! because that is the crate the release is built from; `afisharr-api` serves
//! through the [`AssetSource`](afisharr_api::interface::AssetSource) seam and
//! never knows where the bytes came from.

mod embedded;

pub use embedded::EmbeddedInterface;
