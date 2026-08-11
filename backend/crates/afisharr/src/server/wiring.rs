// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Assembling the API's state from what the boot sequence produced.

use std::sync::Arc;

use afisharr_api::{
    interface::AssetSource,
    proxy::{PublicOrigin, TrustedProxies},
    state::{ApiState, ApiStateParts, InstanceIdentity},
};
use afisharr_core::{
    filesystem::enabled_roots,
    locale::LocaleTag,
    setup::TokenStore,
    time::{Clock, SystemClock},
};
use afisharr_plex::{identity::ClientIdentity, pin::PlexTvClient};
use afisharr_sources::outbound::OutboundClient;
use anyhow::{Context, Result};

use crate::{interface::EmbeddedInterface, startup::Booted};

/// The `User-Agent` every outbound request carries.
///
/// Named and versioned, because a provider that has to rate-limit Afisharr
/// should be able to tell it apart from a browser and from every other client
/// on the operator's network.
fn user_agent(version: &str) -> String {
    format!("Afisharr/{version} (+https://github.com/engels74/afisharr)")
}

/// Builds the router's state from a booted instance.
///
/// # Errors
/// Returns an error when the configured `trustProxy` list holds an entry that
/// is not an address or a range, when the instance's own values cannot be sent
/// as HTTP headers, when the outbound transport cannot be constructed, or when
/// the enabled asset roots cannot be read.
pub async fn build_state(booted: &Booted, bootstrap: Arc<TokenStore>) -> Result<ApiState> {
    build_state_against(booted, bootstrap, None).await
}

/// The same, with plex.tv's API root chosen by the caller.
///
/// `None` is plex.tv, and is what the binary passes. The parameter exists
/// because the PIN exchange is otherwise untestable without the real service,
/// and because the adversarial fake (D-036) is what every phase from Phase 4
/// onward tests against.
///
/// # Errors
/// As [`build_state`].
pub async fn build_state_against(
    booted: &Booted,
    bootstrap: Arc<TokenStore>,
    plex_base: Option<&str>,
) -> Result<ApiState> {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let trusted_proxies = TrustedProxies::parse(&booted.settings.body.http.trust_proxy)
        .context("reading the configured trustProxy list")?;

    // Refused rather than ignored: an operator who mistyped this would get an
    // instance that quietly refuses every hosted Plex sign-in, with nothing in
    // front of them saying which of the two ends is wrong.
    let public_origin = booted
        .settings
        .body
        .http
        .public_origin
        .as_deref()
        .map(PublicOrigin::parse)
        .transpose()
        .context("reading the configured publicOrigin")?;

    let plex_identity = ClientIdentity::new(
        &booted.instance.client_identifier,
        &booted.instance.device_name,
        &booted.instance.app_version,
    )
    .context("building the X-Plex-* header set from the instance row")?;

    let outbound = OutboundClient::new(&user_agent(&booted.instance.app_version))
        .context("building the outbound HTTP client")?;

    let locale = LocaleTag::parse(&booted.instance.locale)
        .with_context(|| format!("reading the configured locale '{}'", booted.instance.locale))?;

    let assets: Arc<dyn AssetSource> = Arc::new(EmbeddedInterface);

    // Read from `asset_roots` rather than from a constant: the operator adds
    // roots from the interface, and a browser whose list came from anywhere
    // else would reach somewhere they did not allow.
    let asset_roots = enabled_roots(booted.database.readers())
        .await
        .context("reading the enabled asset roots")?;

    Ok(ApiState::new(ApiStateParts {
        database: Arc::clone(&booted.database),
        clock: Arc::clone(&clock),
        identity: InstanceIdentity {
            instance_id: booted.instance.instance_id.clone(),
            client_identifier: booted.instance.client_identifier.clone(),
            locale,
            app_version: booted.instance.app_version.clone(),
            setup_completed: booted.instance.setup_completed_at.is_some(),
        },
        secret_key: Arc::clone(&booted.secret_key),
        bootstrap,
        plex: match plex_base {
            Some(base) => PlexTvClient::against(outbound, plex_identity, base),
            None => PlexTvClient::new(outbound, plex_identity),
        },
        trusted_proxies,
        public_origin,
        asset_roots,
        assets,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_the_product_and_its_version() {
        let agent = user_agent("0.1.0");
        assert!(agent.starts_with("Afisharr/0.1.0"), "{agent}");
        assert!(agent.contains("github.com/engels74/afisharr"), "{agent}");
    }
}
