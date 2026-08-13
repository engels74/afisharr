// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where this instance keeps its files, and what it was configured with.

mod load;
mod paths;

pub use load::{Configuration, apply_deployment_environment, load};
pub use paths::DataPaths;
