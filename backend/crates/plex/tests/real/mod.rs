// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

// The contract test is the only consumer, and it uses one part of this at a
// time.
#![allow(dead_code)]

//! Reaching a real Plex, reading the keys its surface is addressed by, and
//! keeping what it answered.

use std::path::PathBuf;

use afisharr_plex::{
    discovery::DiscoveredFilter,
    fake::FakePlex,
    identity::ClientIdentity,
    libraries::{ItemKind, ItemQuery, LibraryKind, RatingKey, SectionKey, Window},
    server::{PlexServerClient, ServerAddress, ServerError, ServerToken},
};
use afisharr_sources::outbound::{OutboundClient, Response};
use serde_json::Value;

/// Where the real server is, in the release lane.
pub const URL: &str = "AFISHARR_PLEX_CONTRACT_URL";

/// The token the release lane supplies for it.
pub const TOKEN: &str = "AFISHARR_PLEX_CONTRACT_TOKEN";

/// The title the write exercise gives what it creates.
///
/// Named so an operator who finds one on their own server knows what it is and
/// that it should not be there. Nothing is meant to survive the test (P2).
pub const SCRATCH: &str = "Afisharr contract test (safe to delete)";

/// What a real Plex answers that this build deliberately does not model.
///
/// The fake is not an emulator (PRD section 21.10.2), and the reverse shape
/// comparison would otherwise fail on every one of these. Each entry is a
/// decision somebody made, which is the point: an omission that is not on this
/// list is a gap the release lane reports rather than a silence.
///
/// It starts as the set of subtrees this build knows it does not read, and it
/// grows by evidence: the first release run against a real server names
/// whatever else that server sends, and each addition arrives with the reason
/// it is not modelled.
pub const ALLOWED: &[&str] = &[
    // Cast, crew, and the tag families Afisharr neither reads nor writes.
    ".MediaContainer.Metadata[].Role",
    ".MediaContainer.Metadata[].Director",
    ".MediaContainer.Metadata[].Writer",
    ".MediaContainer.Metadata[].Producer",
    ".MediaContainer.Metadata[].Country",
    ".MediaContainer.Metadata[].Similar",
    ".MediaContainer.Metadata[].Collection",
    ".MediaContainer.Metadata[].Rating",
    ".MediaContainer.Metadata[].Image",
    ".MediaContainer.Metadata[].UltraBlurColors",
    ".MediaContainer.Metadata[].Chapter",
    ".MediaContainer.Metadata[].Marker",
    ".MediaContainer.Metadata[].Extras",
    ".MediaContainer.Metadata[].Mood",
    // Editorial and rating metadata. Overlays render state the engine knows,
    // and none of this is state the engine knows.
    ".MediaContainer.Metadata[].summary",
    ".MediaContainer.Metadata[].tagline",
    ".MediaContainer.Metadata[].studio",
    ".MediaContainer.Metadata[].contentRating",
    ".MediaContainer.Metadata[].rating",
    ".MediaContainer.Metadata[].audienceRating",
    ".MediaContainer.Metadata[].ratingImage",
    ".MediaContainer.Metadata[].audienceRatingImage",
    ".MediaContainer.Metadata[].userRating",
    ".MediaContainer.Metadata[].slug",
    ".MediaContainer.Metadata[].duration",
    ".MediaContainer.Metadata[].chapterSource",
    ".MediaContainer.Metadata[].primaryExtraKey",
    ".MediaContainer.Metadata[].hasPremiumExtras",
    ".MediaContainer.Metadata[].hasPremiumPrimaryExtra",
    // Watch state. Afisharr never reads it and must never write it.
    ".MediaContainer.Metadata[].viewCount",
    ".MediaContainer.Metadata[].viewOffset",
    ".MediaContainer.Metadata[].lastViewedAt",
    ".MediaContainer.Metadata[].lastRatedAt",
    ".MediaContainer.Metadata[].skipCount",
    // Artwork other than the poster. The poster is the only image this build
    // captures, renders, and restores.
    ".MediaContainer.Metadata[].art",
    ".MediaContainer.Metadata[].banner",
    ".MediaContainer.Metadata[].theme",
    ".MediaContainer.Metadata[].artBlurHash",
    ".MediaContainer.Metadata[].thumbBlurHash",
    ".MediaContainer.art",
    ".MediaContainer.banner",
    ".MediaContainer.thumb",
    ".MediaContainer.theme",
    ".MediaContainer.nocache",
    ".MediaContainer.sortAsc",
    ".MediaContainer.viewMode",
    ".MediaContainer.viewGroup",
    ".MediaContainer.title2",
    ".MediaContainer.librarySectionKey",
    ".MediaContainer.augmentationKey",
    ".MediaContainer.mixedParents",
    // Section attributes about presentation and sync rather than identity.
    ".MediaContainer.Directory[].art",
    ".MediaContainer.Directory[].composite",
    ".MediaContainer.Directory[].thumb",
    ".MediaContainer.Directory[].hidden",
    ".MediaContainer.Directory[].content",
    ".MediaContainer.Directory[].contentChangedAt",
    ".MediaContainer.Directory[].directory",
    ".MediaContainer.Directory[].enableAutoPhotoTags",
    ".MediaContainer.Directory[].secondary",
    ".MediaContainer.Directory[].prompt",
    ".MediaContainer.Directory[].search",
    // Transcode and streaming facts. Overlays badge the file, never the
    // session.
    ".MediaContainer.Metadata[].Media[].optimizedForStreaming",
    ".MediaContainer.Metadata[].Media[].has64bitOffsets",
    ".MediaContainer.Metadata[].Media[].hasVoiceActivity",
    ".MediaContainer.Metadata[].Media[].audioProfile",
    ".MediaContainer.Metadata[].Media[].proxyType",
    ".MediaContainer.Metadata[].Media[].target",
    ".MediaContainer.Metadata[].Media[].title",
    ".MediaContainer.Metadata[].Media[].Part[].indexes",
    ".MediaContainer.Metadata[].Media[].Part[].packetLength",
    ".MediaContainer.Metadata[].Media[].Part[].requiredBandwidths",
    ".MediaContainer.Metadata[].Media[].Part[].optimizedForStreaming",
    ".MediaContainer.Metadata[].Media[].Part[].has64bitOffsets",
    ".MediaContainer.Metadata[].Media[].Part[].hasThumbnail",
    ".MediaContainer.Metadata[].Media[].Part[].audioProfile",
    ".MediaContainer.Metadata[].Media[].Part[].videoProfile",
    ".MediaContainer.Metadata[].Media[].Part[].duration",
    ".MediaContainer.Metadata[].Media[].Part[].deepAnalysisVersion",
    ".MediaContainer.Metadata[].Media[].Part[].Stream[].requiredBandwidths",
    // The server root, which this build reads four fields of.
    ".MediaContainer.allowCameraUpload",
    ".MediaContainer.allowChannelAccess",
    ".MediaContainer.allowMediaDeletion",
    ".MediaContainer.allowSharing",
    ".MediaContainer.allowTuners",
    ".MediaContainer.backgroundProcessing",
    ".MediaContainer.certificate",
    ".MediaContainer.companionProxy",
    ".MediaContainer.countryCode",
    ".MediaContainer.diagnostics",
    ".MediaContainer.eventStream",
    ".MediaContainer.hubSearch",
    ".MediaContainer.itemClusters",
    ".MediaContainer.livetv",
    ".MediaContainer.machineIdentifierHash",
    ".MediaContainer.mediaProviders",
    ".MediaContainer.multiuser",
    ".MediaContainer.musicAnalysis",
    ".MediaContainer.myPlexMappingState",
    ".MediaContainer.myPlexSigninState",
    ".MediaContainer.myPlexSubscription",
    ".MediaContainer.myPlexUsername",
    ".MediaContainer.offlineTranscode",
    ".MediaContainer.ownerFeatures",
    ".MediaContainer.photoAutoTag",
    ".MediaContainer.platformVersion",
    ".MediaContainer.pluginHost",
    ".MediaContainer.pushNotifications",
    ".MediaContainer.readOnlyLibraries",
    ".MediaContainer.streamingBrainABRVersion",
    ".MediaContainer.streamingBrainVersion",
    ".MediaContainer.sync",
    ".MediaContainer.transcoder",
    ".MediaContainer.updater",
    ".MediaContainer.voiceSearch",
];

