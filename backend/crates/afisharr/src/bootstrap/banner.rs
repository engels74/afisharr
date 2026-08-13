// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the operator reads off the console on a first start.

use std::net::SocketAddr;

/// Prints the setup banner.
///
/// Called only while `instance.setup_completed_at` is `NULL`. The three events
/// that end the token's life are all stated, because an operator who does not
/// know a restart mints a new one will paste a dead token and conclude the
/// product is broken (PRD §19.6.1).
///
/// `println!` rather than `info!`: the tracing subscriber writes to
/// `logs/afisharr.log`, and the token must never reach it.
pub fn print_setup_banner(token: &str, bound: SocketAddr) {
    let url = setup_url(bound);
    println!();
    println!("┌────────────────────────────────────────────────────────────┐");
    println!("│  Afisharr is not set up yet.                               │");
    println!("└────────────────────────────────────────────────────────────┘");
    println!();
    println!("  Open   {url}");
    println!("  Token  {token}");
    println!();
    println!("  This token stops working when any of these happens:");
    println!("    · fifteen minutes pass");
    println!("    · this process restarts, which prints a new one");
    println!("    · setup completes");
    println!();
}

/// The address to open, composed from the socket that is actually bound.
///
/// From `bound` rather than from the configured `HttpSettings`, because the two
/// disagree in the case that matters: `port = 0` asks the operating system to
/// choose, so the document holds `0` and the instance is reachable somewhere
/// else. The banner has one job, and printing a URL nothing answers on fails
/// it.
///
/// `0.0.0.0` and `::` are what a container binds to, and neither is somewhere a
/// browser can go. They are rendered as `localhost`, which is where the
/// operator reading this console actually is.
fn setup_url(bound: SocketAddr) -> String {
    if bound.ip().is_unspecified() {
        return format!("http://localhost:{}/setup", bound.port());
    }
    // `SocketAddr`'s own `Display` brackets an IPv6 literal and appends the
    // port, which is exactly the shape a URL wants.
    format!("http://{bound}/setup")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(address: &str) -> SocketAddr {
        address.parse().expect("a valid socket address")
    }

    #[test]
    fn a_wildcard_bind_is_rendered_as_somewhere_a_browser_can_go() {
        assert_eq!(
            setup_url(bound("0.0.0.0:8484")),
            "http://localhost:8484/setup"
        );
        assert_eq!(setup_url(bound("[::]:8484")), "http://localhost:8484/setup");
    }

    #[test]
    fn a_specific_bind_address_is_used_as_written() {
        assert_eq!(
            setup_url(bound("192.168.1.10:9000")),
            "http://192.168.1.10:9000/setup"
        );
    }

    #[test]
    fn an_ipv6_literal_is_bracketed_so_the_port_is_not_read_as_part_of_it() {
        assert_eq!(
            setup_url(bound("[fd00::1]:8484")),
            "http://[fd00::1]:8484/setup"
        );
    }

    #[test]
    fn the_port_printed_is_the_one_the_socket_got() {
        // `port = 0` asks the operating system to choose, so the configured
        // document says `0` and the instance answers somewhere else. Composed
        // from settings, the banner sent the operator to `localhost:0`.
        assert_eq!(
            setup_url(bound("0.0.0.0:41337")),
            "http://localhost:41337/setup"
        );
    }

    #[test]
    fn the_url_points_at_the_setup_page_rather_than_the_root() {
        assert!(setup_url(bound("0.0.0.0:8484")).ends_with("/setup"));
    }
}
