// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Configuration as one versioned document.
//!
//! Settings are one JSON body deserialised into a typed struct that rejects
//! unknown fields, never a key-value table (PRD §19.5). They are read as a unit
//! at pass start, written as a unit by the settings page, and validated as a
//! unit — several of them are only meaningful together. A key-value table
//! produces partial writes that no validator ever sees.
//!
//! Credentials are not here. They live in [`crate::secrets`], because this body
//! is diffed into `settings_history`, is a candidate for export, and would
//! preserve a rotated token forever (D-032).

mod body;
mod error;
mod store;

pub use body::{
    BackupSettings, HttpSettings, InstanceSettings, LoggingSettings, RenderSettings, SettingsBody,
};
pub use error::SettingsError;
pub use store::{SaveSettings, Settings, load};
