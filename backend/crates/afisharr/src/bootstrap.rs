// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The console banner that carries the first-run token.
//!
//! Printed to stdout and to stdout only. The token never reaches the database,
//! never reaches `logs/afisharr.log`, and never reaches a response body
//! (`I-SEC-8`) — which is why this writes with `println!` rather than through
//! `tracing`, where the file layer would pick it up.

mod banner;

pub use banner::print_setup_banner;
