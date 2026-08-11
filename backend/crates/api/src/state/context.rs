// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One handle per feature, and nothing else.

use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};

use afisharr_core::{
    filesystem::Root, secrets::SecretKey, setup::TokenStore, storage::Database, time::Clock,
};
use afisharr_plex::pin::PlexTvClient;

use crate::{
    interface::AssetSource,
    proxy::{PublicOrigin, TrustedProxies},
    ratelimit::RateLimiter,
    security::ContentSecurityPolicy,
    state::InstanceIdentity,
    stream::StreamHub,
};

/// Everything the router needs, behind one cheap clone.
///
/// Axum requires the state type to be `Clone`, so the contents sit behind one
/// `Arc` rather than each field carrying its own. Nothing here is a method per
/// feature: every accessor hands back the feature's own type, and the logic
/// lives with the feature (§24.6.3).
#[derive(Clone)]
pub struct ApiState {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ApiState {
    /// Prints the instance's identity and nothing that seals a credential.
    ///
    /// `SecretKey` is deliberately not `Debug`, and a derived implementation
    /// here would put the instance key in the first `?state` anyone writes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiState")
            .field("instance_id", &self.inner.identity.instance_id)
            .field("setup_completed", &self.setup_completed())
            .finish_non_exhaustive()
    }
}

struct Inner {
    database: Arc<Database>,
    clock: Arc<dyn Clock>,
    identity: InstanceIdentity,
    setup_completed: AtomicBool,
    secret_key: Arc<SecretKey>,
    bootstrap: Arc<TokenStore>,
    plex: PlexTvClient,
    limiter: RateLimiter,
    trusted_proxies: TrustedProxies,
    public_origin: Option<PublicOrigin>,
    asset_roots: Vec<Root>,
    stream: StreamHub,
    assets: Arc<dyn AssetSource>,
    policy: ContentSecurityPolicy,
}

/// The values the binary supplies when it builds the router.
///
/// A struct rather than a dozen positional arguments: the wiring layer is the
/// one place that names all of these, and a positional call there is one
/// transposition away from handing the limiter's clock to the token store.
pub struct ApiStateParts {
    /// The open database.
    pub database: Arc<Database>,
    /// The clock every gate reads.
    pub clock: Arc<dyn Clock>,
    /// Who this instance is.
    pub identity: InstanceIdentity,
    /// The key that seals credentials.
    pub secret_key: Arc<SecretKey>,
    /// The live bootstrap token, if setup is incomplete.
    pub bootstrap: Arc<TokenStore>,
    /// The plex.tv client the login flow uses.
    pub plex: PlexTvClient,
    /// Proxies whose forwarded headers are honoured.
    pub trusted_proxies: TrustedProxies,
    /// The origin operators reach this instance at, when one is configured.
    pub public_origin: Option<PublicOrigin>,
    /// Roots the filesystem browser may walk.
    pub asset_roots: Vec<Root>,
    /// The embedded interface.
    pub assets: Arc<dyn AssetSource>,
}

impl ApiState {
    /// Assembles the state from what the binary owns.
    #[must_use]
    pub fn new(parts: ApiStateParts) -> Self {
        let policy = ContentSecurityPolicy::with_script_digests(&shell_script_digests(
            parts.assets.as_ref(),
        ));
        let limiter = RateLimiter::new(parts.clock.clone());
        let setup_completed = AtomicBool::new(parts.identity.setup_completed);
        Self {
            inner: Arc::new(Inner {
                database: parts.database,
                clock: parts.clock,
                identity: parts.identity,
                setup_completed,
                secret_key: parts.secret_key,
                bootstrap: parts.bootstrap,
                plex: parts.plex,
                limiter,
                trusted_proxies: parts.trusted_proxies,
                public_origin: parts.public_origin,
                asset_roots: parts.asset_roots,
                stream: StreamHub::new(),
                assets: parts.assets,
                policy,
            }),
        }
    }

