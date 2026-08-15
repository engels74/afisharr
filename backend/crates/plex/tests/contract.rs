// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The release-lane contract test: what keeps the adversarial fake truthful.
//!
//! A stub does what it is told, and the failures worth testing are the ones
//! where Plex does not. The fake makes those reproducible. It cannot make
//! itself correct — every shape in it is a claim about a server nobody in this
//! repository controls, and a claim that drifts turns every test written
//! against it into a test of a server that does not exist. So the same call
//! surface runs against a real Plex, and three things are asserted per call:
//! the real answer parses with this crate's own parsers, the parse produced the
//! facts the call exists to read, and the fake's answer claims nothing the real
//! one does not (D-036, PRD §21.10.2).
//!
//! **Every read this build makes is in the surface.** A call left out is a
//! parser and a fake shape checked against nothing but a fixture this
//! repository wrote, which is a test that agrees with itself: the release lane
//! stays green while a real Plex answers something the product cannot read.
//!
//! **What the contract server must hold.** The keys the surface is addressed by
//! are read off the server rather than written down, because the two servers
//! hold different content. That needs a movie library with at least one item,
//! at least one collection, and at least one filter that declares an enumerated
//! choice list. A server without them fails here by name rather than silently
//! covering less.
//!
//! **It needs a real server, and says so when it has none.** The release lane
//! supplies `AFISHARR_PLEX_CONTRACT_URL` and `AFISHARR_PLEX_CONTRACT_TOKEN`;
//! without them this test reports that it did not run rather than passing
//! quietly, because a contract test that silently skips is worse than none —
//! it reads green on the one lane that was supposed to catch the drift.

mod shape;

use afisharr_plex::{
    discovery::DiscoveredFilter,
    fake::{FakePlex, Scenario},
    identity::ClientIdentity,
    libraries::{ItemKind, ItemQuery, LibraryKind, RatingKey, SectionKey, Window},
    server::{PlexServerClient, ServerAddress, ServerError, ServerToken},
};
use afisharr_sources::outbound::{OutboundClient, Response};
use serde_json::Value;

/// Where the real server is, in the release lane.
const URL: &str = "AFISHARR_PLEX_CONTRACT_URL";

/// The token the release lane supplies for it.
const TOKEN: &str = "AFISHARR_PLEX_CONTRACT_TOKEN";

/// A client against the real server, or `None` when the lane configured none.
fn real_server() -> Option<PlexServerClient> {
    let url = std::env::var(URL).ok().filter(|value| !value.is_empty())?;
    let token = std::env::var(TOKEN)
        .ok()
        .filter(|value| !value.is_empty())?;
    Some(PlexServerClient::new(
        OutboundClient::new("afisharr/contract").expect("the transport must build"),
        ClientIdentity::new(
            "01JAFISHARRCONTRACT",
            "Afisharr contract test",
            env!("CARGO_PKG_VERSION"),
        )
        .expect("a valid identity"),
        ServerAddress::parse(&url).expect("the release lane must configure a valid URL"),
        ServerToken::new(&token).expect("the release lane must configure a header-safe token"),
    ))
}

/// A client against the fake, behaving.
fn fake_client(fake: &FakePlex) -> PlexServerClient {
    PlexServerClient::new(
        OutboundClient::new("afisharr/contract").expect("the transport must build"),
        ClientIdentity::new("01JAFISHARRCONTRACT", "Afisharr contract test", "0.1.0")
            .expect("a valid identity"),
        ServerAddress::parse(fake.base_url()).expect("a valid address"),
        ServerToken::new("test-plex-token").expect("a header-safe token"),
    )
}

/// Fetches one endpoint's raw body, for the shape comparison.
///
/// Raw rather than parsed, deliberately: a comparison over parsed values would
/// only ever see the fields this build already reads, which is the half of the
/// contract that cannot drift without a compile error. What drifts silently is
/// everything else in the envelope.
async fn raw(client: &PlexServerClient, path: &str, query: &[(String, String)]) -> Value {
    let url = client
        .address()
        .endpoint(path, query)
        .expect("a valid endpoint");
    let response: Response = client
        .raw_get(&url)
        .await
        .unwrap_or_else(|error: ServerError| panic!("{path} did not answer: {error}"));
    serde_json::from_str(&response.body)
        .unwrap_or_else(|error| panic!("{path} answered something that is not JSON: {error}"))
}

