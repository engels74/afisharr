// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One error type, one wire shape, one status mapping.
//!
//! Every route on this surface fails through [`AppError`] and renders as
//! [`Problem`]. That is not tidiness: the generated TypeScript client narrows
//! a failure by reading `code`, and a handler that answered with its own
//! `(StatusCode, String)` tuple would be a response the client cannot type and
//! the interface has to guess at — which is exactly the guessing `I-UX-2`
//! forbids.

mod app_error;
mod code;
mod extractors;
mod problem;

pub use app_error::{AppError, AppResult};
pub use code::ErrorCode;
pub use extractors::{JsonBody, QueryParams};
pub use problem::{Mismatch, Problem};