    /// The open database.
    #[must_use]
    pub fn database(&self) -> &Database {
        &self.inner.database
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &dyn Clock {
        self.inner.clock.as_ref()
    }

    /// Who this instance is.
    #[must_use]
    pub fn identity(&self) -> &InstanceIdentity {
        &self.inner.identity
    }

    /// The key that seals credentials.
    #[must_use]
    pub fn secret_key(&self) -> &SecretKey {
        &self.inner.secret_key
    }

    /// The live bootstrap token.
    #[must_use]
    pub fn bootstrap(&self) -> &TokenStore {
        &self.inner.bootstrap
    }

    /// The plex.tv client.
    #[must_use]
    pub fn plex(&self) -> &PlexTvClient {
        &self.inner.plex
    }

    /// The rate limiter.
    #[must_use]
    pub fn limiter(&self) -> &RateLimiter {
        &self.inner.limiter
    }

    /// The trusted-proxy list.
    #[must_use]
    pub fn trusted_proxies(&self) -> &TrustedProxies {
        &self.inner.trusted_proxies
    }

    /// The origin operators reach this instance at, if the operator set one.
    ///
    /// `None` is not "derive it from the request": the request's authority is
    /// the caller's to choose, so a route that needs an absolute URL for this
    /// instance refuses instead (`I-SEC-1`).
    #[must_use]
    pub fn public_origin(&self) -> Option<&PublicOrigin> {
        self.inner.public_origin.as_ref()
    }

    /// The roots the filesystem browser may walk.
    #[must_use]
    pub fn asset_roots(&self) -> &[Root] {
        &self.inner.asset_roots
    }

    /// The event stream.
    #[must_use]
    pub fn stream(&self) -> &StreamHub {
        &self.inner.stream
    }

    /// The embedded interface.
    #[must_use]
    pub fn assets(&self) -> &dyn AssetSource {
        self.inner.assets.as_ref()
    }

    /// The policy every response carries.
    #[must_use]
    pub fn policy(&self) -> &ContentSecurityPolicy {
        &self.inner.policy
    }

    /// Whether setup has finished.
    ///
    /// Read on every request through the setup gate, so it is an atomic rather
    /// than a query. `Acquire`/`Release` and not `Relaxed`: the flag is set
    /// after the completion write commits, and a reader that saw the flag
    /// without the write would answer 404 on the setup endpoints while the
    /// database still said otherwise.
    #[must_use]
    pub fn setup_completed(&self) -> bool {
        self.inner.setup_completed.load(Ordering::Acquire)
    }

    /// Records that setup has finished, for the life of this process.
    pub fn mark_setup_completed(&self) {
        self.inner.setup_completed.store(true, Ordering::Release);
    }
}

/// The inline-script digests of the shell this build serves.
fn shell_script_digests(assets: &dyn AssetSource) -> Vec<String> {
    assets
        .shell()
        .and_then(|shell| String::from_utf8(shell.bytes.into_owned()).ok())
        .map(|html| crate::interface::inline_script_digests(&html))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::interface::{Asset, NoAssets};

    use super::*;

    #[derive(Debug)]
    struct OneInlineScript;

    impl AssetSource for OneInlineScript {
        fn get(&self, _path: &str) -> Option<Asset> {
            None
        }

        fn shell(&self) -> Option<Asset> {
            Some(Asset {
                bytes: Cow::Borrowed(b"<html><script>start()</script></html>"),
                content_type: "text/html".to_owned(),
                immutable: false,
            })
        }
    }

    #[test]
    fn a_build_with_no_interface_admits_no_inline_script() {
        assert!(shell_script_digests(&NoAssets).is_empty());
    }

    #[test]
    fn the_policy_admits_the_shell_this_build_actually_serves() {
        let digests = shell_script_digests(&OneInlineScript);
        assert_eq!(digests, vec![afisharr_core::digest::csp_source("start()")]);
    }
}