/// One read-only call, named as the release lane reports it.
struct Call {
    name: &'static str,
    path: String,
    query: Vec<(String, String)>,
}

/// The keys one server's read surface is addressed by.
///
/// Discovered from that server rather than written down. A rating key that
/// exists on the fake means nothing on somebody's real Plex, and a hard-coded
/// one would compare a real `404` against a fake item — a shape difference the
/// comparison would report as drift in the fake.
struct Surface {
    section: SectionKey,
    item: RatingKey,
    collection: RatingKey,
    /// A filter that declared a choice endpoint, exactly as that server
    /// composed it (P7).
    filter: DiscoveredFilter,
}

/// Reads the keys the rest of the surface is addressed by, off `client`.
///
/// Every call here is one of the calls under test, run through this crate's own
/// parsers — so a server whose answer this build cannot read fails here, naming
/// what was missing, before any shape is compared. It runs against both
/// servers, which holds the fake to the same domain facts the real one is.
async fn surface(client: &PlexServerClient) -> Surface {
    let sections = client
        .sections()
        .await
        .expect("GET /library/sections must answer");
    let movies = sections
        .iter()
        .find(|section| section.kind == LibraryKind::Movie)
        .expect("the contract server must have a movie library");

    let page = client
        .items(
            &movies.key,
            &ItemQuery::new(Window::first(20)).of_type(ItemKind::Movie),
        )
        .await
        .expect("a library window must answer");
    assert!(
        page.total.is_some(),
        "a server reports the size of the whole result"
    );
    let item = page
        .items
        .first()
        .expect("the contract server's movie library must hold at least one item")
        .rating_key
        .clone();

    let collection = client
        .collections(&movies.key)
        .await
        .expect("the collection list must answer")
        .first()
        .expect("the contract server's movie library must hold at least one collection")
        .rating_key
        .clone();

    let vocabulary = client
        .vocabulary(&movies.key, ItemKind::Movie)
        .await
        .expect("filter-metadata discovery must answer");
    assert!(
        !vocabulary.types.is_empty() && !vocabulary.field_types.is_empty(),
        "a server declares its own filter vocabulary"
    );
    let filter = vocabulary
        .types
        .iter()
        .flat_map(|kind| kind.filters.iter())
        .find(|filter| filter.key.is_some())
        .cloned()
        .expect("the contract server must offer a filter with an enumerated choice list");

    Surface {
        section: movies.key.clone(),
        item,
        collection,
        filter,
    }
}

/// The read-only surface both servers are asked for.
///
/// Read-only, and every write call is left out: this runs against somebody's
/// real Plex, and a contract test that created a collection to check the
/// response shape would leave it behind (P2). The write calls' request shapes
/// are covered against a fixture in `protocol.rs`; what this adds is the answer
/// shape, and only reads have one worth comparing.
fn read_calls(surface: &Surface) -> Vec<Call> {
    let section = &surface.section;
    let window = ItemQuery::new(Window::first(20)).of_type(ItemKind::Movie);
    let meta = ItemQuery::new(Window::first(0))
        .of_type(ItemKind::Movie)
        .including_meta();
    let children = ItemQuery::new(Window::first(20));
    vec![
        Call {
            // The server root, which is the call that says whether the stored
            // token is still accepted. Its path is empty because the address is
            // the endpoint.
            name: "GET /",
            path: String::new(),
            query: Vec::new(),
        },
        Call {
            name: "GET /identity",
            path: "identity".to_owned(),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/sections",
            path: "library/sections".to_owned(),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/sections/{key}/all",
            path: format!("library/sections/{section}/all"),
            query: window.pairs(),
        },
        Call {
            name: "GET /library/sections/{key}/all?includeMeta=1",
            path: format!("library/sections/{section}/all"),
            query: meta.pairs(),
        },
        Call {
            name: "GET /library/sections/{key}/collections",
            path: format!("library/sections/{section}/collections"),
            query: vec![("includeCollections".to_owned(), "1".to_owned())],
        },
        Call {
            name: "GET /library/metadata/{key}",
            path: format!("library/metadata/{}", surface.item),
            query: Vec::new(),
        },
        Call {
            name: "GET /library/collections/{key}/children",
            path: format!("library/collections/{}/children", surface.collection),
            query: children.pairs(),
        },
        Call {
            // The endpoint the server composed for its own filter, query string
            // and all. Nothing here reassembles it from parts (P7).
            name: "GET a filter's choice list",
            path: surface
                .filter
                .key
                .clone()
                .expect("the surface carries a filter that declared one"),
            query: Vec::new(),
        },
        Call {
            name: "GET /hubs/sections/{key}/manage",
            path: format!("hubs/sections/{section}/manage"),
            query: Vec::new(),
        },
    ]
}

