// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the operator reads off the console on a first start.

use afisharr_core::settings::HttpSettings;

/// Prints the setup banner.
///
/// Called only while `instance.setup_completed_at` is `NULL`. The three events
/// that end the token's life are all stated, because an operator who does not
/// know a restart mints a new one will paste a dead token and conclude the
/// product is broken (PRD §19.6.1).
///
/// `println!` rather than `info!`: the tracing subscriber writes to
/// `logs/afisharr.log`, and the token must never reach it.
pub fn print_setup_banner(token: &str, http: &HttpSettings) {
    let url = setup_url(http);
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

/// The address to open, composed from the configured bind address and port.
///
/// `0.0.0.0` and `::` are what a container binds to, and neither is somewhere a
/// browser can go. They are rendered as `localhost`, which is where the
/// operator reading this console actually is.
fn setup_url(http: &HttpSettings) -> String {
    let host = match http.bind_address.as_str() {
        "0.0.0.0" | "::" | "[::]" | "" => "localhost",
        configured => configured,
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    format!("http://{host}:{}/setup", http.port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(bind: &str, port: u16) -> HttpSettings {
        HttpSettings {
            bind_address: bind.to_owned(),
            port,
            trust_proxy: Vec::new(),
        }
    }

    #[test]
    fn a_wildcard_bind_is_rendered_as_somewhere_a_browser_can_go() {
        assert_eq!(
            setup_url(&http("0.0.0.0", 8484)),
            "http://localhost:8484/setup"
        );
        assert_eq!(setup_url(&http("::", 8484)), "http://localhost:8484/setup");
    }

    #[test]
    fn a_specific_bind_address_is_used_as_written() {
        assert_eq!(
            setup_url(&http("192.168.1.10", 9000)),
            "http://192.168.1.10:9000/setup"
        );
    }

    #[test]
    fn an_ipv6_literal_is_bracketed_so_the_port_is_not_read_as_part_of_it() {
        assert_eq!(
            setup_url(&http("fd00::1", 8484)),
            "http://[fd00::1]:8484/setup"
        );
    }

    #[test]
    fn the_url_points_at_the_setup_page_rather_than_the_root() {
        assert!(setup_url(&http("0.0.0.0", 8484)).ends_with("/setup"));
    }
}
