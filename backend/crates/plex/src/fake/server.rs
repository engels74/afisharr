// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Starting the fake, and the handles a test drives it with.

use std::sync::Arc;

use crate::fake::{instance::FakeInstance, library::World, routes, scenario::Scenario};

/// A running fake Plex server.
///
/// Bound to a port the operating system chose, so any number of them can run at
/// once — which is what lets one test hold two servers and swap between them,
/// the shape `I-ID-5` is written against.
#[derive(Debug)]
pub struct FakePlex {
    base_url: String,
    instance: Arc<FakeInstance>,
    task: tokio::task::JoinHandle<()>,
}

impl FakePlex {
    /// Starts a server running `scenario`.
    ///
    /// # Panics
    /// Panics when no port can be bound, which in a test means the machine has
    /// no loopback interface and nothing else here will work either.
    pub async fn start(scenario: Scenario) -> Self {
        let instance = Arc::new(FakeInstance::new(&scenario));
        let app = routes::router(Arc::clone(&instance));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the fake must bind a loopback port");
        let address = listener
            .local_addr()
            .expect("a bound listener has an address");
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            base_url: format!("http://{address}"),
            instance,
            task,
        }
    }

    /// The address to point a [`crate::server::PlexServerClient`] at.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The machine identifier this server is currently answering with.
    #[must_use]
    pub fn machine_identifier(&self) -> String {
        self.instance.world().machine_identifier.clone()
    }

    /// Makes this server answer as a different one, on demand.
    ///
    /// The `I-ID-5` trigger. Nothing else about the world changes, which is the
    /// realistic shape: the operator pointed the same address at another
    /// machine, and every rating key in the database now means something else.
    pub fn becomes_a_different_server(&self, machine_identifier: &str) {
        machine_identifier.clone_into(&mut self.instance.world().machine_identifier);
    }

    /// Re-keys every item, keeping each one's guid.
    ///
    /// Rating-key churn on demand (`I-ID-1`). The scheduled form is
    /// [`FakePlex::churn_after_fetches`], for a churn that lands mid-pass.
    pub fn churn_rating_keys(&self) {
        self.instance.world().churn_rating_keys();
    }

    /// Churns the rating keys once `fetches` item listings have been served.
    pub fn churn_after_fetches(&self, fetches: u32) {
        self.instance.churn_at_fetch(Some(fetches));
    }

    /// Puts a label on one item, as a Plex-side edit this instance did not make.
    ///
    /// The shape an adoption test needs: something changed in Plex, and the
    /// next pass has to notice rather than overwrite.
    pub fn label_item(&self, rating_key: &str, label: &str) {
        let mut world = self.instance.world();
        for library in &mut world.libraries {
            if let Some(item) = library
                .items
                .iter_mut()
                .find(|item| item.rating_key == rating_key)
            {
                item.labels.push(label.to_owned());
            }
        }
    }

    /// Reads the world, for a test's assertions.
    ///
    /// Handed out as a clone rather than as a guard: a test holding the lock
    /// while it awaits the client would deadlock against the handler serving
    /// that same request, and the deadlock would look like a hung test.
    #[must_use]
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot(self.instance.world().clone())
    }

    /// Stops the server.
    ///
    /// Aborted rather than shut down gracefully: a scenario that stalls a call
    /// is holding a request open for an hour by design, and a graceful shutdown
    /// would wait for it.
    pub fn stop(self) {
        self.task.abort();
    }
}

impl Drop for FakePlex {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A copy of the fake's world, taken at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSnapshot(World);

impl WorldSnapshot {
    /// The rating keys of one library's items, in order.
    #[must_use]
    pub fn item_keys(&self, section: &str) -> Vec<String> {
        self.0
            .libraries
            .iter()
            .find(|library| library.key == section)
            .map(|library| {
                library
                    .items
                    .iter()
                    .map(|item| item.rating_key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The hub identifiers of one library's ordering space, in order.
    #[must_use]
    pub fn hub_order(&self, section: &str) -> Vec<String> {
        self.0
            .libraries
            .iter()
            .find(|library| library.key == section)
            .map(|library| {
                library
                    .hubs
                    .iter()
                    .map(|hub| hub.identifier.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The labels on one item, or `None` when no such item exists.
    #[must_use]
    pub fn labels(&self, rating_key: &str) -> Option<Vec<String>> {
        self.0
            .libraries
            .iter()
            .flat_map(|library| library.items.iter())
            .find(|item| item.rating_key == rating_key)
            .map(|item| item.labels.clone())
    }

    /// The items of one collection, in order.
    #[must_use]
    pub fn collection_items(&self, collection: &str) -> Vec<String> {
        self.0
            .libraries
            .iter()
            .flat_map(|library| library.collections.iter())
            .find(|candidate| candidate.rating_key == collection)
            .map(|candidate| candidate.items.clone())
            .unwrap_or_default()
    }

    /// The sort title of one collection: its value and its lock state.
    #[must_use]
    pub fn collection_sort_title(&self, collection: &str) -> Option<(Option<String>, bool)> {
        self.0
            .libraries
            .iter()
            .flat_map(|library| library.collections.iter())
            .find(|candidate| candidate.rating_key == collection)
            .map(|candidate| (candidate.sort_title.clone(), candidate.sort_title_locked))
    }
}