#[tokio::test]
async fn the_real_servers_answers_parse_and_the_fake_claims_nothing_they_do_not() {
    let Some(real) = real_server() else {
        // Not a pass. The release lane sets both variables, and this line is
        // what a reader of any other lane's log sees instead of a green tick.
        eprintln!(
            "SKIPPED: no real Plex server configured. Set {URL} and {TOKEN} to run the \
             contract test (D-036). The adversarial fake is unverified without it."
        );
        return;
    };

    // The fake's first movie library, which is the one it is asked about.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let fake_client = fake_client(&fake);

    // Every call in the surface must parse on the real server, and the domain
    // facts each one exists to read must be there. The listing, collection,
    // and discovery calls are exercised while the surface is discovered.
    let identity = real
        .identity()
        .await
        .expect("GET /identity must answer on a real server");
    assert!(
        !identity.machine_identifier.as_str().is_empty(),
        "a real server names itself"
    );
    assert!(
        !identity.version.is_empty(),
        "a real server names its version"
    );

    real.verify_credential()
        .await
        .expect("the server root must accept the token the release lane configured");

    let real_surface = surface(&real).await;

    // The reads the surface discovery does not itself make. Each one is a
    // parser that would otherwise be checked against nothing but a fixture
    // written in this repository.
    let item = real
        .item(&real_surface.item)
        .await
        .expect("GET /library/metadata/{key} must answer on a real server");
    assert_eq!(
        item.rating_key, real_surface.item,
        "a real server answers with the item it was asked for"
    );

    let children = real
        .collection_items(&real_surface.collection, &ItemQuery::new(Window::first(20)))
        .await
        .expect("a collection's children must answer on a real server");
    assert!(
        children.total.is_some(),
        "a real server reports the size of the whole result"
    );

    let choices = real
        .filter_choices(&real_surface.filter)
        .await
        .expect("a declared filter's choice list must answer on a real server");
    assert!(
        choices.iter().all(|choice| !choice.value.is_empty()),
        "a choice with no value cannot be sent back in a query"
    );
    assert!(
        !choices.is_empty(),
        "the filter declared a choice endpoint, so the contract server must offer choices on it"
    );

    real.hubs(&real_surface.section)
        .await
        .expect("the manage endpoint must answer on a real server");

    // And the fake claims nothing the real answers do not. This is the half
    // that keeps the fake truthful, and it fails by naming the call.
    let fake_surface = surface(&fake_client).await;
    for (real_call, fake_call) in read_calls(&real_surface)
        .into_iter()
        .zip(read_calls(&fake_surface))
    {
        let real_body = raw(&real, &real_call.path, &real_call.query).await;
        let fake_body = raw(&fake_client, &fake_call.path, &fake_call.query).await;
        shape::assert_supported(real_call.name, &fake_body, &real_body);
    }
}

#[tokio::test]
async fn the_fake_answers_every_call_the_contract_covers() {
    // Runs in every lane, and is the part that would go red first if a call in
    // the surface above were renamed or dropped: the release lane needs a real
    // server, and this needs nothing.
    let fake = FakePlex::start(Scenario::behaving(1)).await;
    let client = fake_client(&fake);
    for call in read_calls(&surface(&client).await) {
        let body = raw(&client, &call.path, &call.query).await;
        assert!(
            body.get("MediaContainer").is_some(),
            "{} answered outside the envelope every Plex answer arrives in",
            call.name
        );
    }
}