/// A client against the real server, or `None` when the lane configured none.
pub fn server() -> Option<PlexServerClient> {
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
pub fn fake_client(fake: &FakePlex) -> PlexServerClient {
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
pub async fn raw(client: &PlexServerClient, path: &str, query: &[(String, String)]) -> Value {
    try_raw(client, path, query)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"))
}

/// The same read, reported rather than panicked.
///
/// For the callers that have something to clean up: a panic between creating a
/// scratch collection on somebody's real Plex and deleting it again leaves the
/// collection behind, which is the one thing these tests may not do (P2).
pub async fn try_raw(
    client: &PlexServerClient,
    path: &str,
    query: &[(String, String)],
) -> Result<Value, String> {
    let url = client
        .address()
        .endpoint(path, query)
        .map_err(|error| format!("{path} is not an endpoint this build can compose: {error}"))?;
    let response: Response = client
        .raw_get(&url)
        .await
        .map_err(|error: ServerError| format!("{path} did not answer: {error}"))?;
    serde_json::from_str(&response.body)
        .map_err(|error| format!("{path} answered something that is not JSON: {error}"))
}

/// The keys one server's surface is addressed by.
///
/// Discovered from that server rather than written down. A rating key that
/// exists on the fake means nothing on somebody's real Plex, and a hard-coded
/// one would compare a real `404` against a fake item.
pub struct Surface {
    pub section: SectionKey,
    pub item: RatingKey,
    pub collection: RatingKey,
    /// A filter that declared a choice endpoint, exactly as that server
    /// composed it (P7).
    pub filter: DiscoveredFilter,
}

/// Reads the keys the rest of the surface is addressed by, off `client`.
///
/// Every call here is one of the calls under test, run through this crate's own
/// parsers, so a server whose answer this build cannot read fails here, naming
/// what was missing, before any shape is compared.
pub async fn surface(client: &PlexServerClient) -> Surface {
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

/// Keeps what the real server answered, under a name a diff can be read from.
///
/// A capture is what lets the fake be corrected without a server in hand, and
/// what makes a later drift a diff rather than an argument. Written on every
/// green run; committing what it wrote is a human act.
pub fn capture(call: &str, body: &Value) {
    let slug: String = call
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real");
    std::fs::create_dir_all(&directory).expect("the capture directory must be writable");
    let pretty = serde_json::to_string_pretty(body).expect("a captured answer serialises");
    std::fs::write(
        directory.join(format!("{}.json", slug.trim_matches('-'))),
        pretty,
    )
    .expect("a captured answer must be writable");
}
