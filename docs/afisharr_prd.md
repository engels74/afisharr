# Afisharr — Product Requirements

A Plex collections, posters, and overlay manager: a single binary with a GUI-first, self-healing
design. Rust backend (stable 1.97.1, edition 2024), SvelteKit static SPA embedded in the binary.

Afisharr is a standalone product. It shares no code, schema, or compatibility surface with any other
tool, offers no data migration from any other tool, and is not described by comparison to one.
Capabilities are stated in the first person throughout.

**Licence:** AGPL-3.0-or-later (D-028).

---

## 0. How to read this document

### 0.1 What this document is

This is the complete product requirements document for Afisharr. It carries the product scope, the
functional requirements for every subsystem, the data model, the invariants that constitute the test
plan, the non-functional requirements, every decision of record with its reasoning, and the coding
guidelines the implementation must satisfy.

It has one companion, and only one: the implementation plan, which turns these requirements into
phases, tasks, and subtasks. Together the two documents are the complete reference set for what to
build and in what order.

**Two rule files carry the stack-level coding guidelines, and both are normative.** They live in the
repository, not in this document:

| File | Covers |
| --- | --- |
| `.augment/rules/frontend-dev-pro.md` | The frontend stack: Bun, Svelte 5 (runes), SvelteKit 2, UnoCSS `presetWind4`, shadcn-svelte |
| `.augment/rules/backend-rust-dev-pro.md` | The backend stack: Rust 1.97.1, edition 2024, tokio, axum, SQLx, serde |

**Read the rule file for the surface you are about to touch before you write any code for that
surface.** This binds every author, human or agent, on every change. It is not background reading and
it is not an appendix: the rule files state the one current idiomatic pattern for each construct on
each stack, version-anchored, and code that contradicts them is wrong even when it compiles and the
tests pass. §24.1 states the rule and its authority; D-048 records why.

**One further obligation binds interface work, and only interface work: an agent that builds or
reshapes a screen loads its `frontend-design` skill first.** The rule file says how to write correct
Svelte; it says nothing about whether the result looks like anything. An agent left to its own
defaults produces the same centred card on a grey page every time, and fifteen pages built that way
are fifteen pages nobody chose. The skill is the counterweight, and it is read before the markup is
written rather than consulted after a reviewer calls the page bland. §24.3.5 states the rule and the
scope; D-051 records why. It does not join the normative set below: it is a working obligation on the
author, not a source of requirements — where the skill and this document disagree, this document
wins.

Those four files — this document, the implementation plan, and the two rule files — are the complete
normative set. Nothing else is normative.

### 0.2 Authority

Within this document, the more specific statement wins over the more general one. **A conflict is a
bug to report, not an ambiguity to resolve by choosing.**

- *Decisions of record* (§22) is the frozen record of what is being built. Reopening an entry is a
  dated change request in §22.4, never a silent edit elsewhere.
- *Invariants* (§20) is authoritative for the test plan. Every test obligation in the product lives
  there. A functional section states *why* a property matters; the invariant states it in a form a
  build can fail on.
- The functional sections (§11 through §19, §21) are authoritative for how each subsystem works.
- §1 through §10 are the product-level summary. Where a summary and a functional section disagree,
  the functional section wins.
- *Coding guidelines* (§24) is normative for *how* code is written and sits outside this hierarchy.
  One exception is recorded in §24.4: the SPA has no server runtime, so the server-side half of the
  frontend guidelines is structurally inapplicable.
- The two rule files in `.augment/rules/` are normative for *how* code is written on each stack, and
  they also sit outside this hierarchy. Read the relevant one before writing code (§0.1, §24.1).
  §24 is the project layer over them: it selects, tightens, and adds to them for Afisharr. Where a
  rule file and §24 disagree, §24 wins, because §24 knows this project's architecture and the rule
  file does not. Where §24 is silent, the rule file binds on its own.

### 0.3 Conventions

**Voice.** No shipped Afisharr document describes the product by comparison to another tool.
Capabilities are stated in the first person.

**Identifiers, not section numbers.** Decisions (`D-nnn`), open questions (`Q-nnn`), change requests
(`CR-n`), and invariants (`I-GROUP-n`) carry stable identifiers that survive any renumbering. Cite
those rather than a section wherever one exists.

**Sizes and dates.** This document carries no dates for unbuilt work and no capacity figures. Both
would be fabrications against work that has not started (D-039).

**Link integrity.** A citation of the form `§N` points at a section of this document; a citation of
the form `D-nnn`, `Q-nnn`, `CR-n`, or `I-GROUP-n` points at an identifier defined in it. Both must
resolve. Check a citation you add or move, because a dangling one still reads as if it resolves.

**Verify by executing.** The data model's claims about what the schema enforces were checked by
running the DDL against SQLite 3.45, and the check caught real errors. Do the same for anything
added.

### 0.4 Contents

| Section | Contains |
| --- | --- |
| §1–§3 | Product thesis, the full capability scope by tier, non-goals, architecture |
| §4–§10 | Users, journeys, information architecture, page inventory, state policy, interface requirements |
| §11–§12 | The collection pipeline and the definition layer |
| §13–§14 | The four registries and the external source policy |
| §15–§16 | Placement and ordering; posters and overlays |
| §17–§18 | The lifecycle state machine and acquisition policy |
| §19 | The data model: 68 tables, conventions, migration, concurrency, retention |
| §20 | Seven recurring failure patterns and all 97 invariants |
| §21 | Scale, budgets, security, platforms, backup, upgrade, privacy, licence, test strategy |
| §22–§23 | Every decision of record with its reasoning; open questions |
| §24 | The normative coding guidelines: the two stack rule files (§24.1), the per-surface rules, and the modular-structure requirement (§24.6) |

### 0.5 What is still open

Two questions are empirical, and both invalidate work if answered late. They are scheduled in the
implementation plan as explicit spikes rather than folded into implementation tasks: the real
precision budget before exhaustion (Q-014), and whether the home screen is one global sequence or
per-library sequences merged at render (Q-015). The second determines whether ordering is one
planning problem or several, and it also blocks the home-screen board design (Q-013). The full list
of open questions is §23.

---
## 1. Product overview and thesis

Afisharr is a Plex collections, posters, and overlay manager. It ships as a single binary with a
GUI-first, self-healing design: a Rust backend (stable toolchain 1.97.1, edition 2024) serving an
embedded SvelteKit static SPA.

Afisharr is a standalone product. It shares no code, schema, or compatibility surface with any other
tool, offers no data migration from any other tool, and is not described anywhere in this document by
comparison to one. Every capability below is stated in the first person.

The product thesis rests on five commitments:

- **Collection automation is the product.** Sources resolve to items, items reconcile into Plex
  collections, collections take their place on the home screen. Everything else is downstream.
- **The lifecycle system is the differentiator.** Tracking a title from announcement through release
  to availability — materializing placeholders, driving acquisition, surfacing status on posters — is
  the capability that makes the product worth running rather than a nicer way to make lists.
- **Overlays render state the engine already knows.** Media streams, lifecycle status, ratings.
  Overlays are a projection of the core, not a second product.
- **Breadth is a data problem, not an engine problem.** I build capability *classes* — a generic
  source interface, a generic condition language, a generic element model — and close content gaps
  with importable, shareable packs rather than with code.
- **Everything the GUI edits is a versioned, exportable definition document.** GUI-first, never
  GUI-trapped.

**Licence.** Afisharr is licensed AGPL-3.0-or-later, recorded as D-028. Because Afisharr is a web
application, AGPL section 13 obliges a modified instance offered to others over a network to make its
source available to them; the interface carries a source link for exactly that reason.

**Status.** This overview reconciles the product against its frozen capability scope, decided
2026-08-08. The scope covers roughly 120 distinct user-visible capabilities, each carrying an
explicit tier decision; the full tier tables follow in *Scope* below.

## 2. Scope

The capability scope below is frozen at capability granularity. Reopening any single decision is a
dated change request against this ledger, never a silent edit to the tables. Two tier codes recur
throughout: **T0** means present at first shippable release; **T1** means committed, after first
release; **T2** marks the content/pack tier, committed only as a pack or a later add; **CUT** means
not building, recorded with a reason so it stays cut. Confirmed cuts and the reasoning behind them are
carried in full in *Consequences of the frozen scope*, §2.6, rather than repeated inline in the tier
tables below.

### 2.1 Tier 0 — first shippable release

**Principle: no capability regression.** Anything a user could reasonably expect from a tool in this
category is present at launch or explicitly listed as a non-goal in §2.4.

#### Identity, auth, users

| Capability | What it does | Notes |
| --- | --- | --- |
| Plex OAuth login | Sign in with a Plex account; token stored | |
| Local login | Username/password local accounts | |
| Sessions | Cookie sessions, persisted | |
| Plex token refresh job | Scheduled re-auth so tokens don't die silently | |
| Permission model | Bitfield permissions per user | Admin-only surface at T0; the schema is modelled as a principal set from day one so per-user targeting is a widening rather than a migration |
| API key auth | Static key for external/API callers | |
| CSRF protection toggle | On/off switch for CSRF checks | Always on; no toggle exposed |
| Trust-proxy toggle | Honour `X-Forwarded-*` behind a reverse proxy | |

#### Libraries, metadata, caching

| Capability | What it does | Notes |
| --- | --- | --- |
| Library discovery + enable/disable | Lists Plex libraries; pick which are managed | |
| Library item cache | Local mirror of library contents for fast diffing | |
| Sync-scoped cache | Per-run memoization | Internal |
| Metadata change tracking | Detects item metadata drift | |
| Canonical ID matching | TMDB/TVDB/IMDb ↔ Plex GUID resolution | |
| Anime ID mapping | Cross-maps AniList/MyAnimeList ↔ TMDB/TVDB | Required for anime sources |
| Season model | Season-level records for TV | |
| TMDB poster file cache | 7-day disk cache of TMDB artwork | |
| TMDB language setting | Locale for TMDB metadata fetches | |
| External API response cache | Shared HTTP cache layer | |

#### Sources

Each row is a distinct source builder; subtypes are the real unit of work — one "TMDB source" is
eight.

| Source | Subtypes / modes | Notes |
| --- | --- | --- |
| TMDB charts | popular, top_rated, trending | |
| TMDB franchise | Collection/franchise expansion | |
| TMDB custom list | List by URL | |
| TMDB random | Random pick from a pool | |
| TMDB Discover (advanced) | Nested filter groups, AND/OR operators, sort order | |
| TMDB watch providers | Collections by streaming service + region | |
| TMDB person collections | Auto-collections per actor / director, with a min-items threshold and an optional separator collection | Person auto-collections are Tier 0 and a different capability from the TMDB People browse source |
| Trakt charts | trending, popular, watched (+ time period) | |
| Trakt custom list | List by URL, including official lists | |
| Trakt recommendations | Personalized recommendations | |
| Trakt OAuth | User auth for personal lists | |
| IMDb charts | Top / most-popular charts | Via documented JSON/GraphQL endpoints — an API-tier source, not scraped; capability flags follow the endpoint rung actually answering |
| IMDb custom list | List by URL | |
| Letterboxd list | List by URL, plus random | |
| MDBList | List by URL | |
| AniList | Charts + custom list URL | |
| MyAnimeList | Charts + list | |
| Networks (FlixPatrol) | Per-country network/platform charts | |
| Originals | Platform-originals groupings | |
| Overseerr | Requests: per-user and global | |
| Tautulli | most_popular / most_watched × plays / duration, configurable window in days, minimum plays | |
| Radarr tag | Items carrying a Radarr tag, per instance | |
| Sonarr tag | Items carrying a Sonarr tag, per instance | |
| Plex library | Library-derived: recently added, recently released, recently released episodes | |
| Lifecycle Coming Soon | Unreleased/upcoming feed, monitored + unmonitored | |
| Multi-source composition | N sources combined into one collection, per-source priority | |
| Hub replacement | Not a source type. A shadow of a Plex default hub (recently added / released / released-episodes) that excludes placeholder items, because placeholders would otherwise pollute Plex's native rows. Lives in the hub-management subsystem: hide Plex's native row, substitute a clean one | Redesigned from an earlier "filtered hub" source concept |
| Ratings as filter/overlay inputs | Critic + audience score lookups, and separate rating lookups, feeding filters and overlays | Not sources in their own right |

#### Merge, filter, order

| Capability | What it does | Notes |
| --- | --- | --- |
| Combine modes | `interleaved`, `list_order`, `randomised`, `cycle_lists` | All four map to a merge strategy plus an ordering rule |
| Per-source priority | Ordering weight per source in a multi-source collection | |
| Item cap | Maximum items per collection | |
| Position cap | Only consider list positions 1 through X from a source | |
| Sort order | Collection item ordering options | |
| Genre / country / language filters | Include **or** exclude mode per axis | |
| Keyword filters | TMDB keyword include/exclude | |
| Minimum year | Release-year floor | |
| Minimum IMDb rating | Rating floor | |
| Minimum RT critic / audience | Two independent floors | |
| Global exclusions | Never-include list by TMDB/TVDB ID | |
| Mutual exclusion | Item in collection A is excluded from collection B | |
| Unwatched-only smart collections | Server-side Plex smart collection filtered to unwatched, with its own sort; compiled to Plex where every predicate is server-native | |
| Time restrictions | Seasonal date ranges (DD–MM) plus a weekly day mask; inactive behaviour is configurable as hide or remove | |

#### Plex presentation, hubs, ordering

The largest source of complexity, and historically the largest source of bugs — decided deliberately.

| Capability | What it does | Notes |
| --- | --- | --- |
| Three-axis visibility | Owner home / users home / library recommended, set independently | |
| Home ordering | Explicit position on the Plex home screen | |
| Library ordering | Position within the library tab | |
| Promoted vs A-Z section | Two library zones, driven by sort-title prefix characters | |
| Sort-title prefix management | Writes and strips prefix characters to force ordering | One implementation, reconciled |
| Randomize home order | Shuffles flagged items among themselves on a schedule | |
| Built-in Plex hub management | Position/visibility of Plex's own hubs (Recently Added, Continue Watching, etc.) | |
| Foreign collection adoption | Manage position and art of collections Afisharr did not create | |
| Structural multi-library targeting | A definition targets a set of libraries and fans out; there is no per-library configuration duplicate to keep in sync | Replaces an earlier "linked collections" concept — auto-grouping configs that share a base hub identifier across libraries. That concept is deleted outright: multi-library targeting is structural, not a feature requiring grouping heuristics, sticky link identifiers, or unlink/relink state |
| Hide individual items | Show the collection instead of its children | |
| Multi-collection configs | One config producing many Plex collections (per-user, per-franchise) | |
| Self-healing rating keys | Recover when Plex keys change or collections are deleted | |

#### Collection extras (art, text, audio)

| Capability | What it does | Notes |
| --- | --- | --- |
| Custom poster | Upload/choose art, optionally per-library | |
| Auto-generated poster | Render a poster from a template at sync time | |
| Franchise poster passthrough | Use the provider's franchise art instead of generating | |
| Custom wallpaper (art/backdrop) | Sets the collection background image | |
| Custom summary | Overrides the collection description | |
| Local asset folders | Scans a directory tree for posters/art by name | |
| Font asset management | Upload/manage fonts for rendering | |
| Icon asset management | Icon library for overlay mappings | |
| File upload endpoint | Generic asset upload | |
| Server filesystem browser | Pick paths on the host from the GUI | Jailed to configured root paths |

#### Poster generation

| Capability | What it does | Notes |
| --- | --- | --- |
| Poster template model | Layered design: background, text, tiles, 1000×1500 canvas | |
| Poster editor GUI | Visual editor with a layer panel and background controls | Core selling point; full editor at T0, not a stub |
| Saved posters | Persist generated output | Content-addressed |
| Preview asset packs | Sample posters/persons for editor preview | |
| Default template seed | Ships one usable template out of the box | Shipped as a pack |

#### Overlays

| Capability | What it does | Notes |
| --- | --- | --- |
| Overlay template model | Layered element canvas with conditions | |
| Overlay renderer | Composites elements onto item posters | |
| Overlay context builder | Assembles the state an overlay renders from | |
| Per-library overlay config | Which templates apply to which library | |
| Value→icon mappings | Lookup tables (codec → logo), default plus user layers | |
| Preset templates | Shipped starter overlays | Shipped as packs |
| Base poster capture | Downloads and stores the pristine original before overlaying | |
| Poster reset | Restores originals | |
| Base poster source choice | Prefer TMDB / Plex / local as the pristine base | |
| Overlay test endpoint | Render one item on demand for debugging | Preview endpoint |
| Apply overlays during sync | Immediately overlay newly-added items | |

#### Lifecycle: Coming Soon, placeholders

| Capability | What it does | Notes |
| --- | --- | --- |
| Coming Soon item tracking | Tracks unreleased items and their dates | |
| Placeholder creation | Writes stub media files so Plex shows unreleased titles | |
| Placeholder discovery | Finds existing placeholders on disk/in Plex | |
| Placeholder cleanup | Removes placeholders when state changes | |
| Placeholder title repair | Fixes Plex mis-matching placeholder files | |
| Per-library placeholder roots | Movie/TV placeholder paths per library | |
| Look-ahead window | How far ahead to include upcoming releases | |
| Released-retention window | Keeps released items overlaid for N days, then restores | |
| Include-all-released toggle | Ignores the date cutoff for released items | |
| Independent placeholder filters | Year/rating/genre/country/language/keyword filters, separate from grab filters | |
| Monitored-source filtering | Restricts Coming Soon to specific *arr instances and tags, include/exclude | |
| Missing-item records | Tracks what a collection wants but lacks | |
| Subject and season tracking | Subjects track a whole title by default; a show can opt into one subject per season | Recorded as D-025. Season *overlays* remain Tier 1.5, so season tracking serves placeholders and acquisition at launch only |

#### Acquisition (requests and grabbing)

| Capability | What it does | Notes |
| --- | --- | --- |
| Routing: requests vs. direct | Creates Overseerr requests, or adds straight to an *arr instance | |
| Auto-request service | Creates requests for missing items | |
| Direct download service | Adds to Radarr/Sonarr directly | |
| Search-on-add / auto-approve | Per-media-type toggles | |
| Season limits | Max seasons to request; caps each show to its first X seasons | |
| Season grab order | first / latest / airing | |
| *arr overrides | Server, quality profile, root folder, tags, monitor mode, search flag, season folder | These six overrides are in use at T0; the full grab override matrix is Tier 1 |
| Multi-instance *arr | Several Radarr/Sonarr servers, selected per collection | |

#### Scheduling and jobs

| Capability | What it does | Notes |
| --- | --- | --- |
| Job registry | Named jobs with cron schedules | |
| Per-collection schedules | Each collection has its own cadence | Includes jitter |
| Job settings GUI | View/edit schedules, trigger runs | |
| Persisted job state | Last run, result, next run | |
| Collection cleanup | Removes orphaned/abandoned collections | |

#### Observability and operations

| Capability | What it does | Notes |
| --- | --- | --- |
| Per-collection sync status | Last-synced time, needs-sync flag, last error plus timestamp | |
| Global sync status | Master error plus timestamp | |
| Logs page | Reads and filters the app log in-GUI | |
| Dashboard | Overview of collections/activity | |
| Setup wizard | First-run guided configuration, gated by a console bootstrap token and leased to one browser | Claim, resume, and recovery built new (D-045, D-046) |
| About page | Version, build info | |
| App-data warning | Warns on a misconfigured persistent volume | |
| In-app search | Searches library/providers | Editor-scoped |
| Title fetch helper | Resolves a URL to a title for the GUI | |
| Doctor / self-check page | Configuration sanity, Plex connectivity, source reachability, orphaned overlay detection, base-poster audit, ambiguous-match resolution, and the destructive operator actions that exist nowhere else | No prior equivalent audited; built new |
| Docker image | Container distribution | |
| OpenAPI spec | Machine-readable API description | Generated via utoipa, not hand-written |
| Editor preview endpoints | "What would this collection contain right now?" before a save | Distinct from in-app content browsing, which is a non-goal (§2.4); this preview capability is retained at T0 |

#### Internationalization

| Capability | What it does | Notes |
| --- | --- | --- |
| UI translation framework | Message catalog and locale switch | Framework ships from day one; retrofitting i18n later is far more expensive than building with it, so message extraction and a "no hardcoded strings" lint rule belong in the first milestone even though only English ships |
| English locale | Shipped at launch | |

### 2.2 Tier 1 — after first release

#### Identity, auth, users

| Capability | What it does | Notes |
| --- | --- | --- |
| Managed-user list GUI | Browse Plex users, manage access | |
| Service user | A dedicated Plex account used for writes | Schema accommodates it from T0 |
| Plex user labels for access filtering | Applies labels to Plex users to scope collection visibility | |
| Per-user collection targeting | Managed-user GUI, service user, and label-based scoping combined | |

#### Sources

| Capability | What it does | Notes |
| --- | --- | --- |
| TVDB client | Show/movie lookup | |
| GitHub API | Release/update check | Opt-in |
| TMDB Company and Keyword sources | Additional TMDB source types | |
| TMDB People browse source | Browsing people, distinct from person auto-collections (Tier 0) | |
| Remaining TMDB charts | Chart types beyond popular/top_rated/trending | |
| Trakt user lists and box office | Additional Trakt source types | |

#### Merge, filter, order

| Capability | What it does | Notes |
| --- | --- | --- |
| Separator collections | Inserts a visual divider collection in the library A-Z | |

#### Plex presentation, hubs, ordering

| Capability | What it does | Notes |
| --- | --- | --- |
| Plex Collectionless, Pilots, Watchlist | Additional Plex library modes | |

#### Collection extras (art, text, audio)

| Capability | What it does | Notes |
| --- | --- | --- |
| Theme music | Uploads/sets collection theme audio | |

#### Poster generation

| Capability | What it does | Notes |
| --- | --- | --- |
| Provider brand palettes | Per-source colour schemes auto-applied to generated posters; user-overridable | Trivial once the poster editor exists |

#### Acquisition (requests and grabbing)

| Capability | What it does | Notes |
| --- | --- | --- |
| Existing-item tagging | Tags items already in the library | |
| *arr "All" builders | Builders over an *arr instance's full library state | |
| Full grab override matrix | The complete override set, beyond the six overrides in use at T0 | |

#### Scheduling and jobs, observability and operations

| Capability | What it does | Notes |
| --- | --- | --- |
| Plex webhook receiver | Reacts to Plex events | Route reserved at T0 |
| Update checking | Notifies of new releases | Opt-in, via the GitHub API |

#### Internationalization

| Capability | What it does | Notes |
| --- | --- | --- |
| Community locales | Locales beyond English: da, de, es, fr, hu, it, ja, nl, pt-BR, ru, sv, uk, zh-Hans | Fourteen locales existed in the audited prior art; framework ships at T0, these follow as community-maintained additions |

Also Tier 1: the dynamic "group library by attribute" collection type, and the visual smart-filter
builder generated from the server-discovered field layer. Playlists sit at Tier 1.5 specifically: the
engine supports ordered, per-user-owned item lists from day one, and the UI plus Plex playlist
integration follow afterward. Season-level *overlays* are likewise Tier 1.5, distinct from the
Tier 0 season tracking described above.

### 2.3 Tier 2 — content packs

Tier 2 is the content-pack tier: capability classes exist in the engine at Tier 0/1, and Tier 2 adds
first-party or community data packs on top rather than new code.

| Pack family | Contents |
| --- | --- |
| Media-info overlay packs | Resolution, HDR and Dolby Vision by profile, audio codec, languages, aspect ratio, runtime, editions, versions |
| Content-rating packs | Content rating badges and related overlay elements |
| Network and studio packs | Network and studio branding overlays |
| Ribbon and award packs | As curated-data quality allows |
| Seasonal collection packs | Seasonal/holiday collection definitions |

### 2.4 Non-goals

Each of these is a decision with a reason, not an omission.

| Non-goal | Reason |
| --- | --- |
| Music libraries | Different metadata model, different artwork conventions, different user |
| Preroll management | A different problem domain with adequate dedicated tools |
| Third-party configuration importers | Foreign schemas are moving targets; an importer rots silently into producing bad imports, which is worse than having none. Onboarding is served by the setup wizard and first-party packs. If demand appears after 1.0, an importer may exist only as a clearly labelled, best-effort, snapshot-of-a-version community tool — never a core compatibility promise |
| Headless browsers for anti-bot solving | Preserves the single-binary story; the external source policy describes what replaces it |
| Long-tail scraped sources | Each is a permanent maintenance liability against a site that never agreed to be read by Afisharr |
| Watchlist synchronization | A second product sharing acquisition plumbing; unrelated to collections, posters, or overlays |
| In-app content browsing | Afisharr is a management console. Browsing happens in Plex. The collection editor's preview — "what would this collection contain right now?" — is a different thing and is in scope |
| Trailer acquisition | Placeholders ship with a static placeholder video instead |

Additional capabilities considered and cut during scope review — separately from these product-level
non-goals — are catalogued with their reasons in §2.6 below.

### 2.5 Reversibility is a commitment

Reversibility is a commitment, not a non-goal. Every change Afisharr makes to a Plex library —
replaced artwork, modified sort titles, applied labels, created collections, materialized
placeholders, rearranged hubs — is reversible by a first-class teardown operation that restores the
originals byte-exactly and reports anything it could not restore. Decided 2026-08-08 as D-022; tested
by I-REV-4.

### 2.6 Consequences of the frozen scope

#### Conflicts found, and how each was resolved

Eight conflicts between the frozen scope and earlier design drafts were raised on 2026-08-08. Each is
recorded with its resolution, because a conflict that is merely fixed teaches nothing, and a reader
who meets the same tension later needs to know it was already argued.

| # | Conflict raised | Resolution |
| --- | --- | --- |
| 1 | Four already-shipping capabilities had been deferred to Tier 1 in an earlier scope-tier draft: TMDB advanced Discover with nested filter groups, watch providers, person auto-collections, and unwatched-only smart collections | **All four are Tier 0** (recorded in §2.2.15 below, resolution 1). No capability regression, ever. The tier summary was corrected |
| 2 | Theme music and local assets sat in Tier 0 in an earlier draft but in no milestone plan | **Theme music is Tier 1** (resolution 7, below). Local assets stay Tier 0. The earlier implementation plan is superseded |
| 3 | Hub management, collection adoption, and linked collections appeared in no design document, yet were roughly a third of the audited prior art's Plex-write complexity | A dedicated placement-and-ordering design was written to cover hub management and foreign-collection adoption. Linked collections were deleted as a concept — multi-library targeting makes them structural (resolution 4, below; see *Plex presentation, hubs, ordering*, §2.1) |
| 4 | i18n appeared in no design document despite fourteen locales existing in the audited prior art | Recorded here: framework at launch, English only shipped (§2.1, §2.2 Internationalization). Interface obligations are tracked as I-UX-7 and in the interface and onboarding design |
| 5 | An early architecture draft listed a YAML importer that the non-goals list and the definition-layer description had already ruled out | Removed from the architecture description. Third-party configuration import stays a non-goal, recorded as D-002 |
| 6 | An early draft made IMDb charts non-launch-blocking while also listing them as a Tier 0 source elsewhere | **IMDb charts are Tier 0** (§2.1 Sources). The conflicting line no longer exists |
| 7 | An early draft claimed "no open items" while the capability audit raised twenty | Resolved by the capability audit itself: open items are tracked as dated decisions, and what were architectural decisions embedded in the spec now carry stable D-nnn identifiers |
| 8 | With a static-adapter SPA, the frontend has no server runtime, so the server-side half of the frontend coding guidelines is structurally inapplicable | Stated explicitly in *Architecture*, §3, under "Frontend boundary," so it is not followed by habit |

A ninth was raised on 2026-08-09, after the freeze, and is recorded separately so the freeze date
stays legible:

| # | Conflict raised | Resolution |
| --- | --- | --- |
| 9 | The capability ledger recorded IMDb charts as Tier 0 "via documented JSON/GraphQL endpoints," while an external-source-policy draft omitted IMDb from the API-first list and an engine source-registry draft filed `imdb.chart` and `imdb.list` under the scraped tier | **IMDb is an API-tier source** (CR-3, D-040). The capability ledger's decision took precedence over the other two drafts, which were amended; capability flags became per-rung — not per-source — so a fallback path can never inherit a flag that was only true of the rung that answered |

#### Scope decisions resolved

Resolved 2026-08-08, round 1:

| # | Decision | Outcome |
| --- | --- | --- |
| 1 | Four already-shipping features in Tier 0? | **Yes — no capability regression, ever.** Discover filter groups, watch providers, person auto-collections, unwatched smart collections are all Tier 0 |
| 2 | Manage built-in Plex hubs? | **Yes.** Afisharr owns the home screen |
| 3 | Adopt foreign collections? | **Yes** |
| 4 | Keep linked collections? | **Concept deleted.** Multi-library targeting makes it structural |
| 5 | Poster editor in Tier 0? | **Yes — full editor.** Core selling point |
| 17 | What is the "filtered hub" concept? | **Answered from code.** Becomes hub replacement in the lifecycle/hub subsystem (§2.1 Sources) |

Resolved 2026-08-08, round 2:

| # | Decision | Outcome |
| --- | --- | --- |
| 8 | Watchlist sync | **CUT** — out of scope |
| 10 | Discovery / recommended browsing pages | **CUT** — management console, not a browser. Editor preview endpoints retained |
| 12 | Permission model depth | **Admin-only surface in Tier 0, schema modelled for per-user targeting** |
| 13 | Managed-user list GUI | Tier 1 (follows 12) |
| 14 | Service-user account | Tier 1 (follows 12; schema accommodates) |
| 15 | Plex user labels for visibility scoping | Tier 1 (follows 12) |

Accepted as recommended, 2026-08-08 — flagged for reopening only if disputed:

| # | Decision | Outcome |
| --- | --- | --- |
| 6 | Provider brand palettes | Tier 1 |
| 7 | Theme music | Tier 1 — struck from the Tier 0 list |
| 9 | Existing-item tagging | Tier 1 |
| 11 | Plex webhook receiver | Tier 1, route reserved in Tier 0 |
| 16 | Server filesystem browser | Tier 0, jailed to configured roots |
| 18 | Update check | Tier 1, opt-in |
| 19 | Locales | i18n framework at first milestone, English at launch |
| 20 | Separator collections | Tier 1 |

**Scope status: frozen for PRD purposes.** All twenty items resolved. Reopening any of them is a
change request against this ledger, recorded with a date, not a silent edit.

#### Cuts confirmed

| Cut | Reason |
| --- | --- |
| Maintainerr integration | Peripheral; a separate tool's domain |
| YouTube trailer downloads | Fragile external dependency; a static placeholder video suffices |
| Quick-sync / full-sync split | Two rule paths that drift; replaced by one pipeline |
| Overlay quick sync as a separate path | Same reason; incrementality belongs in the render cache, not in a second pipeline |
| Ordering-method alternation counter | A workaround for a convergence bug that gets fixed properly instead |
| Linked-collection grouping machinery | Made unnecessary by structural multi-library definition targeting |
| Watchlist sync | A second product sharing *arr plumbing; unrelated to collections, posters, or overlays |
| Discovery / recommended browsing pages | Afisharr is a management console; browsing happens in Plex |
| Music library support | Recorded as a non-goal, §2.4 |
| Preroll management | Recorded as a non-goal, §2.4 |
| Foreign-config importers | Recorded as a non-goal, §2.4, and in the definition-layer and onboarding descriptions |
| Headless browser | Recorded as a non-goal, §2.4, and in the external source policy |

#### Consequences to carry forward

**Tier 0 grew by roughly a factor of two.** Full hub ownership, foreign-collection adoption, four
restored features, and a visual poster editor are each substantial. An earlier 20-week, nine-milestone
plan could not absorb this and was rebuilt rather than compressed — see the implementation plan.

**Hub ordering is the highest-risk subsystem in the product.** Afisharr writes positions for its own
collections, foreign collections, and Plex's native hubs, in one shared ordering space, across
multiple libraries. This needs a specified convergent algorithm with a proof obligation — not a retry
loop, and not the alternating-strategy workaround that was cut. It deserves a dedicated design
treatment ahead of implementation.

**The definition layer must carry hub and foreign-collection state.** Beyond Collection, Playlist,
OverlayTemplate, PosterTemplate, SmartFilterDef, and PackManifest, owning native hubs and adopted
collections means either two new kinds (`HubPlacement`, `AdoptedCollection`) or one shared `Placement`
kind covering all three. This was the first question for the data-model pass.

**Placeholder-vs-native-hub pollution is a first-class design constraint,** not an accident to be
patched. Any item Afisharr materializes into a library must carry a marker the hub-replacement system
can filter on, decided at the schema level.

**The poster editor implies the overlay editor.** Both are layer-based canvas editors over the same
element model; building two would be waste. One editor, two document kinds — which makes the element
model in the engine's schema load-bearing for both and worth a second review pass.

**Cutting watchlist sync removes per-user Plex tokens from Tier 0 entirely.** It was the only Tier 0
feature needing a Plex credential other than the admin's. This aligns cleanly with the admin-only
permission surface: one Plex identity does every write in the first release.

**Cutting the browsing pages does not cut the preview endpoints.** The collection editor must still
answer "what would this collection contain right now?" before a save, and that stays Tier 0. The
distinction is explicit here so the capability is not cut twice by accident.

**"Schema modelled for per-user, surface admin-only" is a testable claim, not a good intention.** The
data-model pass owes it a concrete obligation: visibility is stored as a set of principals from day
one, with `everyone` as the only value the Tier 0 GUI can write. If a Tier 1 feature later needs a
migration to add per-user targeting, this decision was not honoured.

## 3. Architecture

Single binary. Axum HTTP API plus an embedded SvelteKit static SPA. SQLite via SQLx with migrations.
OpenAPI via utoipa generating the TypeScript client. SSE for job progress and source health.

The two surfaces get one directory each, and neither keeps anything at the repository root.

```
backend/           the Cargo workspace, and everything scoped to it:
                   Cargo.toml, Cargo.lock, rust-toolchain.toml, rustfmt.toml,
                   clippy.toml, deny.toml, .cargo/config.toml, .sqlx/
  crates/core      domain model, definition schema, reconciliation engine,
                   filter/order pipeline, lifecycle state machine, scheduling
  crates/sources   one module per provider behind a common SourceBuilder trait;
                   each with typed client, rate limiter, circuit breaker,
                   response validation, health status
  crates/plex      Plex client: libraries, collections, hubs, labels,
                   media streams, artwork, filter-metadata discovery
  crates/render    poster and overlay renderer, element model, layers,
                   font handling, content-addressed cache
  crates/packs     pack format (manifest, definitions, assets) and installer
  crates/api       Axum routes, auth and sessions, SSE, OpenAPI
  crates/afisharr  binary: wiring, embedded SPA, migrations, CLI entrypoints
frontend/          SvelteKit 2, Svelte 5, UnoCSS (presetWind4), shadcn-svelte,
                   adapter-static, Bun tooling
scripts/           the gates that span both surfaces, so they belong to neither
docs/              this document and the implementation plan
.github/           the merge, nightly, and release lanes (§A.5 of the plan)
```

Rust toolchain pinned at 1.97.1, edition 2024.

**Every cargo command runs from `backend/`.** Cargo discovers `.cargo/config.toml` and rustup
discovers `rust-toolchain.toml` by walking *up* from the working directory, and neither ever
descends. A cargo step left at the repository root gets the default toolchain and no `[env]` block,
silently. The offline query data needs no such care: sqlx resolves `.sqlx/` through `cargo metadata`
→ `workspace_root`, which follows the manifest rather than the working directory.

**Inside each crate, the same division continues.** A crate's `src/` divides into subfolders named
after a domain, not after a layer; every file states one thing; no module collects unrelated
responsibilities; every file carries a size limit; and every module declares a narrow public surface.
This is normative and gated per change, not a cleanup to schedule. Full requirement in §24.6,
recorded as D-047.

**Frontend boundary.** The SPA is fully prerendered and embedded; there is no JavaScript server
runtime in production. Server-side SvelteKit features — server load functions, form actions, server
hooks, server-side form validation, direct database access from the frontend — are structurally
unavailable and must not be used. All data flow is client-side fetch against the Rust API using the
generated typed client. General frontend coding guidance documents those server-side features because
it describes the framework generally; on this stack their server-side half does not apply. This is the
one exception to how those guidelines otherwise govern how code is written.
## 4. Users and modes

### 4.1 The states are the product

Most management consoles have three states — loading, empty, error — and flatten every other
condition into one of them. I cannot do that, because the engine deliberately distinguishes
conditions a conventional UI would collapse.

A collection can be: currently syncing; empty because nothing matched; empty because a source failed
and the contribution was frozen at last-known-good; populated but rendering against stale evidence;
populated but degraded because a field the definition uses is unavailable on this server; correct but
unplaceable because the library will not converge; or fine. Seven conditions, and a UI with three
states shows six of them as either "loading" or "error".

Every hard decision in the engine — persisting state rather than recomputing it, refusing to act on
absent evidence, freezing rather than emptying, flagging rather than dropping — exists so I can tell
the truth about what I know. **If the interface flattens those distinctions, the entire design is
discarded at the last mile.** The user sees a spinner, then a wrong answer, and none of the care
underneath ever reaches them.

So the state policy in §8 is not a detail at the end of this document. It is the reason the document
is shaped the way it is, and it is the acceptance criterion for every page in §7.

The second thesis follows from §1: **GUI-first, never GUI-trapped.** Every page is an editor
over a definition document. Anything the GUI can produce can be exported, diffed, version controlled,
and re-imported unchanged. The GUI is the most convenient way to author definitions, never the only
way.

### 4.2 Constraints that shape the interface

These are settled elsewhere and are restated here because they eliminate whole categories of design
that would otherwise seem available.

| Constraint | Consequence for the interface | Source |
| --- | --- | --- |
| Static SPA, no JavaScript server runtime | No server load functions, no form actions, no server hooks, no server-side validation. All data flow is client-side fetch against the Rust API through the generated typed client. Auth state is client-held and verified per request | §3 |
| Admin-only surface at launch | One audience. No role-switching UI, no permission editors, no per-user views. Visibility controls write whole-audience values only | §2, §17.8 |
| Afisharr is a management console, not a browser | No discovery pages, no content browsing, no "what should I watch". Browsing happens in Plex. The editor's *preview* — "what would this collection contain right now?" — is a different thing and is in scope | §2.4 |
| Everything is a definition | Every editor is a form over a document. Export, import, history, and diff are available everywhere, not on a special page | §12.1 |
| Forms are generated from schemas | Source parameter forms come from each source's JSON Schema; condition builders come from the field registry. Adding a source adds a form with no frontend work | §13.1, §16.3 |
| One renderer | The editor preview uses the same renderer as production output, compiled into the server. Preview cannot drift from result | §12.6 |
| i18n framework from day one, English only shipped | No hard-coded user-facing strings, ever. A lint rule enforces it | §2 |
| Full reversibility is a product commitment | Teardown is a first-class surface, not a hidden command | D-022 |

### 4.3 The operator — the only Tier 0 user

I model three audiences, only one of which uses the interface, plus one I deliberately exclude.

The operator runs a Plex server for a household or a small circle. They are technically capable:
comfortable with Docker, a reverse proxy, and the `*arr` suite. Not necessarily a developer, and
specifically **not** an expert in this domain — they know what they want the home screen to look
like, not what a fractional position budget is.

There is one operator. Modelling more personas at Tier 0 would be inventing users to justify features
the scope has already excluded.

What matters is that the operator works in **two modes**, and the modes want opposite things from the
interface. This distinction does more design work than any persona split would.

**Builder mode.** Long sessions at a desk. Creating collections, designing overlay templates,
arranging the home screen. Wants density, keyboard navigation, live preview, undo, and to be left
alone. Tolerates complexity in exchange for control. This is where the operator spends their first
weekend and then a few hours a month.

**Checker mode.** Thirty seconds, often on a phone, often prompted by someone in the household saying
something looks wrong. Wants one question answered: *is anything broken, and if so what?* Tolerates
no complexity at all. This is where the operator spends nearly all of their total sessions after the
first month.

Two consequences follow, both non-obvious and both load-bearing:

1. **The dashboard is the most-used page in the product and the least interesting to design.** It must
   answer "is it fine?" above the fold, on a phone, without interaction. Everything else on it is
   secondary.
2. **Builder-mode pages need not be responsive below tablet width.** A layered canvas editor on a
   phone is a bad experience that costs real effort to build. Checker-mode surfaces — dashboard,
   doctor, job status, logs — must be excellent on a phone. I state the split here so it is a decision
   rather than an accident.

### 4.4 The pack author — Tier 1, but the tooling is Tier 0

The pack author authors overlay and collection packs for others. They use the same editors as the
operator plus export and a manifest. I give them no dedicated surface at launch; the requirement they
impose on Tier 0 is that every editor produces a clean, exportable, portable document — a constraint
on the editors, not a page.

### 4.5 The household — never uses this interface, and is the point

The household is the people who open Plex and see the results. They never see Afisharr. Every decision
here is ultimately judged by what appears on their home screen: whether posters look right, whether a
collection is suddenly empty, whether an unreleased film shows a sensible badge.

I name them because doing so settles arguments. When a design choice trades operator convenience
against household-visible correctness — an "apply now, verify later" button, a fast path that skips
verification — the household wins. They cannot report a bug, and they will not know Afisharr exists
when something looks wrong.

### 4.6 Excluded: the household member as a user

No Tier 0 surface lets a non-admin log in and manage their own collections. The permission schema
supports it (§19.6.1) and Tier 1 may add it. Until then, I design no page with a second
audience in mind, because designing for a user who does not exist yet produces navigation that serves
neither.

---

## 5. Journeys

Six journeys. Each is a sequence the product must support end to end; each names its failure mode,
because the failure mode is what the design has to be tested against.

### 5.1 First run — from a fresh container to a populated library

**Target: under ten minutes, zero hand-written definitions** (§5.1).

1. **Claim the instance.** The operator reads a `xxxx-xxxx-xxxx` token from the server console, where
   it was printed at startup, and enters it. The wizard is then leased to that browser for ten
   minutes at a time. Nothing else on the instance responds until this completes. Mechanism and
   parameters are §19.6.1; the reasoning is D-045.
2. **Create an admin account.** Local credentials or Plex sign-in. Nothing else is reachable until
   this completes.
3. **Connect to Plex.** PIN or OAuth flow, then server selection if the account owns several. The
   connection is verified before the step completes — a wizard that accepts an unreachable server and
   fails four steps later has wasted the operator's time and their goodwill.
4. **Select libraries.** Discovered movie and show libraries, with counts. Music and photo libraries
   are not offered, since they are non-goals.
5. **Connect integrations.** TMDB is required. Everything else — Trakt, the `*arr` suite, Overseerr,
   Tautulli — is optional, each with a test button that reports a specific failure rather than a red
   cross. Skipping is a first-class choice with a visible consequence: *"Skipping Radarr means
   collections cannot request missing films."*
6. **Choose starter packs.** A small set of first-party collection and overlay packs, each with a
   preview of what it produces. Nothing is enabled without an explicit choice; a pack requiring an
   integration that was skipped is shown as degraded with the reason, not hidden and not silently
   installed broken.
7. **Report what is already there.** Afisharr lists the collections it found in the selected libraries,
   with counts per library. **It adopts nothing and offers no bulk adoption control.** The step
   explains what adoption is, states that Afisharr leaves these collections alone until told otherwise,
   and links to the page where adoption happens. Decided as D-026.
8. **Review and run.** A plain-language summary of what the first sync will do, including counts —
   how many collections will be created, whether posters will be replaced, whether placeholder files
   will be written and where. Then the first sync, with live progress.

**The failure mode this journey exists to prevent** is the operator reaching step 8 without
understanding that Afisharr is about to overwrite every poster in their library. Step 8 is a consent
step wearing a summary's clothing, and it must state the irreversible-looking parts plainly — along
with the fact that they are, in fact, reversible.

**The second failure mode is the one step 1 exists for**, and it is not a usability failure: an
instance that is reachable before it is configured belongs to whoever loads it first. D-029 says
plainly that the instance may face the internet, so an unclaimed wizard is an open offer of an admin
account and, one step later, of a Plex token that authorises deletion. The console token is the
cheapest available proof that the person at the keyboard is the person who started the container.

**Step 1 costs one paste, and the ten-minute target absorbs it.** The token is on screen in the same
terminal that just ran `docker compose up`, which is where an operator installing a self-hosted
service already is.

**The journey is resumable and re-runnable.** Which step it resumes at is derived from what is
actually in the database, never from anything the browser sends (D-046, §7.14). An operator who
closes the tab at step 5 reopens at step 5. An operator whose container restarted at step 5 signs in
with the admin account they created at step 2 and continues; the console token is gone by then, and
recovery does not need it.

**Step 7 has no "adopt all" button, and that is the point.** An operator with sixty hand-made
collections is exactly the operator for whom one click is most tempting and most alarming. Bulk
adoption five minutes after install is the fastest way to make someone feel they lost control of
their own library, and a first impression is the worst moment to spend that trust — even though
teardown (D-022) makes adoption genuinely reversible. Reporting still pays the step's way: the
operator learns Afisharr sees their collections, learns it will not touch them, and learns where the
control lives.

**Placeholder configuration is deliberately not in the wizard.** Writing files into a user's library
is the most invasive thing Afisharr does, and it is not a first-run decision. The wizard mentions the
capability and points at where to enable it.

### 5.2 Building a collection

The core builder journey, and the one that determines whether the product feels good.

1. New collection → choose target libraries (a set, not one).
2. Add sources. Each source's form is generated from its schema. A source that needs credentials that
   are not configured is listed but disabled, with a link to configure it.
3. **Preview.** At any point, "what would this contain right now?" — resolved items with the source
   each came from, and the count. This runs against live sources and is explicitly not a save.
4. Filters, built with the registry-generated condition builder. Invalid combinations are impossible
   to construct rather than rejected after the fact — a numeric operator is not offered on an
   enumeration.
5. Order, limit, and reconciliation policy.
6. Presentation: poster template or upload, summary, placement.
7. Lifecycle: whether to materialise placeholders and whether to acquire, both off by default.
8. Schedule.
9. Save. Validation runs; errors point at the specific control.

**Preview is the feature that makes this journey work**, and it is the one most likely to be cut for
cost. Without it the loop is: guess, save, wait for a sync, look in Plex, guess again. That loop is
several minutes long and is the reason tools in this category feel unusable. It is Tier 0
(§2.6) and I treat it as load-bearing rather than as a nicety.

### 5.3 Designing an overlay or poster template

One editor, two document kinds (§12.6). Layer list, canvas, element properties, condition
editor per element, live preview against a chosen real item from the library.

The preview must be switchable across items that exercise different conditions — a 4K Dolby Vision
film, an SD one, an unreleased placeholder, an item missing ratings — because the entire point of
conditional elements is behaviour that differs per item, and a preview showing one item verifies
nothing.

**Failure mode:** a template that looks right on the previewed item and wrong on nine thousand others.
The mitigation is a "preview across a sample" mode showing the template rendered over a grid of items
chosen to span the conditions the template references.

### 5.4 Arranging the home screen

The riskiest subsystem gets the most direct interface: a single ordered board per surface, mixing
Afisharr's collections, adopted collections, and native Plex rows, with drag to reorder and visibility
toggles.

Three things this page must make legible, because they are true and surprising:

- **Some rows cannot be moved as freely as others.** Native hubs are anchors — they cannot be removed
  from the ordering space and re-added. The board shows this rather than letting the operator discover
  it through a move that does not stick.
- **Adopted collections require consent before their sort title is modified**, with the before and
  after shown.
- **Reordering costs something.** Not a number the operator must understand, but a visible signal when
  a library is under ordering pressure, and an honest explanation when a rebalance is scheduled.

**Failure mode:** an operator drags a row, the board shows it in the new position, and the Plex home
screen disagrees. The board must reflect *verified* state, not intended state — a move is shown as
pending until read-back confirms it.

### 5.5 Ongoing operation — checker mode

Someone in the house says the "New Releases" row looks wrong. The operator opens Afisharr on a phone.

The answer must be reachable in one view and at most one tap: everything is fine, or these specific
things are not, with each linking to the thing that is wrong. No hunting through logs, no
cross-referencing job history against collection status.

This journey is why `doctor_findings` is durable and deduplicated rather than recomputed (*Data
model* §15): the question "what is wrong right now" must be answerable by a single cheap query, not
by re-running every check while the operator waits.

### 5.6 Leaving

The operator decides Afisharr is not for them.

Settings → Teardown. A preview of exactly what will be restored and what will be deleted, with counts:
posters restored, sort titles reverted, labels removed, collections deleted, placeholder files
removed, hub placement restored. A typed confirmation. Then a progress view, resumable if interrupted,
ending in a report of anything that could not be restored and why.

**This journey is a feature, not an admission.** It is the cheapest possible answer to "what if I try
this and hate it", and it is the only thing that proves the reversibility invariants work
(I-REV-4, *Invariants*).

---

## 6. Information architecture

### 6.1 Navigation model

Six primary destinations plus a settings area. The organising principle is **the object the operator
is thinking about**, not the subsystem that owns it.

```
Dashboard        is anything wrong, and what ran recently
Collections      the definitions that build collections; list and editor
Design           poster and overlay templates, packs, assets
Home Screen      placement and visibility across home and library surfaces
Lifecycle        upcoming titles, placeholders, acquisition activity
Doctor           everything that needs a decision or is not right
─────────────
Settings         Plex, integrations, libraries, jobs, users, exclusions,
                 general, teardown, about, logs
```

Three conscious choices here:

**Doctor is primary navigation, not buried in settings.** It is where the product's honesty becomes
visible. Making it a settings sub-page signals that unresolved problems are an administrative
afterthought, which is the opposite of the intent.

**Lifecycle is primary navigation.** It is the differentiator (§1). Filing it under
collections would hide the thing that makes the product worth running.

**Overlays and posters live together under Design**, because they are the same editor over the same
element model. Splitting them in navigation would imply two systems and invite two implementations —
the failure mode pattern P7 in *Invariants* exists to prevent.

### 6.2 What is deliberately absent

| Not present | Why |
| --- | --- |
| A "browse content" or "discover" section | Non-goal. Browsing happens in Plex |
| A separate "definitions" section | Every page is already a definition editor; a raw document browser would be a second way to do everything. Export and history live on each object |
| A notifications inbox | Findings are durable and live on the doctor page. A second, ephemeral notification stream would compete with it and lose |
| Per-user views or a role switcher | Admin-only surface at launch |
| A dedicated search page | Search is scoped to where it is used — collections list, asset picker, item picker in the editor preview |

### 6.3 Cross-cutting affordances

Available on every definition-backed object rather than on a special page:

- **Export** — canonical JSON, round-trippable.
- **History** — the last twenty bodies, with a diff and restore.
- **Duplicate** — including forking a pack-origin definition to `user/`.
- **Enable / disable** — operational state, distinct from deleting.
- **Where used** — inbound references, resolved from the reference graph.

### 6.4 The source link is a licence obligation, not a courtesy

Afisharr is a web application under AGPL-3.0-or-later (D-028), so section 13 of that licence obliges
any modified instance offered to other users over a network to make its source available to them. The
interface therefore carries a permanent **Source** link, reachable from every page, pointing at the
source for the exact running version.

Two requirements follow, and both are easy to get wrong:

1. **The link resolves to the running version, not to whatever is on the default branch.** It is
   built from the version stamp the binary carries. A link to `main` from a six-month-old container
   satisfies nobody and misstates what the operator is running.
2. **It survives forking.** Someone running a modified build has the same obligation, so the target
   is configurable rather than compiled in. A field that a fork can point at its own repository is
   what makes the obligation dischargeable by the person who inherits it.

The version stamp, the link, and the licence name sit together on an **About** panel in Settings
(§7.13), and the link itself is repeated in the footer.

---

## 7. Page inventory

Each page lists its purpose, the states it must handle beyond the universal ones in §8, and its
primary acceptance criterion.

### 7.1 Dashboard

**Purpose:** answer "is it fine?" without interaction, on a phone.

**Above the fold:** a single status line — everything is fine, or *N* things need attention — followed
by the specific items, each linking to its object. Then recent job activity and the next scheduled
run.

**Special states:** first run with nothing configured (routes to the wizard); a sync currently in
progress (live, via SSE); degraded operation where some sources are frozen but collections are
otherwise correct.

**Acceptance:** on a 375px viewport, with three open findings, the operator can identify all three
and reach the first without scrolling past the fold or tapping twice.

### 7.2 Collections list

**Purpose:** the working list. Name, target libraries, item count, last run outcome, next run,
current state.

**Special states:** frozen (a source failed, showing last-known-good), degraded (a referenced field is
unavailable), never run, disabled, mid-sync.

**Acceptance:** the state of every collection is legible from the list without opening any of them.

### 7.3 Collection editor

**Purpose:** the primary builder surface. Sections for sources, filters, order, presentation,
lifecycle, and schedule.

**Special states:** unsaved changes; validation errors bound to specific controls; a save conflict
where the definition changed elsewhere (shows a diff, never silently overwrites — I-DATA-3,
*Invariants*); a source whose credentials are missing; preview running; preview failed for one source
but succeeded for others.

**Acceptance:** every validation error highlights the control that caused it. A structured error the
UI cannot bind to a control is a bug in the error, not in the UI.

### 7.4 Preview panel

**Purpose:** "what would this contain right now?" Resolved items with per-source attribution and
counts.

**Special states:** partial results where one source failed — shown with the failure named and the
successful sources still listed, never replaced by an error page.

**Acceptance:** a single failing source never hides the results from the others.

### 7.5 Template editor (overlay and poster)

**Purpose:** layered canvas editor. Layer list, canvas, element inspector, per-element conditions,
live preview.

**Special states:** a referenced font or icon asset is missing; an element's bound field is
unavailable on this server; preview item has no media (so `media.*` resolves null); pack-origin
template is read-only until forked.

**Acceptance:** preview output is byte-identical to applied output for the same inputs (I-RENDER-3,
*Invariants*).

### 7.6 Home screen board

**Purpose:** ordering and visibility across the home surface and each library surface.

**Special states:** move pending verification; move failed verification; library non-convergent;
rebalance scheduled or in progress; adopted collection lacking consent; anchor row; unrecognised
participant present.

**Acceptance:** the board never shows an order as settled before read-back confirms it, and never
offers an operation on an anchor that anchors cannot support.

### 7.7 Lifecycle

**Purpose:** what is coming, what is materialised, what is being acquired.

**Views:** upcoming by date; placeholders currently in the library; acquisition activity; **stale
placeholders past their retirement window** (per D-011 — the list that makes "keep" a managed choice
rather than silent accumulation), with bulk removal.

**Special states:** stale subject (evidence could not be refreshed); ambiguous match blocking action;
intent pending; retire-window expired.

**Acceptance:** every placeholder in the library is reachable from this page, and each shows why it
exists and which definitions want it.

### 7.8 Doctor

**Purpose:** everything that needs a decision or is not right, and the destructive operator actions
that exist nowhere else.

**Contents:** open findings by severity; configuration and connectivity checks; ambiguous-match
resolution (authoritative surface per D-013); suspect base posters; orphan-sweep candidates awaiting a
decision; non-convergent libraries; asset store reconciliation; and the explicitly dangerous actions —
full hub reset, forced re-discovery, cache rebuild — each behind a preview of what will be lost.

**Acceptance:** no destructive action executes without a preview naming the specific objects affected.

### 7.9 Packs

**Purpose:** installed packs, their state, and installation from file, URL, or repository.

**Special states:** degraded (requires an unconfigured integration, with the specific fields named);
update available; user forks now behind upstream.

**Acceptance:** a pack that cannot work on this server says so before installation, not after.

### 7.10 Assets

**Purpose:** fonts, icons, uploads, local asset roots, and the jailed filesystem browser.

**Special states:** asset file missing from the store; local root unreadable; asset in use (deletion
blocked, with the users listed).

**Acceptance:** the filesystem browser cannot navigate outside a configured root, and says so plainly
when a path is refused.

### 7.11 Jobs and schedules

**Purpose:** what runs, when, what happened, and manual triggering.

**Special states:** running; overdue; repeatedly failing with backoff; disabled.

**Acceptance:** every job's next run is shown as a concrete time, not only as a cron expression.

### 7.12 Logs

**Purpose:** structured run events, filterable by run, definition, library, source, and level.

**Acceptance:** "everything that happened to this collection during last night's run" is one filter,
not a text search.

### 7.13 Settings

Sub-pages: Plex connection; libraries; integrations; general (timezone, locale, appearance); users and
API keys; exclusions; teardown; about.

**About carries a licence obligation.** It states the licence, the exact running version, and the
source link for that version, per §6.4. The source-link target is editable here, so a fork can
discharge the same obligation.

**Special states:** a configured integration that has since become unreachable; a library that has
disappeared from the server; a changed server machine identifier (blocking, per I-ID-5, *Invariants*).

### 7.14 Setup wizard

**Purpose:** journey §5.1. Eight steps — Claim, Admin, Plex, Libraries, Integrations, Packs, Report,
Review — resumable, and re-runnable later without destroying existing configuration.

**The resume step is derived, never carried.** The wizard asks the server which step it is on and
the server answers from state, returning the first step whose evidence is absent. A step index in a
query string, a cookie, or a client-held draft would let a caller name the step they would like to
be on, which on the claim step means naming step 2 (D-046).

| Resumes at | When |
| --- | --- |
| 1 Claim | no active `setup:claim` lease matching this browser's cookie |
| 2 Admin | no admin user row exists |
| 3 Plex | no `plex_server` row, or the `plex.token` secret is absent |
| 4 Libraries | no library is selected |
| 5 Integrations | the TMDB credential is absent — it is the one required integration |
| 6 Packs | `packs` is not in `instance.setup_acked_steps` |
| 7 Report | `existingCollections` is not in `instance.setup_acked_steps` |
| 8 Review | everything above is satisfied |

Steps 6 and 7 complete by acknowledgement because neither necessarily writes configuration: choosing
no starter packs is a valid choice, and the report writes nothing by design (D-026). §19.5 carries
the column.

**Re-running it later is a different mode, and says so.** The wizard reached from Settings on a
configured instance edits configuration and never asks for a token — the operator is already
authenticated, and a claim is meaningless once `setup_completed_at` is set. It shows current values
as current values rather than as empty fields, and completing it destroys nothing the operator did
not change.

**Special states:**

- **Blocked** — the wizard is claimed by another browser and this one does not hold the claim. The
  response says `blocked`, names the reason, and carries the time the claim expires; the client
  renders the shared Blocked component (§8.1) with that time. It does not invent a state and it does
  not relax a gate. This is the one genuinely stranded case: before an admin exists, all three doors
  are shut — the wizard steps refuse without a claim, a fresh claim refuses because one is held, and
  recovery refuses because there is no account to recover with. The only correct answer is to say
  when the wait ends.
- **Recovery available** — an admin account exists, setup is incomplete, and no claim is active. The
  claim step offers admin sign-in alongside the token field.
- **Token expired or wrong** — one message for wrong, expired, malformed, and empty, because
  distinguishing them tells a guesser which of the four they achieved. The message says where a fresh
  token comes from: restart the container and read the console.
- **Rate limited** — 429 with the retry time shown, per §21.4.3.
- **Integration unreachable at its step** — the step reports the specific failure and offers to skip
  where skipping is allowed, per journey step 5. It never accepts an unverified connection.

**Acceptance:** I-UX-8 for the ten-minute target, I-SEC-8 for the claim, I-UX-10 for the derived
resume.

### 7.15 Teardown

**Purpose:** journey §5.6. Preview, typed confirmation, resumable progress, final report.

**Acceptance:** the preview's counts match what teardown actually does, and interrupting it mid-run
leaves a library that a resumed run finishes correctly.

---

## 8. State policy

The core of this document. Every data-bearing surface must handle every state below, and "handle"
means a specific, designed treatment — not a fallback.

### 8.1 The nine states

Three are universal to any interface. Six are specific to what the engine knows, and they are the
ones that make this product honest.

| State | Meaning | Treatment |
| --- | --- | --- |
| **Loading** | Data is being fetched | §8.2 |
| **Empty** | Successfully retrieved, nothing to show | §8.3 |
| **Error** | The request failed | §8.4 |
| **Frozen** | A source failed; the contribution is held at last-known-good | Show the data, marked, with the source and time of last success. Never an error page |
| **Degraded** | Working, but a capability is unavailable — an unconfigured integration, a field this server does not have | Show the result, name the missing capability, link to configure it |
| **Stale** | Evidence could not be refreshed; state is being preserved deliberately | Show last-known values with the age of the evidence. Never blank, never a spinner |
| **Pending** | An intent is committed but not yet confirmed — a placeholder being created, a move not yet verified | Show as in-flight and distinct from settled. Never render optimistically as done |
| **Blocked** | Action is refused pending a human decision — an ambiguous match, missing consent, a suspect base poster | Show what is blocked, why, and the one action that unblocks it |
| **Non-convergent** | An ordering surface cannot be settled within the escalation ladder | Show last verified state, the specific items that would not settle, and that it will be retried |

**The rule that binds them:** these states are reported by the API and rendered by the UI. I never let
the frontend *infer* a state — it does not decide that a slow response means degraded, or that an
empty array means empty. The engine has already made that distinction carefully, and re-deriving it
in the client would silently reintroduce exactly the flattening this design exists to avoid.

This makes a demand on the API, stated here because it is easy to miss: **every collection endpoint
returns the state alongside the data.** A bare array is not a sufficient response shape.

### 8.2 Loading

- **Under 300ms:** show nothing. A flash of skeleton is worse than a brief pause.
- **300ms to ~3s:** skeleton matching the shape of the content, so layout does not jump.
- **Beyond ~3s:** progress with what is happening — *"Fetching from TMDB (2 of 5 sources)"* — because
  a spinner that has been turning for eight seconds is indistinguishable from a hang.
- **Long operations** (sync, teardown, bulk render) use SSE progress with counts and the current
  item, never an indeterminate bar.
- **Partial data renders as it arrives.** A page needing four calls shows three sections and one
  skeleton, not one skeleton.

### 8.3 Empty

Every empty state answers: what would be here, why is it not, and what is the one thing to do next.

Three kinds, and conflating them is the most common failure:

| Kind | Example | Treatment |
| --- | --- | --- |
| **Nothing created yet** | No collections | Explain the concept in one line and offer the primary action |
| **Nothing matched** | A filter excludes everything | State that the query succeeded and returned nothing, and show the narrowing predicate |
| **Nothing yet, but pending** | Never synced | Say so and offer to run now |

**"Nothing matched" is never shown for a failed fetch.** That conflation is the interface expression
of pattern P1 in *Invariants* — absence of evidence presented as evidence of absence — and it teaches
the operator to distrust every empty state in the product.

### 8.4 Error

- **Say what failed, in terms of the thing the operator recognises.** *"Radarr (4K) did not respond"*,
  not *"Request failed with status code 500"*.
- **Say what it means for them.** *"Missing films will not be requested until this is fixed."*
- **Offer the next action** — retry, configure, view details.
- **Keep the technical detail available but collapsed.** The operator may be pasting it into a forum
  thread.
- **Never discard partial success.** Four of five sources worked; show the four.
- **Never block the page for a peripheral failure.** A failed rating lookup does not prevent a
  collection from rendering.

### 8.5 Destructive actions

Anything that deletes user data, writes into a library, or cannot be undone by pressing the button
again:

1. **Preview first**, with specific counts and named objects. *"This will delete 47 placeholder files
   from /media/placeholders"*, not *"This will remove placeholders"*.
2. **Confirmation proportional to consequence.** A button for reversible actions; typed confirmation
   for teardown, full hub reset, and bulk placeholder removal.
3. **Report afterwards** — what was done, what failed, what was skipped.
4. **Never destructive by default.** No destructive action is a default button, a keyboard-focused
   element, or reachable without an intermediate step.

### 8.6 What the interface must never do

| Never | Because |
| --- | --- |
| Show a spinner where the engine has a known last value | Stale data with its age is more useful than no data, and it is what the engine deliberately preserved |
| Render an optimistic result as settled | Placement in particular: a move is pending until read-back confirms it |
| Convert a rich state into a generic error | Frozen, degraded, and blocked are not errors and must not be styled as failures |
| Auto-retry silently in a loop | Retry is an action with a visible outcome |
| Auto-save a definition | Saving runs validation and may conflict; it is deliberate |
| Hide a finding because it is old | Findings resolve or are acknowledged; they do not expire |

---

## 9. Live status

Job progress, source health, and pass outcomes stream over SSE (§10).

- **One connection**, multiplexed by topic, established after auth and reconnected with backoff.
- **The stream is an accelerator, not a source of truth.** Every surface it feeds must be correct
  after a plain page load with no stream at all. A lost connection degrades liveness, never
  correctness.
- **Reconnection reconciles by refetching**, never by replaying missed events.
- **Disconnection is visible** — a small, non-modal indicator. Silently showing frozen numbers as
  live is a way to make an operator distrust the whole interface.

Placement pressure metrics — moves and rebalances per pass — surface on the home screen board and the
doctor page, because §15.9 identifies them as leading indicators of a failure that is
otherwise only visible once it has already happened.

---

## 10. Cross-cutting interface requirements

### 10.1 Internationalisation

No hard-coded user-facing strings, enforced by lint from the first commit. Message catalogues with
interpolation and plural rules. English ships; the framework is complete.

The concrete reason this cannot be deferred: formatters in §13.5 are locale-dependent, and a
pack may pin a locale for a badge while the interface follows the user's setting. Locale is already a
data concept in the engine; retrofitting the interface half later means touching every component.

The scope for this framework is:

| Capability | What it does | Evidence | Recommendation | Decision |
| --- | --- | --- | --- | --- |
| UI translation framework | Message catalog + locale switch | `src/i18n`, `main.locale` | T0 (framework from day one) | T0 |
| Shipped locales | 14 locales present: da, de, en, es, fr, hu, it, ja, nl, pt-BR, ru, sv, uk, zh-Hans | `src/i18n/locale` | Framework in M0; **English at launch**; community locales after | **T0 (en) / T1 (rest)** |

Retrofitting i18n is far more expensive than building with it. The framework, message extraction, and
the "no hardcoded strings" lint rule belong in the first milestone even if only English ships.
Shipping fourteen *maintained* locales at launch is a different commitment — that is the open
question, not whether to support i18n.

### 10.2 Accessibility

Baseline, non-negotiable: keyboard reachability for every interactive element, visible focus, semantic
markup, form labels bound to controls, WCAG AA contrast in both themes, live-region announcements for
async results, and respect for reduced-motion.

**Drag-and-drop always has a keyboard equivalent.** The home screen board is the primary ordering
surface, and an interface where the main feature is mouse-only is not accessible in any meaningful
sense. Move-up, move-down, and move-to-position satisfy this and are also faster for bulk work.

The layered canvas editor is the hard case. My commitment: every operation reachable by drag is also
reachable through the layer list and the element inspector, both of which are ordinary keyboard-
navigable controls. The canvas is an accelerator over an accessible model, not the only way in.

### 10.3 Responsive baseline

| Surface | Phone | Tablet | Desktop |
| --- | --- | --- | --- |
| Dashboard, doctor, jobs, logs, lifecycle lists | Excellent | Excellent | Excellent |
| Collections list, settings | Usable | Good | Excellent |
| Collection editor | Viewable, limited editing | Usable | Excellent |
| Home screen board | View and reorder via keyboard-equivalent controls | Good | Excellent |
| Template editor | Not supported; states so plainly | Limited | Excellent |

The template editor's phone exclusion is a decision, not an omission (§4.3). A page that is not
supported says so; it does not render broken.

### 10.4 Theming

Light and dark, following the system by default, with an explicit override. Both are first-class —
this product is used at night, and a dark theme that is an afterthought shows.

**The palette is `tangerine`, taken from tweakcn.** It is fetched once as a shadcn registry item and
lands in the repository as CSS variables:

```bash
bunx shadcn@latest add https://tweakcn.com/r/themes/tangerine.json
```

The item is `"type": "registry:style"` against `https://ui.shadcn.com/schema/registry-item.json`. It
carries no components and no dependencies — only `cssVars.light` (53 tokens), `cssVars.dark` (52),
`cssVars.theme` (fonts, radius `0.75rem`, the `tracking-*` scale), and one `@layer base` rule setting
`body { letter-spacing: var(--tracking-normal) }`. Its primary is a warm orange,
`oklch(0.6397 0.1720 36.4421)`, the same value in both modes; light backs it with a soft blue-grey
`oklch(0.9383 0.0042 236.4993)` and dark with a deep blue-purple `oklch(0.2598 0.0306 262.6666)`.
§24.3.5 says where each group of tokens lands on this stack, and why the command above is a fetch
rather than a build step we depend on.

**The default mode is automatic**: the interface follows the operating system's
`prefers-color-scheme` on first visit, and an explicit choice from the operator overrides and
persists from then on. **Where the system preference cannot be read, the interface renders light.**
That fallback is stated because the obvious implementation gets it backwards — `mode-watcher` tests
`(prefers-color-scheme: light)` and treats every non-match as dark, so a browser without `matchMedia`
lands in dark by accident rather than by choice. Light is the safer miss: a dark interface shown to
somebody who asked for neither is the one that looks broken in a lit room.

**The theme's fonts are self-hosted, never fetched from a font CDN.** Tangerine names Inter, JetBrains
Mono, and Source Serif 4. Loading those from Google's servers would send the operator's IP address to
Google on every page load of a product that collects nothing (D-038, §21.8), which is a privacy
regression arriving through a stylesheet. The faces ship inside the binary with the rest of the SPA,
and the interface makes no outbound request no operator asked for.
## 11. The collection pipeline

I run one pipeline for every collection, every mode. There is no divergent quick path and full
path.

```
sources[]  → resolve to canonical identifiers (TMDB/TVDB/IMDb triple + Plex GUID matching)
           → merge (union / intersect / subtract, per-source caps and priority)
           → filter (exclusions, thresholds, attribute filters, mutual exclusion,
                     time restrictions)
           → order (deterministic: source position, release date, rating,
                     seeded random; franchise parts by release date)
           → reconcile against Plex (diff desired against actual; create, update, trim;
                     self-heal keys by label and canonical identity;
                     never act on a failed or unaffirmed-empty source fetch)
           → apply extras (poster, overlay context, hub placement and visibility)
           → lifecycle (placeholders, acquisition per policy, per-source health report)
```

I state the following as product-level requirements. Each is testable, and I name the invariant
that tests it.

### 11.1 Zero items is a failure unless the source affirms emptiness

A source fetch returning zero items is a failure **unless the source affirmatively reports an
empty list**. Sources declare this capability explicitly; scraped sources default to "cannot
affirm." Failed sources freeze their contribution at last-known-good rather than emptying
collections. Tested by I-SRC-1.

### 11.2 Reconciliation is idempotent

A second run with unchanged inputs performs no writes. Tested by I-IDEM-1.

### 11.3 Ordering converges or reports

Ordering operates on a deduplicated identifier set, converges within a bounded escalation ladder,
and surfaces non-convergence as visible status rather than retrying forever. Tested by I-CONV-6.

### 11.4 Randomness is seeded

Randomness carries an explicit seed rotated on schedule, never per run. Tested by I-IDEM-2.

### 11.5 Library targeting is structural

A definition targets a set of libraries and fans out; there is no per-library configuration
duplicate to keep in sync.

**The overlay engine.** The original poster is sacred: base art is captured once,
content-addressed and deduplicated. Every application composites pristine base plus current
template plus current state. An overlay is never applied over an overlay.

**Render key = hash(base poster, template version, state snapshot, renderer version).** An
unchanged key skips the upload entirely. Removal is trivially complete: upload the base, drop the
key. I include the renderer version in the key because a rasteriser or font-shaping change alters
the output for identical definition-layer inputs; without it, a rendering improvement matches
every existing cache entry and therefore reaches nobody. Added 2026-08-08.

This generalizes, and I state it as a rule under D-043: a cache keyed only on inputs is correct
only while the function from inputs to outputs is fixed. Where that function ships as code I
change, its version belongs in the key. The HTTP response cache carries a per-source parser
version for exactly this reason (see §19.11.3).

Restoration is byte-exact: removing overlays uploads the stored base poster exactly as
captured — never a resized, re-encoded, or re-cropped copy of it. Overlay inputs, in priority
order, are lifecycle state, Plex media streams, library metadata, and external ratings.
Formatters are pure functions, which is what makes the render key sound — if a formatter could
vary independently of its inputs, the hash would not identify the output. Templates, elements, and
packs are definition documents; the GUI editor and the preview renderer use the same renderer as
production output, so preview and result cannot drift. Episode and season-level overlays are
Tier 1.5.

Every collection, template, placement, and pack is a versioned definition document — canonical
JSON in the database and on export. The GUI is an editor over definitions. Import and export
round-trip exactly.

First-party packs are authored in my own schema, versioned with the app, and updated on my own
schedule. My launch set is: media-info overlay packs, content-rating packs, the lifecycle status
pack, and starter collection packs. Community packs are definition bundles plus assets with a
manifest, installed from file, URL, or repository; a registry may follow.

## 12. The definition layer

Everything in this contract has two halves, and I treat them as one document because they are one
contract. The definition schema in §12 defines the shape a definition may take; the registries it
depends on (the field registry, the operator set, the source registry, the formatter registry)
define the vocabulary those definitions may use. Neither half is checkable without the other: an
expression tree is meaningless until the field registry says which fields exist, and the field
registry exists precisely so the expression tree is validatable at save time rather than at render
time.

I keep the definition schema and its registries merged into one contract rather than split across
documents, because empty-child quantifier semantics, the smart-collection constraint, and the
render-key definition each have exactly one owner this way, and every other place that needs them
cites that owner instead of restating it.

I treat every value a condition can test or a template can render, every comparison a condition can
perform, every source type, and every template transformation as a **registry entry** — data
describing a capability, not code. That single choice buys me four things at once:

1. **The GUI is generated, not written.** A condition builder that knows a field's type,
   cardinality, and legal operators renders the right control automatically. Adding a field adds a
   form control with no frontend work.
2. **Validation is total.** `resolution gte "PG-13"` is rejected when the definition is saved,
   because the registry says `resolution` is an enumeration and `gte` is numeric-only. There is no
   class of "the pack renders nothing and nobody knows why" bug.
3. **Packs are checkable.** A pack declares the fields it requires; the installer can tell the user
   "this pack needs Rotten Tomatoes ratings, which you haven't configured" *before* installing.
4. **Evolution is governed.** Adding, deprecating, and removing capabilities follow one recorded
   process rather than ad-hoc edits scattered across the engine.

All registries are **closed sets under version control**. Nothing may reference a key that is not
registered; validation rejects it at save time, not at render time.

### 12.1 Design principles

**12.1.1 Pure data, no logic.** A definition never contains executable code or a string language to
parse. Conditions and expressions are structured trees. GUI builders, validation, and security all
become trivial — nothing to inject, nothing to sandbox.

**12.1.2 Deterministic.** Same definition plus same world state produces the same result. Anything
random carries an explicit seed that the engine rotates on schedule, never per run.

**12.1.3 Versioned per kind.** Every document declares `kind` and an integer `schemaVersion`.
In-code migrations upgrade old documents on load; I can always read anything Afisharr ever wrote.

**12.1.4 Diffable and round-trippable.** Canonical serialization is JSON with stable key ordering.
Export is pretty-printed canonical JSON, and `import(export(x)) == x` byte-for-byte after
canonicalization.

**12.1.5 Referenced, not embedded.** Definitions reference each other by stable identifier.
Updating a template updates every user of it, unless a reference pins a version.

**12.1.6 Namespaced identifiers.** Every definition has an immutable ULID plus a human handle
`namespace/slug`. Packs own their namespace; user definitions live under `user/`.

### 12.2 The envelope

```json
{
  "kind": "Collection",
  "schemaVersion": 1,
  "registryVersion": 1,
  "id": "01J9Z7Q0K8Y3X2W1V0U9T8S7R6",
  "handle": "user/trending-now",
  "name": "Trending Now",
  "meta": {
    "description": "",
    "createdAt": "2026-08-05T12:00:00Z",
    "updatedAt": "2026-08-05T12:00:00Z",
    "origin": { "type": "user" },
    "tags": []
  },
  "spec": { }
}
```

`origin` is `user`, or `{ "type": "pack", "pack": "afisharr.media-info", "packVersion": "1.2.0" }`.
Pack-originated definitions are read-only in the GUI until forked to `user/` (copy-on-write), so
pack updates never clobber user edits and user edits never block pack updates.

`registryVersion` records which registry revision validated this document. It is what lets a later
release tell the difference between a definition that is wrong and one that was written against an
older vocabulary.

I added `registryVersion` to the envelope, made visibility a principal set, made multi-library
targeting structural, and added the `Placement` kind, all together as one revision. The lifecycle
specification is covered separately (see *Lifecycle*).

### 12.3 Kinds

| kind | what it is |
| --- | --- |
| `Collection` | A managed Plex collection: sources → filters → order → presentation |
| `Playlist` | Same pipeline, ordered and per-user owned (engine at launch, UI later) |
| `Placement` | Position and visibility of one participant on the home and library surfaces |
| `OverlayTemplate` | Layered element canvas rendered onto item posters, driven by item state |
| `PosterTemplate` | Generated collection-poster design |
| `SmartFilterDef` | Reusable filter tree, referenced by collections |
| `PackManifest` | Names a pack, its namespace, version, and member definitions and assets |

Schedules are embedded in the definitions they govern — a collection's schedule is part of what the
collection *is* — rather than being a separate kind.

### 12.4 Collection spec

```json
"spec": {
  "libraries": ["movies", "movies-4k"],
  "sources": [
    { "type": "tmdb.chart", "chart": "trending", "window": "week", "limit": 40 },
    { "type": "trakt.chart", "chart": "trending", "limit": 40 }
  ],
  "merge": { "strategy": "union", "perSourceCap": null, "dedupe": "canonicalId" },
  "filters": { "ref": null, "tree": { "all": [
      { "field": "item.year", "op": "gte", "value": 2000 },
      { "not": { "field": "item.genre", "op": "anyOf", "value": ["Documentary"] } }
  ]}},
  "order": { "by": "sourcePosition", "direction": "asc", "seed": null },
  "limit": 30,
  "reconcile": {
    "onSourceFailure": "freezeContribution",
    "emptyResultPolicy": "requireAffirmativeEmpty",
    "removeDeparted": true,
    "mutualExclusionGroup": null
  },
  "presentation": {
    "posterTemplate": { "ref": "afisharr.core/gradient-title", "pin": null },
    "placement": { "ref": "01J9Z…" },
    "sortTitlePrefix": null,
    "summary": null,
    "wallpaper": null
  },
  "lifecycle": {
    "placeholders": { "enabled": false },
    "acquisition": { "enabled": false, "route": null, "policy": {} }
  },
  "schedule": { "cron": "0 */6 * * *", "jitterSeconds": 300, "enabled": true }
}
```

**Multi-library targeting is structural.** `libraries` is a set, and one definition fans out to a
Plex collection per library. There is no per-library duplicate definition, and therefore no
linking or grouping concept — the thing those would exist to solve does not arise. Per-library
overrides (a different poster for the 4K library) are expressed as a map keyed by library on the
overriding field, never as a second definition.

**Source parameters** are validated against each source type's published JSON Schema, which also
generates the editor form. Adding a source never changes the Collection schema.

**Filters** accept an inline `tree` or a `ref` to a `SmartFilterDef`, never both.

**Reconcile** encodes the safety invariants as data with defaults, so packs can be explicit and the
engine keeps one honest code path.

**Compilation to Plex smart filters.** When every predicate resolves to a server-discovered field
with a compatible operator, the engine may compile the filter to a Plex smart collection; otherwise
it evaluates locally. The definition does not care — with one exception that validation enforces: a
Plex smart collection's membership and order are properties of its filter, so items cannot be added,
removed, or reordered within one. A definition combining Plex-native filtering with
`order.by: sourcePosition` or manual ordering is rejected at save time rather than silently
producing a collection whose order ignores the definition.

**Seeds.** Any source or ordering mode declared non-deterministic requires a seed; validation
rejects its absence.

### 12.5 Condition and filter expression trees

I define one structured expression language, shared by collection filters, overlay element
conditions, and lifecycle rules.

- **Leaves:** `{ "field": <registry key>, "op": <op>, "value": <json> }`
- **Combinators:** `{ "all": [...] }`, `{ "any": [...] }`, `{ "not": <node> }`
- **Scoped quantifiers:** `{ "scope": "episodes", "quantifier": "any" | "all" | "none" |
  { "countGte": 3 }, "tree": <node> }` — filter a parent by facts about its children. This is what
  powers rules like "shows whose last episode aired more than 45 days ago," and it is the mechanism
  behind smart TV acquisition.

Empty-child semantics are where these go wrong, so I specify them exhaustively under
*Empty-child semantics*, and I-DEF-7 tests them. The short form: over zero children, `all` is
vacuously true.

**Fields, operators, and their type compatibility live in the field registry and the operator
set.** The registry is the authority; this document defines only the tree shape. Validation
rejects nonsense such as comparing an enumeration with a numeric operator at save time, and the GUI
condition builder is generated from the registry rather than written by hand.

Regular expressions are permitted on string fields. Safety comes from the engine rather than the
schema: the Rust regex crate guarantees linear-time matching with no backtracking, so catastrophic
patterns are impossible by construction. Patterns are compiled and size-capped at save time.

**No string language. The tree is the language.**

### 12.6 OverlayTemplate spec

```json
"spec": {
  "canvas": { "aspect": "poster", "safeArea": { "top": 0, "right": 0, "bottom": 0, "left": 0 } },
  "variables": [
    { "name": "resolution", "source": "media.videoResolution" },
    { "name": "daysUntil", "source": "lifecycle.daysUntilRelease" }
  ],
  "elements": [
    {
      "id": "badge-bg",
      "type": "tile",
      "layer": 10,
      "anchor": { "h": "left", "v": "top", "dx": 24, "dy": 24 },
      "size": { "w": 220, "h": 72 },
      "style": { "fill": "#000000CC", "cornerRadius": 12 },
      "when": { "field": "media.videoResolution", "op": "in", "value": ["4k", "1080"] }
    },
    {
      "id": "badge-text",
      "type": "text",
      "layer": 11,
      "bindsTo": "badge-bg",
      "text": "{resolution|upper}",
      "font": { "asset": "afisharr.core/inter-bold", "size": 40, "fill": "#FFFFFF" },
      "when": { "field": "media.videoResolution", "op": "exists" }
    }
  ]
}
```

- **Element types (closed set):** `text`, `tile`, `raster`, `svg`, `variable`, `mappedIcon`.
- **Unresolved variables skip their element.** An element whose bound variable resolves to null or
  unavailable is not rendered — never rendered blank, never rendered as a placeholder string. This
  is what makes conditional packs work.
- **Mapped icons require a fallback.** A value-to-asset table without one fails validation, because
  a future release may add an enumeration value the pack has never seen.
- **Formatters** are engine functions from the closed formatter registry, composed into
  type-checked pipelines. They are pure — no clock, no I/O, no randomness — which is what makes the
  render cache key sound: see *The collection pipeline* for the key definition, and a formatter
  that could vary independently of its inputs would stop the key identifying the output. I-RENDER-5
  is the test.
- **Coordinates** are canvas units against a fixed reference size per aspect; the renderer scales.
  `bindsTo` anchors one element relative to another, which is how packs stay
  resolution-independent.
- **Renderer constraint.** Text styling covers fills, single stroke or outline, drop shadow, and
  letter spacing. No per-glyph gradients or text-on-path in v1; pack authoring guidelines target
  this set.
- The template renders in the GUI preview using the *same* renderer compiled into the server, so
  preview and output cannot drift.

The same element model and editor serve `PosterTemplate`; the two kinds differ in their canvas,
their available field scope, and their output target, not in their structure.

### 12.7 Placement spec

```json
"spec": {
  "participant": { "type": "collection", "ref": "01J9Z…" },
  "surfaces": {
    "home":        { "position": 3, "visibility": ["owner", "sharedAll"] },
    "library":     { "position": 1, "zone": "promoted" },
    "recommended": { "visibility": ["everyone"] }
  },
  "randomize": false,
  "sortTitleConsent": false,
  "timeRestriction": {
    "alwaysActive": true,
    "whenInactive": "hide",
    "dateRanges": [],
    "weeklySchedule": null
  }
}
```

`participant.type` is `collection` (Afisharr-managed), `adopted` (a collection the user created), or
`nativeHub` (one of Plex's own rows). The three behave differently, and I record the differences
under *Placement and ordering*; the schema is common so ordering operates on one homogeneous set.

**Visibility is a set of principals**, not booleans, and it is scoped to a *surface*. Amended
2026-08-08: an earlier revision of this section listed `recommended` inside a visibility array
alongside `admin` and `shared`, which conflated a surface with a principal and made "visible on the
recommended row to shared users only" inexpressible. Home and recommended are independently
controlled surfaces; `owner`, `sharedAll`, and `everyone` are principals. At launch the GUI writes
only whole-audience principals; per-user targeting later widens the set without a schema migration.
See §19.13.3.

**`sortTitleConsent`** must be true before Afisharr will modify the sort title of an adopted
collection. Zone placement is achieved by writing sort-title prefixes, which mutates a user's own
object; consent is required, the original value is recorded, and demotion restores it exactly.

**Positions need not be dense or unique.** Ties break deterministically by ULID, which is what stops
two same-positioned participants from swapping places on alternate runs.

### 12.8 PackManifest spec and install-time variables

```json
"spec": {
  "namespace": "afisharr.media-info",
  "version": "1.2.0",
  "requiresApp": ">=0.3",
  "requiresRegistry": ">=1",
  "requiresFields": ["media.videoResolution", "media.doviProfile"],
  "definitions": ["overlays/resolution.json", "overlays/audio-codec.json"],
  "assets": ["assets/dolby-vision.svg", "fonts/inter-bold.ttf"],
  "variables": {
    "corner": { "type": "string", "enum": ["topLeft", "topRight"], "default": "topRight" },
    "scale":  { "type": "number", "minimum": 0.5, "maximum": 2, "default": 1 }
  },
  "expand": [
    { "template": "overlays/resolution.json", "over": "media.videoResolution" }
  ],
  "defaultsEnabled": false
}
```

A pack is a directory or zip: manifest, definition files, assets. Assets are referenced as
`namespace/asset-slug` and content-hashed on install. Installing never enables anything unless
`defaultsEnabled` is set and the user consents in the install dialog.

`requiresFields` is resolved against the field registry at install time. A pack needing an
unconfigured integration installs in a **degraded** state the GUI shows explicitly, rather than
installing cleanly and then appearing broken.

I author first-party packs in my own schema, versioned with the app, and updated on my own
schedule. My launch set is media-info overlay packs, content-rating packs, the lifecycle status
pack, and starter collection packs. Community packs are definition bundles plus assets with a
manifest, installed from file, URL, or repository; a registry may follow.

**Variables are resolved by the installer, never by the engine.**

`variables` declares named parameters as JSON Schema — same shape as source parameters — so the
install dialog is generated the same way and needs no pack-specific frontend work. `expand`
declares that one template document materializes once per member of a registry enumeration, which
is what turns a single parameterized overlay into one concrete definition per resolution, codec, or
content rating. A media-info pack ships one parameterized overlay rather than thirty
near-identical ones, while what lands in the database is still plain, validatable, diffable data
with no substitution syntax in it.

**The installer substitutes; storage receives concrete documents.** A row in `definitions` never
contains a variable reference, a conditional, or an unexpanded template. This is not a style
preference — it is what keeps two properties that nothing else protects:

- **The pure-data-no-logic principle (§12.1.1) holds.** A stored document with an unresolved
  variable cannot be validated against the field registry until something resolves it, so
  validation-at-save stops being a guarantee and becomes a guess.
- **The diffable-and-round-trippable principle (§12.1.4) holds.** Two documents differing only in
  an unresolved variable diff as identical, so history, export, and the fork-vs-upstream comparison
  (see §19.10) all silently lose information.

The resolved values are stored against the installed pack so an upgrade can re-materialize. Without
them, a pack update either loses the operator's choices or cannot regenerate the definitions the
operator has not forked. Decided as D-044; schema in §19.10; tested by I-DEF-8.

**What this deliberately does not become.** Variables substitute into values. They do not nest, do
not reference each other, and do not select which document is emitted by a predicate. The
unrestricted version of that is a template language, which is the same thing the pure-data-no-logic
principle forbids arriving by a different route — the argument CR-1 already made for computed
fields, applied to packs.

### 12.9 Storage and API

- **SQLite.** `definitions(id, kind, handle, schema_version, registry_version, body_json,
  updated_at, origin_pack, …)` with the canonical JSON body as the single source of truth; hot
  columns are extracted for indexing only.
- **API.** Definitions CRUD is generic — `/api/definitions/{kind}` — with per-kind validation from
  registry-driven JSON Schemas. The generated TypeScript client gives the SPA typed documents.
- **Every save validates** the envelope, the kind schema, source parameters, expression trees
  against the field registry, formatter pipelines, ordering compatibility with source capabilities,
  seed presence, reference integrity, cron expressions, and regex compilation. Errors are structured
  — JSON pointer plus expected versus actual — so the GUI highlights the offending control.
- **History.** The last N versions of each body are retained: cheap undo, and forensics for "what
  changed and broke my collection."

### 12.10 Worked examples

- Renaming a definition never breaks references — references use ULIDs; handles are display and
  export sugar.
- Deleting a definition with inbound references requires an explicit cascade choice in the GUI.
- Pack upgrade replaces pack-origin definitions, leaves user forks untouched, and reports which
  forks are now behind upstream.
- Exporting a user definition that references pack assets bundles the reference, not the asset;
  importing without the pack yields a missing-dependency prompt.
- A definition that references a server-discovered field unavailable on the current server falls
  back to local evaluation and is flagged — never silently dropped.
- A definition targeting multiple libraries produces one collection per library, and removing a
  library from the set removes exactly that collection.
## 13. The registries

Three registries and one operator set form the contract between the engine, the definition layer,
the GUI, and packs.

- **Field registry** (§13.2) — every value a condition can test or a template can render.
- **Operator set** (§13.3) — every comparison a condition can perform, and on what types.
- **Formatter registry** (§13.5) — every transformation a template can apply to a value.
- **Source registry** (§13.6) — every source type, its parameters, and its behavioural guarantees.

All four are **closed sets under version control**. I let nothing reference a key that is not
registered; validation rejects it at save time, not at render time.

### 13.1 Why these are registries and not code

A registry entry is data describing a capability. I make that single choice because it buys four
things at once:

1. **The GUI is generated, not written.** A condition builder that knows a field's type, cardinality,
   and legal operators renders the right control automatically. Adding a field adds a form control
   with no frontend work.
2. **Validation is total.** `resolution gte "PG-13"` is rejected when the definition is saved,
   because the registry says `resolution` is an enumeration and `gte` is numeric-only. There is no
   class of "the pack renders nothing and nobody knows why" bug.
3. **Packs are checkable.** A pack declares the fields it requires; the installer can tell the user
   "this pack needs Rotten Tomatoes ratings, which you haven't configured" *before* installing.
4. **Evolution is governed.** Adding, deprecating, and removing capabilities follow one recorded
   process rather than ad-hoc edits scattered across the engine.

### 13.2 Field registry

#### 13.2.1 Entry shape

```json
{
  "key": "media.videoResolution",
  "type": "enum",
  "values": ["sd", "480", "720", "1080", "4k"],
  "cardinality": "single",
  "scope": "item",
  "availability": "always",
  "provenance": "plex.mediaStream",
  "nullable": true,
  "ops": ["eq", "neq", "in", "noneOf", "exists"],
  "formatters": ["upper", "lower", "title"],
  "label": "Video resolution",
  "description": "Resolution class reported by Plex for the selected media part.",
  "since": 1
}
```

| Attribute | Purpose |
| --- | --- |
| `key` | Namespaced, stable, never reused after removal |
| `type` | `string`, `number`, `integer`, `boolean`, `date`, `duration`, `enum` |
| `values` | Required for `enum` — drives GUI dropdowns and rejects typos |
| `cardinality` | `single` or `multi` — determines which operators and formatters apply |
| `scope` | `item`, `collection`, or a child scope (`seasons`, `episodes`) |
| `availability` | `always`, `integration`, or `derived` (§13.2.3) |
| `provenance` | Where the value comes from, for debugging and the doctor page |
| `nullable` | Whether absence is legal |
| `ops` | Legal operators — a subset of §13.3, narrowed by type and cardinality |
| `formatters` | Legal formatters — a subset of §13.5 |
| `label`, `description` | GUI text, translatable |
| `since` / `deprecated` | Registry version bounds |

#### 13.2.2 Null, missing, and the rendering rule

Three distinct situations that must never be conflated:

| Situation | Meaning | Condition behaviour | Render behaviour |
| --- | --- | --- | --- |
| **Present** | A value exists | Compare normally | Render it |
| **Null** | Known to have no value (a film with no HDR track) | `exists` is false; all other operators are false | Element is **skipped** |
| **Unavailable** | Could not be determined (integration not configured, fetch failed) | `exists` is false; all other operators are false; the evaluation is marked degraded | Element is **skipped**, and the item is flagged in the render audit |

The rendering rule: **an element whose bound variable does not resolve is skipped, never rendered
blank.** An empty badge on a poster is worse than no badge, and a badge reading `undefined` is worse
than both. Skipping is also what makes conditional packs work — an HDR badge with `when: hdr exists`
simply doesn't appear on SDR titles.

Distinguishing null from unavailable is what lets the doctor page say "412 items have no Rotten
Tomatoes score because RT is not configured" instead of silently rendering nothing.

#### 13.2.3 Availability classes

| Class | Meaning | Example |
| --- | --- | --- |
| `always` | Derivable from Plex alone | `media.videoResolution`, `item.title` |
| `integration` | Requires a configured external service | `ratings.rtCritics`, `ratings.imdb` |
| `derived` | Computed by an Afisharr subsystem | `lifecycle.status`, `item.daysSinceAdded` |

Packs declare `requiresFields`; the installer resolves each against availability and warns before
install. A pack that needs an unconfigured integration installs in a **degraded** state that the
GUI shows explicitly, rather than appearing broken.

#### 13.2.4 Two layers: static core and server-discovered

The field registry is **not** a single static catalog. Plex serves its own filter metadata per
library, and ignoring that would mean maintaining a hand-written mirror of a vocabulary that changes
with every Plex release and differs per library type.

| Layer | Source | Lifetime | Contents |
| --- | --- | --- | --- |
| **Static core** | Compiled into Afisharr, versioned with the app | Fixed per release | `lifecycle.*`, `ratings.*`, `media.*`, derived `item.*` values, `collection.*` |
| **Server-discovered** | Fetched per library from Plex, cached with the library cache | Refreshed on library scan and on Plex version change | Plex-native filter fields, their types and subtypes, the operators legal for each type, and enumerated choice lists |

Plex exposes, per library and per library type (movie, show, season, episode, collection):

- a **filtering type** per library type, each carrying its fields, filters, and sorts;
- **fields** with `key`, `title`, `type`, and `subType` (decade, rating, …);
- **field types**, each carrying the **list of operators legal for that type**;
- **filter choices** — the enumerated values for tag fields (genres, studios, labels…), each with a
  fast key that lists matching items directly.

I take three consequences from this, all of them improvements over a hardcoded catalog:

1. **The smart-filter builder is generated, not maintained.** It matches the user's actual Plex
   version and their actual library types, and a genre dropdown is populated from the server's own
   choice list rather than from a list Afisharr has to keep current.
2. **Plex-native compilation becomes decidable by lookup.** A filter tree compiles to a Plex
   smart-filter query when every predicate is Plex-native, and evaluates locally otherwise. With the
   discovered layer, "Plex-native" means precisely: the field appears in the discovered layer for
   this library and libtype, **and** the requested operator appears in that field type's operator
   list. No allowlist to maintain, no drift.
3. **Version differences degrade gracefully.** A field that a newer Plex adds simply appears; one an
   older Plex lacks simply doesn't, and any definition referencing it falls back to local
   evaluation with a recorded reason rather than failing.

**Precedence and collision.** Static core wins on key collision, and discovered fields are
namespaced `plex.*` to make collisions rare by construction. A definition referencing a discovered
field records the library it was authored against; if that field is later absent, the predicate
falls back to local evaluation and the definition is flagged, never silently dropped.

**Compile target.** A compiled smart filter is a library search URL —
`/library/sections/{key}/all?` plus validated filter arguments, with `sort`, `type`, and `limit`.
Operators are expressed as suffixes on the field key (`!=`, `>>=`, `<<=`, and an `&=` form for
conjunctive multi-value matching); string fields take a doubled `=` form for exact matching. The
exact operator key set per field type comes from the server, which is the point.

**Not to be confused with client-side filtering.** Reference client libraries also ship an
in-process operator set (`exact`, `icontains`, `startswith`, `regex`, …) used to filter objects
already fetched. That is a different mechanism from the server-side filter vocabulary and must not
be conflated with it — one runs in Plex, the other after the data is already in memory. Afisharr's
local evaluation path is its own expression engine, not a mirror of either.

#### 13.2.5 Namespaces and catalog (static core)

Field keys are namespaced by origin. The catalog below is the Tier 0 static set, verified against
the fields Plex actually exposes on media, video, and item objects.

**`item.*` — library item metadata** (availability `always`)

Common: `title`, `sortTitle`, `originalTitle`, `summary`, `tagline`, `year`, `contentRating`,
`studio`, `rating`, `audienceRating`, `userRating`, `duration` (duration), `originallyAvailableAt`
(date), `addedAt` (date), `updatedAt` (date), `lastViewedAt` (date), `lastRatedAt` (date),
`viewCount` (integer), `viewOffset` (integer), `guid`, `slug`, `editionTitle`, `mediaType` (enum),
`genre` (multi), `country` (multi), `director` (multi), `writer` (multi), `producer` (multi),
`actor` (multi), `label` (multi), `collection` (multi), `theme` (nullable).

Derived: `daysSinceAdded`, `daysSinceLastPlayed`, `daysSinceRelease`, `isPlaceholder`,
`isWatched`, `versionCount`.

Show-specific: `network`, `childCount`, `seasonCount`, `leafCount`, `viewedLeafCount`,
`episodeSort`, `showOrdering`, `flattenSeasons`, `audioLanguage`, `subtitleLanguage`.

**`media.*` — stream and file facts** (availability `always`, nullable — an item may have no media)

Container level: `videoResolution` (enum), `videoCodec`, `videoProfile`, `videoFrameRate`,
`audioCodec`, `audioProfile`, `audioChannels`, `aspectRatio`, `bitrate`, `width`, `height`,
`container`, `duration`, `optimizedForStreaming` (boolean), `has64bitOffsets` (boolean).

Video stream: `bitDepth`, `chromaSubsampling`, `chromaLocation`, `colorPrimaries`, `colorRange`,
`colorSpace`, `colorTrc`, `codedWidth`, `codedHeight`, `frameRateMode`, `level`, `profile`,
`pixelFormat`, `pixelAspectRatio`, `refFrames`, `scanType`, `anamorphic` (boolean),
`hasScalingMatrix` (boolean).

Dolby Vision — a distinct attribute family, not a single boolean: `doviPresent` (boolean),
`doviProfile`, `doviLevel`, `doviVersion`, `doviBlPresent`, `doviElPresent`, `doviRpuPresent`,
`doviBlCompatId`. Overlay packs that badge DV by profile need these; a single `dolbyVision` boolean
cannot express profile 5 versus profile 8.1 compatibility, which is exactly the distinction users
want badged.

Audio stream: `audioChannelLayout`, `audioBitDepth`, `audioBitrateMode`, `audioSamplingRate`,
`audioLanguage`, `audioLanguages` (multi), `audioLanguageCodes` (multi), `audioFormat`,
`audioTitle`, `visualImpaired` (boolean), `hearingImpaired` (boolean), `forced` (boolean).

Subtitle stream: `subtitleLanguages` (multi), `subtitleLanguageCodes` (multi), `subtitleFormat`,
`subtitleForced` (boolean), `subtitleHearingImpaired` (boolean), `hasSubtitles` (boolean),
`subtitleProvider`.

Part level: `filePath`, `fileSize`, `partContainer`, `accessible` (boolean), `exists` (boolean),
`deepAnalysisVersion`, `videoProfilePart`, `audioProfilePart`.

`accessible` and `exists` deserve attention: they are Plex's own report that a file is missing or
unreadable, which makes them the honest basis for a "broken media" overlay and a doctor-page check —
better than inferring absence from a failed playback.

**`plex.*` — server-discovered filter fields** (availability `always`, per library)

Populated at runtime from the server. I do not enumerate them here by design: enumerating them would
recreate the maintenance burden the discovered layer exists to remove.

**`ratings.*`** (availability `integration`)

`imdb` (number), `rtCritics` (number), `rtAudience` (number), `rtCertifiedFresh` (boolean),
`rtVerifiedHot` (boolean), `tmdb` (number), `trakt` (number).

**`lifecycle.*`** (availability `derived`) — full definitions in *the lifecycle model*

`status` (enum), `phase` (enum), `acquisition` (enum), `presence` (enum), `production` (enum),
`releaseDate` (date), `releaseDateBasis` (enum), `daysUntilRelease` (integer),
`daysSinceRelease` (integer), `isStale` (boolean), `isPlaceholder` (boolean),
`seasonNumber` (integer, nullable — null on a whole-title subject; see the lifecycle model).

**`show.*` / `season.*` / `episode.*`** — scoped fields for the quantifiers in §13.4

`show.status` (enum), `show.seasonCount`, `show.episodeCount`, `show.viewedEpisodeCount`,
`show.lastAired` (date), `show.nextAirs` (date); `season.number`, `season.episodeCount`,
`season.viewedEpisodeCount`, `season.airDate`, `season.year`; `episode.number`,
`episode.seasonNumber`, `episode.airDate`, `episode.watched` (boolean), `episode.hasMedia`
(boolean), `episode.duration`, `episode.rating`.

**`collection.*`** — available when rendering collection posters, not item overlays

`name`, `itemCount` (integer), `sourceType` (enum), `library`, `lastSyncedAt` (date),
`isSmart` (boolean), `mode` (enum), `sortMode` (enum).

**Scope rule:** a condition may only reference fields whose scope matches its position. An item
overlay cannot test `collection.itemCount`; a collection filter cannot test `episode.watched`
except inside a scoped quantifier. Validation enforces this from the registry — no special cases in
the engine.

#### 13.2.6 Enumerations are declared, never free text

Every `enum` field declares its complete value set. This is what makes `resolution in ["4k","1080"]`
a dropdown multi-select instead of a text box, and what turns a typo into a save-time error rather
than a badge that never appears. Provider vocabularies that differ (TV status strings across
metadata providers) are normalized into Afisharr's enum by a translation table stored **in the
registry entry**, so the mapping is inspectable rather than buried in a client.

### 13.3 Operator set

Closed set. Each operator declares which types and cardinalities it accepts; the intersection with a
field's declared `ops` is what the GUI offers.

| Group | Operators | Accepts |
| --- | --- | --- |
| Equality / sets | `eq`, `neq`, `in`, `anyOf`, `allOf`, `noneOf`, `exists` | `anyOf`/`allOf`/`noneOf` require `cardinality: multi`; `exists` accepts everything |
| Strings | `contains`, `startsWith`, `endsWith`, `matchesGlob`, `matchesRegex` | `string`, `enum` |
| Numbers | `gt`, `gte`, `lt`, `lte`, `between` | `number`, `integer`, `duration` |
| Counts | `countGt`, `countGte`, `countLt`, `countLte` | `cardinality: multi` only |
| Dates | `before`, `after`, `between`, `withinLast`, `olderThan` | `date` |

**Boolean fields** take `eq` and `exists` only. There is no `isTrue` — `{"field": "media.hdr", "op":
"eq", "value": true}` is unambiguous and needs no synonym.

**Regex** compiles at save time with a pattern-size cap and is rejected if it fails to compile. The
linear-time guarantee of the regex engine makes catastrophic backtracking impossible by
construction, so the safety concern is pattern size and legibility, not runtime.

**Date operators** evaluate against the pass's evaluation clock, not `now()` at the moment each
predicate runs. A pass that straddles midnight must not disagree with itself. Timezone is an
instance setting; `withinLast`/`olderThan` take a duration and are day-aligned in the instance
timezone.

### 13.4 Scoped quantifiers

A parent can be filtered by facts about its children:

```json
{ "scope": "episodes", "quantifier": "none", "tree": { "field": "episode.hasMedia", "op": "eq", "value": true } }
```

| Attribute | Values |
| --- | --- |
| `scope` | `seasons`, `episodes` |
| `quantifier` | `any`, `all`, `none`, `{ "countGte": N }`, `{ "countLt": N }` |

**Empty-child semantics must be stated, because they are where these go wrong.** For a show with
zero episodes: `any` is false, `none` is true, `all` is **true** (vacuous truth), `countGte: 1` is
false. Vacuous truth surprises people — a rule "all episodes watched" matches a show with no
episodes — so the GUI warns when `all` is used without a companion `countGte` guard, and pack
authoring guidelines say to pair them.

Nesting depth is capped (seasons inside episodes is meaningless; two levels is the maximum). Cost is
bounded — a quantifier over episodes on a large library is the most expensive thing the filter
engine can do, so the planner requires the child data to be batch-loadable and refuses to evaluate
per-item.

### 13.5 Formatter registry

Closed set. Formatters are **pure engine functions** — no expressions, no chaining beyond a declared
pipeline, no user-supplied code.

| Formatter | Applies to | Example |
| --- | --- | --- |
| `upper`, `lower`, `title` | string, enum | `4k` → `4K` |
| `decimals:N` | number | `8.732` → `8.7` |
| `percent` | number (0–10 scale) | `8.7` → `87%` |
| `fraction` | number | `8.7` → `8.7/10` |
| `hours`, `minutes` | duration | `142` → `2h`, `22m` |
| `words`, `wordsUpper`, `wordsLower` | integer | `3` → `three` |
| `pad:N` | string, number | `7` → `07` |
| `date:FMT` | date | strftime subset |
| `join:SEP` | multi | `["EN","DE"]` → `EN·DE` |
| `first:N`, `count` | multi | first N values; cardinality |

**Locale.** `words`, `title`, and `date` are locale-dependent. Each takes the instance locale by
default and accepts an explicit override, so a pack can pin a locale where the design demands it
(an English-language badge pack) while a general pack follows the user's setting. This is the
concrete reason the i18n framework belongs in the first milestone rather than being retrofitted.

**Pipelines** are a declared sequence with a maximum length, each stage type-checked against the
previous stage's output at save time. `{resolution|upper|pad:4}` is validated when saved, not
discovered broken at render.

**Determinism obligation.** A formatter is a pure function of (value, locale, arguments). No clock
reads, no I/O, no randomness. This is what makes the render cache key (defined alongside the caching
and invalidation rules) sound: a formatter that varied independently of its inputs would leave the
key identifying nothing. Enforced by I-RENDER-5.

### 13.6 Source registry

#### 13.6.1 Entry shape

```json
{
  "type": "tautulli.mostWatched",
  "tier": "api",
  "params": { "$schema": "…", "type": "object", "properties": { … } },
  "auth": ["tautulli"],
  "idSpace": ["plex", "tmdb"],
  "endpoints": [
    {
      "rung": "structured",
      "parserVersion": 1,
      "capabilities": {
        "ordered": true,
        "paginated": false,
        "affirmativeEmpty": true,
        "deterministic": false,
        "supportsLimit": true
      }
    }
  ],
  "rateLimit": { "requests": 30, "per": "minute" },
  "cache": { "ttl": "10m" },
  "health": { "breakerAfter": 5, "cooldown": "15m" }
}
```

**`endpoints` is an ordered ladder, tried top to bottom.** `rung` is one of `structured` (a
machine-readable interface), `embedded` (a structured payload inside a page), or `markup`, matching
the three rungs of the endpoint ladder (§14.1). Most sources declare exactly one rung; the field is
a list so that the sources with a fallback declare the fallback's honest capabilities rather than
inheriting the primary's. See §13.6.2.

**`parserVersion`** is per rung and is a component of the HTTP cache key (the data model's caching
section). Bumping it retires every cached response that the previous parser shaped, which is the
only thing that makes a parser fix reach a running instance before the TTL expires (D-043).

**`volatileParams`**, where present, names the parameters this source's rungs draw from the
out-of-band feed (D-041, described further at §14.4) — a query hash or an endpoint path the
provider rotates. Each entry declares a name, a type, and a syntactic constraint. A fetched value
failing its constraint is rejected and the last-known-good value is kept.

#### 13.6.2 The capability flags that carry safety weight

**`affirmativeEmpty`** is the most important field in this registry. A source returning zero items
is treated as failure *unless the source affirmatively reports an empty list*. That distinction
cannot be made by the engine — only the source adapter knows whether its API distinguishes "no
results" from "request failed" or "challenge page returned." So the adapter declares it:

- `affirmativeEmpty: true` — a zero-item response is trustworthy and means empty.
- `affirmativeEmpty: false` — a zero-item response is **always** treated as failure, freezing the
  source's contribution to last-known-good.

Scraped-tier sources are `false` by default and may only be promoted with a documented reason.

**Capabilities belong to the rung that answered.** A source with a fallback declares capabilities
per `endpoints` entry, and the engine applies the flags of the rung that produced the result — not
the source's best rung. This is D-040, and it exists because a single per-source flag is wrong in
exactly the case the safeguard is for: a source whose structured endpoint returns a typed
"not found" and whose embedded-payload fallback returns an empty page would carry
`affirmativeEmpty: true`, and the first time the fallback ran, an empty page would empty a
collection. A rung that cannot distinguish "no results" from "the page changed" declares `false`,
whatever the rung above it can do. Tested by I-SRC-8.

The health record is per source, not per rung: falling through to a lower rung is itself a
degradation the doctor page reports, because a source silently running on its fallback for weeks is
how a fixable break becomes a permanent one.

**`ordered`** declares whether the source's sequence is meaningful. `order: { by: "sourcePosition" }`
against an unordered source is a save-time validation error, not a silently arbitrary result.

**`deterministic`** declares whether identical parameters yield identical results within a cache
window. Non-deterministic sources (anything random, anything time-windowed) require a seed field so
a plan is reproducible.

**`idSpace`** declares which canonical identifiers the source returns, which drives the resolution
strategy and lets the engine warn about sources that need lossy matching.

#### 13.6.3 Parameters are JSON Schema, and that is the whole GUI

Each source publishes a JSON Schema for its parameters. That schema validates definitions **and**
generates the editor form. Adding a source never touches the collection schema and never requires
frontend work, which is the reason the source count can grow without the GUI rotting.

Schemas carry annotations beyond types: `title` and `description` for labels, `enum` with
`x-labels` for dropdowns, `x-control` for pickers that need a live lookup (an *arr tag selector must
populate from the configured instance), and `x-dependsOn` for fields that only apply given another
field's value.

#### 13.6.4 Tier 0 catalog

Grouped by tier. Every entry needs its parameter schema written out during implementation; the
column here records the parameters each source is known to need.

**API tier**

| Type | Parameters | ordered | affirmativeEmpty |
| --- | --- | --- | --- |
| `tmdb.chart` | chart (popular/topRated/trending), window, mediaType, limit | yes | yes |
| `tmdb.franchise` | franchise id or seed title, includeParts | yes | yes |
| `tmdb.list` | list url or id | yes | yes |
| `tmdb.discover` | filter groups (nested, and/or), sortBy, mediaType | yes | yes |
| `tmdb.watchProvider` | providerId, region, mediaType | yes | yes |
| `tmdb.person` | role (actor/director), minItems, separator options | yes | yes |
| `tmdb.random` | pool params, seed | no | yes |
| `trakt.chart` | chart (trending/popular/watched), period, mediaType | yes | yes |
| `trakt.list` | list url | yes | yes |
| `trakt.recommendations` | mediaType, limit | yes | yes |
| `mdblist.list` | list url | yes | yes |
| `anilist.*` | chart or list url | yes | yes |
| `mal.*` | chart or list | yes | yes |
| `overseerr.requests` | scope (global/perUser), status | yes | yes |
| `tautulli.stats` | metric (popular/watched) × unit (plays/duration), days, minPlays | yes | yes |
| `radarr.tag` / `sonarr.tag` | instanceId, tagId | no | yes |
| `plex.library` | mode (recentlyAdded / recentlyReleased / recentlyReleasedEpisodes), limit | yes | yes |
| `lifecycle.comingSoon` | window, monitored scope, instance + tag filters | yes | yes |
| `imdb.chart` | chart id, limit | yes | per rung — see below |
| `imdb.list` | list url or id, limit | yes | per rung — see below |

**IMDb sits in this tier and declares two rungs** (CR-3, D-040). Its structured endpoint returns
cursor-paginated results with typed error codes — a "not found" and a "forbidden" are distinct and
neither is an empty list — so that rung declares `affirmativeEmpty: true`. Its `embedded` fallback
reads the hydration payload out of the page and cannot make that distinction, so that rung declares
`affirmativeEmpty: false`. The structured rung's query is authenticated by a hash the provider
rotates, which is declared in `volatileParams` and supplied by the out-of-band feed (§14.4).

Scope is unchanged: charts and custom lists only, per the source-scope decision record. IMDb
*ratings* are not a source at all — they are a field with availability class `integration`
(§13.2.3), supplied by the bulk dataset import (§14.5), on a different cadence and through a
different transport. The two share a provider name and nothing else.

**Scraped tier** — all `affirmativeEmpty: false`, all behind challenge detection and circuit
breakers, none introduced before the safety rails exist.

| Type | Parameters | ordered |
| --- | --- | --- |
| `letterboxd.list` | list url, random | yes |
| `flixpatrol.networks` | country, platform | yes |
| `flixpatrol.originals` | platform | yes |

Each of these is audited against the endpoint ladder (§14.1) before its parser is written. A source
that turns out to expose a structured or embedded rung moves up, and the move is a registry edit
rather than a rewrite — which is the point of declaring rungs at all.

**Smart-collection constraint.** A definition that compiles to a Plex smart collection forfeits
`sourcePosition` ordering and manual item ordering, and validation rejects that combination at save
time. Where a definition requires both Plex-native filtering and a custom order, the engine builds a
regular collection and evaluates locally — correctness over server-side elegance.

**Meta sources** — compose other sources rather than fetching

| Type | Notes |
| --- | --- |
| `multi` | N child sources with per-source priority and caps; combine modes map to merge strategy plus order |
| `hubReplacement` | Shadows a native Plex hub with placeholder items excluded (see the lifecycle model's hub-management section) |

**Scope decisions behind the Tier 0 catalog.** The catalog above was checked source by source
against the working set already proven out before Afisharr, and every builder in that set became a
distinct Tier 0 source type (a "TMDB source" is really eight: charts, franchise expansion, custom
list, random pick, Discover-style advanced filters, watch-provider collections, person collections,
and OAuth-backed personal lists via Trakt). Two adjustments were made on the way in:

- **Maintainerr integration is cut.** Reading another collection manager's own collections/rules as
  a source was judged out of scope — bloat relative to what Afisharr's own registry already covers.
- **The "filtered hub" idea is not a source type.** It is redesigned as *hub replacement*: hiding a
  Plex native hub (recently added / recently released / recently released episodes) and substituting
  a clean row with placeholder items excluded, because placeholders were polluting Plex's native
  rows. This is the `hubReplacement` meta source above.
- **RT and IMDb ratings are confirmed as fields, not sources** — they are filter and overlay inputs,
  consistent with `ratings.*`'s `integration` availability class (§13.2.3, §13.2.5).
- **TVDB lookups and a GitHub release/update check are held at Tier 1**, opt-in, and are not part of
  the Tier 0 registry above; they may be added as registry entries in a later minor version under the
  same evolution rules (§13.8).

**Collection-level combine, filter, and order capabilities.** These sit alongside individual source
parameters and are Tier 0 in full:

| Capability | What it does |
| --- | --- |
| Combine modes | `interleaved`, `list_order`, `randomised`, `cycle_lists` — all four map to a merge strategy plus order |
| Per-source priority | Ordering weight per source in a multi-source collection |
| Item cap | Maximum items per collection |
| Position cap | Only consider list positions 1–X from each source |
| Sort order | Collection item ordering options |
| Genre / country / language filters | Include **or** exclude mode per axis |
| Keyword filters | Include/exclude by keyword |
| Minimum year | Release-year floor |
| Minimum IMDb rating | Rating floor |
| Minimum RT critic / audience | Two independent floors |
| Global exclusions | Never-include list by canonical ID |
| Mutual exclusion | Item in collection A is excluded from B |
| Unwatched-only smart collections | Server-side Plex smart collection filtered to unwatched, with its own sort |
| Separator collections (Tier 1) | Inserts a visual divider collection in the library A–Z |
| Time restrictions | Seasonal date ranges (day-month) plus a weekly day mask; inactive behaviour is configurable as hide or remove |

### 13.7 Validation

Every definition save runs, in order — and stops at the first failure with a pointer to the exact
node:

1. Envelope: `kind`, `schemaVersion`, id, handle format.
2. Kind schema.
3. Source parameters against each source's JSON Schema.
4. Every field key exists in the registry and is in scope for its position.
5. Every operator is legal for its field's type and cardinality.
6. Every literal value matches its field's type, and enum values are members of the declared set.
7. Every formatter is legal for its input type; pipelines type-check stage by stage.
8. Ordering mode is compatible with source capabilities (`sourcePosition` requires `ordered`).
9. Non-deterministic sources carry seeds.
10. References resolve; cron expressions parse with the scheduler's own parser.
11. Regex patterns compile within the size cap.

Errors are structured — JSON pointer, registry key, expected versus actual — so the GUI highlights
the offending control rather than showing a paragraph.

**Pack installation runs the same eleven steps**, on each materialized document, after variable
substitution and expansion. There is no relaxed path for pack-origin definitions. A pack whose
expansion produces one invalid document fails the install and writes nothing — a partial install is
a pack the operator cannot reason about, and the failure is reported against the source template and
the variable values that produced it, not against the generated document the author never wrote.

### 13.8 Versioning and evolution

The registries are versioned as a unit. `registryVersion` is recorded on every definition that was
validated against it.

| Change | Allowed | Requires |
| --- | --- | --- |
| Add a field, operator, formatter, source | Yes | Minor bump |
| Add a value to an enum | Yes | Minor bump; packs must tolerate unknown values |
| Widen a field's legal operators | Yes | Minor bump |
| Narrow legal operators, remove an enum value, change a type | No, directly | Deprecate, then migrate |
| Remove a field or source | No, directly | Deprecation period, then a definition migration |
| Reuse a removed key | **Never** | — |

Deprecated entries stay in the registry with `deprecated: <version>` and a `replacedBy` pointer.
The GUI hides them from new definitions but renders existing ones with a warning, and the definition
migration path rewrites them on load — the same mechanism definition schema versioning already
requires.

**Enum tolerance is a pack-authoring rule:** a pack that maps enum values to icons must declare a
fallback, because a future release may add a value the pack has never seen. Mapping tables without a
fallback fail validation.

---

## 14. External source policy

**API first.** TMDB, Trakt, TVDB, IMDb, MDBList, AniList, MyAnimeList, the *arr suite, Overseerr,
Tautulli, and Plex are the reliability core.

**No headless browser.** I replace it with two independent ladders. They answer different questions
and I specify them separately because conflating them is what turns a documented endpoint into a
scraper.

### 14.1 The endpoint ladder

What I ask for, in descending order of trust. Every source is audited for the highest rung it can
reach before any parser is written.

1. **A structured endpoint.** A machine-readable interface with typed responses and typed errors.
   Exact ordering, no markup, and — decisively — an error vocabulary that distinguishes "this list is
   empty" from "this list is private" from "this request failed." This is the rung I prefer for
   every source, because it is the only rung on which `affirmativeEmpty: true` can be trusted without
   qualification.
2. **A structured payload embedded in a page.** Many sites hydrate their own interface from a JSON
   blob inside the document. Reading that blob is not markup parsing: it survives redesigns that
   break every selector, and it yields the same typed values the site's own client uses. It is a
   weaker rung than a structured endpoint mainly because its error vocabulary is thinner — a missing
   or reshaped blob often looks the same as an empty one.
3. **Markup.** Last, and only where the first two do not exist. The fragile rung: layout changes
   break it silently, and it carries no reliable signal for distinguishing empty from broken, which
   is why it is `affirmativeEmpty: false` by default (§13.6.2).

### 14.2 The transport ladder

How I ask. Plain client first; detect challenge pages by response validation; retry through a
browser-fingerprint client only when the plain client is blocked. The specific crate is chosen at
implementation time based on maintenance health. The transport ladder and the endpoint ladder answer
different questions — one is about which interface a source exposes, the other is about how a
request reaches it — and I keep them specified separately so neither gets treated as a stand-in for
the other.

### 14.3 Per-rung capability flags

**Capability flags follow the rung, not the source.** A source declares its ladder, and each rung
carries its own `affirmativeEmpty`, `ordered`, and `deterministic`. The engine applies the flags of
the rung that actually answered.

A source whose top rung reports emptiness affirmatively and whose fallback cannot must never carry a
single flag. A single per-source flag is wrong in exactly the case the safeguard exists for: it would
be correct precisely when the fallback was not needed, and wrong the moment it was needed. Concretely
— a source whose structured endpoint returns a typed "not found" (so `affirmativeEmpty: true` is
safe) but whose embedded-payload fallback returns an indistinguishable empty page on failure (so
`affirmativeEmpty: false` is required) would, under a single source-level flag, empty a user's
collection the first time the fallback ran. Declaring the flag per rung instead of per source is what
prevents that: the fallback rung declares its own honest, weaker capability regardless of what the
primary rung can do. Decided as D-040; tested by I-SRC-8. This is also why the source registry's
`endpoints` field (§13.6.1) is a list rather than a single object.

### 14.4 Out-of-band volatile parameters

Where an endpoint authenticates a query by a hash or a path that the provider rotates on its own
schedule, that value is not compiled in. It arrives through a signed feed the running instance
fetches, constrained by a registry the binary ships: the feed can change a declared parameter's
value and can do nothing else. A rotated hash is then a one-file repair rather than a release to
every installed copy. The source registry's `volatileParams` field (§13.6.1) names exactly which
parameters a given source's rungs draw from this feed, each with a name, a type, and a syntactic
constraint; a fetched value failing its constraint is rejected and the last-known-good value is
kept. Decided as D-041; tested by I-SEC-7.

### 14.5 Bulk datasets

Where a provider publishes its dataset as a periodic file — ratings and genres for every title,
refreshed daily — I import the file, stage it in SQLite, and swap it atomically. A partial import
leaves the previous dataset in place and reports. This is the mechanism behind the `ratings.*`
field family's `integration` availability (§13.2.3, §13.2.5): those values are not fetched per item
on demand, they are joined against the staged dataset. Decided as D-042; tested by I-DATA-13.

### 14.6 Shared source obligations

Every external source sits behind the same interface, regardless of tier or rung:

- **Mandatory response validation** — a challenge page must never reach the parser and be counted as
  zero items.
- **Retry with exponential backoff and jitter.**
- **Hard per-request timeouts.**
- **Log deduplication.**
- **A per-source circuit breaker**, surfaced in the UI.
- **Degrade to last-known-good** — every source degrades without emptying user collections.

These obligations are what make the capability flags in §14.3 trustworthy in practice: a flag that
says "this rung reports emptiness affirmatively" only means something if a challenge page or a
malformed response was already filtered out before the emptiness check ran.
## 15. Placement and ordering

Placement is where I write the most, own the least, and can do the most visible damage. I position
three different kinds of thing in a shared ordering space that belongs to Plex, through an API with
no transactions, no absolute positions, and a finite hidden precision budget. Owning the entire home
screen promoted this from a feature to the highest-risk subsystem in the product.

### 15.1 What is being placed

| Participant | Origin | Can be removed from the ordering space? | Sort-title writable? |
| --- | --- | --- | --- |
| **Managed collection** | Created by me | Yes — unpromote and re-promote freely | Yes |
| **Adopted collection** | Created by the user in Plex, adopted by me | Yes, but it is the user's object | **Only with consent** (§15.6) |
| **Native hub** | Plex's own rows (recently added, continue watching, …) | **No** — cannot be unpromoted | No |

That third row governs the entire algorithm. Native hubs cannot be removed and re-added, so the
recovery move available for everything else does not exist for them. They are **anchors**: fixed
points the plan must work around rather than through.

### 15.2 Two ordering surfaces

**Home surface.** Ordering across all libraries as it appears on the Plex home screen, with three
independent visibility axes: owner's home, other users' home, and the library's recommended row.

**Library surface.** Ordering within a single library, split into two zones — a promoted zone with
explicit positions, and an alphabetical zone. Which zone an item lands in is not an API parameter: it
is a consequence of the item's sort title (§15.6).

The two surfaces share participants but not positions. A collection can sit third on the home screen
and tenth in its library.

Whether the home surface is itself one sequence or several is not yet settled — see the open spike
noted at the end of §15.4.

### 15.3 The precision problem

**This is the failure the design has to solve, so it is stated plainly.**

Plex stores each promoted item's position as a number, with new promotions spaced roughly 1000 units
apart. The only ordering primitive is relative: *move item A to sit after item B*. Plex implements
that by assigning A a value between B and B's successor — a midpoint.

Midpoint insertion between a fixed pair has a finite budget. After enough insertions into the same
gap, no representable value remains between the neighbours, and the move **silently does nothing**.
The item stays where it was, the API reports success, and the verification read shows the wrong
order.

The only way to restore headroom for an item is to unpromote and re-promote it, which appends it at
the end with fresh 1000-unit spacing — which is precisely the operation unavailable for native hubs.

Three consequences the design must respect:

1. **Every move consumes a non-renewable resource.** Move count is not merely a performance concern;
   it is the thing that eventually breaks correctness. Minimizing moves *is* the correctness
   strategy.
2. **A move can fail silently.** Every applied plan must be verified by reading back the actual
   order. Optimism is not available here.
3. **Recovery is asymmetric.** Managed and adopted collections can be rebalanced individually;
   native hubs cannot.

### 15.4 The algorithm

**Desired order is computed, not stored per item.** Each participant carries a `position` within a
scope (home, or a given library's promoted zone). Positions need not be dense or unique. The desired
sequence is derived deterministically:

1. Sort by `position` ascending.
2. Break ties by ULID ascending.

Tie-breaking by an immutable identifier is what stops two same-positioned collections from swapping
places on alternate runs — a flip-flop that would consume precision budget forever while appearing to
be an ordering bug.

Before anything else, the desired sequence is **deduplicated by identifier**. A participant appearing
twice — the same collection reachable as both managed and adopted, or a hub listed under two
identifiers — is collapsed to its first occurrence, and the duplication is reported. Ordering an
identifier set with duplicates cannot converge, because the target is not a permutation of the actual.

The per-participant placement definition looks like this:

```json
"spec": {
  "participant": { "type": "collection", "ref": "01J9Z…" },
  "surfaces": {
    "home":        { "position": 3, "visibility": ["owner", "sharedAll"] },
    "library":     { "position": 1, "zone": "promoted" },
    "recommended": { "visibility": ["everyone"] }
  },
  "randomize": false,
  "sortTitleConsent": false,
  "timeRestriction": {
    "alwaysActive": true,
    "whenInactive": "hide",
    "dateRanges": [],
    "weeklySchedule": null
  }
}
```

`participant.type` is `collection` for a collection I created, `adopted` for a collection the user
created, or `nativeHub` for one of Plex's own rows. The three behave differently (§15.1), and the
schema is common so ordering operates on one homogeneous set.

**Minimal move planning.** Given the actual sequence read from Plex and the desired sequence:

1. Compute the **longest subsequence of actual that is already in desired relative order** (a
   longest-increasing-subsequence over desired-rank).
2. Every item in that subsequence **stays put**. Every item not in it is moved exactly once, in
   desired order, each `after` its already-correct predecessor.

This yields `n − LIS` moves, which is the provable minimum under a relative-move primitive. In the
common case — one collection added, or one position edited — it is one or two moves rather than a
full rewrite of the library.

Move count reads naturally as an efficiency metric. Under the precision model it is not one: every
move spends a non-renewable resource, so move count is the primary *safety* metric, and it belongs in
the status surface (§15.9).

**Anchor preference.** When the plan has freedom — several move sets produce the desired order —
prefer plans that move re-promotable participants over anchors. Concretely, when choosing whether to
move item A after B or B before A, move the one that can be rebalanced if it later runs out of
headroom. A plan that repeatedly targets the gap in front of a native hub is a plan that will
eventually wedge with no recovery available.

**Precision budget accounting.** I cannot read Plex's stored position values, so I estimate instead.
Per library, I record a **subdivision depth** for each adjacent pair. When a planned insertion targets
a gap whose depth exceeds `gapBudget` (default 8), the plan **schedules a rebalance first** rather
than attempting the move and handling the failure.

**Depth, not a raw insertion count** (amended 2026-08-08). Inserting C between A and B does not leave
the pair (A,B) with one insertion — it *destroys* that pair and creates (A,C) and (C,B), each with
roughly half the numeric headroom the original had. If the children start at zero, the budget check
can never fire: a caller can subdivide the same region indefinitely, always into a "fresh" pair, and
exhaust precision while every counter reads 1. So both child pairs inherit `depth = parent.depth + 1`,
and depth resets to zero for a pair whose participant was just re-promoted with fresh spacing. The raw
insertion count is retained as a diagnostic — a pair with high insertions and low depth is a different
and interesting signal. Storage for this accounting lives in the data model's placement-tracking
tables.

This is the central inversion versus an earlier design that alternated between two Plex write
strategies purely to dodge a convergence problem — a strategy since cut as a symptom of the ordering
bug it never actually fixed. Under this design, rebalancing becomes a *planned step derived from
accounting*, not an exception handler triggered by a silent failure. Failures still occur — estimates
drift, other clients write — but they become the rare path rather than the routine one.

**The escalation ladder.** Bounded, deterministic, and reported at every rung. No rung is skipped, and
no rung is entered without a recorded reason.

| Rung | Action | Bound |
| --- | --- | --- |
| 0 | Apply the minimal move plan; verify by read-back | 1 attempt |
| 1 | For each item still misplaced and re-promotable: unpromote, re-promote (fresh spacing), re-plan the remainder | ≤ `rebalanceLimit` items per pass, default 5 |
| 2 | Full rebalance of one library: unpromote every re-promotable participant, re-promote in desired order, then position the anchors around them | 1 per library per pass |
| 3 | Stop. Mark the library `non-convergent`, surface it, retry next pass | — |

**Rung 3 is a real destination, not a theoretical one.** Non-convergence must be surfaced as a visible
status rather than hidden. A library that cannot be ordered stays in its last verified state with an
explanation, which is strictly better than a reset loop.

**What rung 2 must not become.** The obvious fallback — reset all hub management for a library and
rebuild — is forbidden. It discards positioning for adopted collections and native hubs, which is user
state I did not create, and reaching for it from an error path runs the most destructive operation in
the subsystem exactly when the system understands the situation least. Under this design, rung 2 is
scoped to re-promotable participants, keeps anchors in place, is idempotent, and is recorded before
execution. A true full reset exists only as an explicit operator action in the doctor page, with a
preview of what will be lost.

**Idempotency.** Before planning, compare a hash of (desired sequence, visibility set) against the
last verified state for that surface. Unchanged means **zero API calls** — not "zero moves after
computing a plan," but no reads or writes at all beyond the cheap verification read. This applies the
sync engine's idempotency rule here, and I-IDEM-1 is the test: a second run with unchanged inputs
writes nothing.

**Two open spikes.** Q-014 asks what the real precision budget is before exhaustion: `gapBudget`
defaults to 8, which is a guess — too high and moves start failing silently, too low and rebalances
run constantly, burning the very budget they exist to protect. The true figure depends on Plex's
numeric representation, which is undocumented and may vary by version, so it can only be calibrated
against a real server, and against a sequence of at least 2,500 items rather than a short one. It
blocks calibrating the default used in precision budget accounting above. Q-015 asks whether the home
screen is one global sequence, or per-library sequences merged at render. This is the most
consequential unknown left in the design: it determines whether ordering is one planning problem or
several, which changes the planner, the gap accounting, and the lease scope, and it also blocks the
question of whether the home-screen board is one merged surface or one board per library. Q-015
should be resolved first, since Q-014's measurement depends on its answer.

### 15.5 Visibility

Three independent axes per participant: owner home, shared-user home, library recommended. Under an
admin-only permission surface, these are set globally; the stored shape is a set of principals with
`everyone` as the only writable value, so per-user targeting later is a widening rather than a
migration.

**Visibility is a set of principals**, not booleans, and it is scoped to a *surface*. An earlier
revision conflated a surface with a principal by listing `recommended` inside a visibility array
alongside `admin` and `shared`, which made "visible on the recommended row to shared users only"
inexpressible. Home and recommended are independently controlled surfaces; `owner`, `sharedAll`, and
`everyone` are principals. At launch the GUI writes only whole-audience principals; per-user targeting
later widens the set without a schema migration.

Visibility changes are applied **before** ordering within a pass. An item being made visible must
exist in the ordering space before its position can be set, and an item being hidden should not
consume a move.

### 15.6 Sort titles, zones, and consent

The promoted/alphabetical split in the library surface is achieved by writing prefix characters into
the item's **sort title** — Plex metadata the user can see and may have set deliberately.

This is the only place where achieving a layout requires mutating a user's object. The rules are
therefore strict.

**One function.** Exactly one implementation computes and strips sort-title prefixes. Multiple
sanitizers that disagree is how prefixes accumulate and titles drift.

**Original values are recorded** before the first mutation, per item, so restoration is always
possible. A boolean remembering that an item was once promoted is not sufficient: it records that a
value changed without recording what it was.

**Adopted collections require consent.** I do not write sort titles on collections I did not create
until the user opts in. **The library is the consent unit** (decided 2026-08-08): one control per
library, with a per-collection override for exceptions, and no global control at launch. The GUI
states plainly that the sort title will be modified and shows the before/after. Consent is recorded
alongside the captured original, so a title written under a consent later revoked is still
explainable.

**Round-trip obligation.** Promote then demote must restore the exact original sort title, byte for
byte — a three-property rule covering **its value, its presence, and its lock state**. Plex clients
default a missing sort title to the item's title when parsing, so absence and "equal to the title" are
indistinguishable after parsing, and presence must be recorded from the raw attribute rather than the
parsed value. Separately, every editable Plex metadata field carries a lock flag that the edit
endpoint writes alongside the value — a restore that leaves the field locked has permanently disabled
the server's own metadata refresh for that item, silently and with no report (amended 2026-08-08; see
I-REV-3). This is a test, not an intention.

**Prefix depth is bounded and idempotent.** Applying the prefix twice produces the same string.

`sortTitleConsent` must be true before I will modify the sort title of an adopted collection. Zone
placement is achieved by writing sort-title prefixes, which mutates a user's own object; consent is
required, the original value is recorded, and demotion restores it exactly.

### 15.7 Randomized home ordering

Participants flagged for randomization are shuffled among **their own positions only** — the set of
positions they collectively occupy is preserved, so randomizing never displaces a pinned item.

The shuffle is seeded by `(rotationEpoch, surface)`, where `rotationEpoch` advances on the
randomization schedule rather than on every pass. Consequences that matter:

- A sync that runs three times in an hour produces the *same* order all three times — no gratuitous
  moves, no precision burned for nothing.
- The order is reproducible from the audit record, so "why did this move?" is answerable.
- A user can force a re-roll by advancing the epoch explicitly.

### 15.8 Self-healing

Participants are tracked by identifier, and identifiers change. The pass reconciles:

| Situation | Behaviour |
| --- | --- |
| Managed collection missing in Plex | Recreate, restore position, record the heal |
| Managed collection found with a new rating key | Rebind by label and canonical identity; no reordering of others |
| Adopted collection deleted by the user | Drop the adoption, report it; never recreate — it was theirs |
| Adopted collection renamed | Rebind by rating key; name is display only |
| Native hub absent (Plex version differences) | Drop from the plan, report; never fail the pass |
| Unknown participant present in the ordering space | Leave it alone, record it. I do not evict what I do not manage |

That last row is a policy, not an oversight: an ordering space containing a hub I don't recognize is a
space shared with another tool or a newer Plex, and evicting it would be exactly the behaviour that
makes tools unusable together.

### 15.9 Status and observability

Per library and per surface, each pass records: participant count, moves planned, moves applied,
verification result, rebalances performed, escalation rung reached, gap-budget pressure, and
non-convergence with the specific items that would not settle.

Surfaced live over SSE and summarized on the doctor page. Two numbers deserve prominence because they
are leading indicators of the failure mode: **moves per pass** and **rebalances per pass**. A library
trending upward on either is heading for rung 3 before it gets there.

## 16. Posters and overlays

### 16.1 The original poster is sacred

Base art is captured once, content-addressed and deduplicated. Every application composites pristine
base plus current template plus current state. An overlay is never applied over an overlay.

Capture happens through a dedicated download step that pulls the pristine original before any
overlaying occurs, and the source of that pristine base is a configurable preference — TMDB, Plex, or
a local file are each candidates, chosen in order of preference rather than hard-coded to one
provider.

### 16.2 The render key

**Render key = hash(base poster, template version, state snapshot, renderer version).** An unchanged
key skips the upload entirely. Removal is trivially complete: upload the base, drop the key. The
renderer version is part of the key because a rasteriser or font-shaping change alters the output for
identical definition-layer inputs; without it, a rendering improvement matches every existing cache
entry and therefore reaches nobody.

This generalizes, and D-043 states it as a rule: **a cache keyed only on inputs is correct only while
the function from inputs to outputs is fixed.** Where that function ships as code that changes, its
version belongs in the key. Elsewhere, the HTTP response cache carries a per-source parser version for
exactly this reason — the same rule applied to a different cache.

Overlaying is applied immediately to newly-added items during sync rather than deferred to a separate
pass, and there is deliberately only one pipeline for this: an earlier design ran a second,
incremental-only sync path over changed items to save time, but that path was cut, because
incrementality is a property the render key already gives for free — it is a cache concern, not a
reason for a second pipeline.

### 16.3 Byte-exact restoration

Restoration is byte-exact. Removing overlays uploads the stored base poster exactly as captured —
never a resized, re-encoded, or re-cropped copy of it. The same restoration step is what a poster
reset invokes: it restores originals rather than regenerating or re-fetching them.

### 16.4 Overlay inputs and formatters

**Overlay inputs, in priority order:** lifecycle state; Plex media streams; library metadata; external
ratings. A context builder assembles this state before any element is rendered, so every formatter and
every template element draws from the same resolved snapshot.

**Formatters are pure functions** — no clock, no I/O, no randomness. This is what makes the render key
sound: if a formatter could vary independently of its inputs, the hash would not identify the output.
I-RENDER-5 is the test. Formatters are engine functions from a closed set, composed into type-checked
pipelines.

The template that drives an overlay looks like this:

```json
"spec": {
  "canvas": { "aspect": "poster", "safeArea": { "top": 0, "right": 0, "bottom": 0, "left": 0 } },
  "variables": [
    { "name": "resolution", "source": "media.videoResolution" },
    { "name": "daysUntil", "source": "lifecycle.daysUntilRelease" }
  ],
  "elements": [
    {
      "id": "badge-bg",
      "type": "tile",
      "layer": 10,
      "anchor": { "h": "left", "v": "top", "dx": 24, "dy": 24 },
      "size": { "w": 220, "h": 72 },
      "style": { "fill": "#000000CC", "cornerRadius": 12 },
      "when": { "field": "media.videoResolution", "op": "in", "value": ["4k", "1080"] }
    },
    {
      "id": "badge-text",
      "type": "text",
      "layer": 11,
      "bindsTo": "badge-bg",
      "text": "{resolution|upper}",
      "font": { "asset": "afisharr.core/inter-bold", "size": 40, "fill": "#FFFFFF" },
      "when": { "field": "media.videoResolution", "op": "exists" }
    }
  ]
}
```

- **Element types (closed set):** `text`, `tile`, `raster`, `svg`, `variable`, `mappedIcon`.
- **Unresolved variables skip their element.** An element whose bound variable resolves to null or
  unavailable is not rendered — never rendered blank, never rendered as a placeholder string. This is
  what makes conditional packs work.
- **Mapped icons require a fallback.** A value-to-asset table without one fails validation, because a
  future release may add an enumeration value the pack has never seen. Value→icon mappings ship in two
  layers, a default set and a user-editable set on top of it.
- **Coordinates** are canvas units against a fixed reference size per aspect; the renderer scales.
  `bindsTo` anchors one element relative to another, which is how packs stay resolution-independent.
- **Renderer constraint.** Text styling covers fills, single stroke or outline, drop shadow, and letter
  spacing. No per-glyph gradients or text-on-path in v1; pack authoring guidelines target this set.

### 16.5 The template editor and preview

The template renders in the GUI preview using the *same* renderer compiled into the server, so preview
and output cannot drift. Templates, elements, and packs are definition documents; the editor and the
preview renderer use the same renderer as production output, so preview and result cannot diverge. An
on-demand test endpoint renders one item using this same renderer, for debugging a single item without
running a sync.

The same element model and editor serve the poster template as well; the two kinds differ in their
canvas, their available field scope, and their output target, not in their structure.

Which templates apply to which library is a per-library configuration, independent of the templates
themselves, and starter overlay templates ship as packs rather than as hard-coded defaults.

**Episode and season-level overlays are Tier 1.5.**

### 16.6 Poster generation

| Capability | What it does |
| --- | --- |
| Poster template model | Layered design: background, text, tiles, 1000×1500 canvas |
| Poster editor GUI | Visual editor with layer panel and background controls |
| Provider brand palettes | Per-source colour schemes auto-applied to generated posters; user-overridable |
| Saved posters | Persist generated output, content-addressed |
| Preview asset packs | Sample posters/persons for editor preview |
| Default template seed | Ships one usable template out of the box, as a pack |

### 16.7 Assets, fonts, and icons

| Capability | What it does |
| --- | --- |
| Custom poster | Upload/choose art, optionally per-library |
| Auto-generated poster | Render a poster from a template at sync time |
| Franchise poster passthrough | Use the provider's franchise art instead of generating |
| Custom wallpaper (art/backdrop) | Sets the collection background image |
| Custom summary | Overrides collection description |
| Theme music | Uploads/sets collection theme audio (Tier 1) |
| Local asset folders | Scans a directory tree for posters/art by name |
| Font asset management | Upload/manage fonts for rendering |
| Icon asset management | Icon library for overlay mappings |
| File upload endpoint | Generic asset upload |
| Server filesystem browser | Pick paths on the host from the GUI, jailed to configured root paths |
## 17. Lifecycle

The lifecycle system tracks a title from announcement through release to availability, materializes placeholders so unreleased titles appear in the library, drives acquisition, and supplies the state that status overlays render. It is the product's central differentiator and therefore the component with the strictest correctness obligations: it writes files into user libraries and deletes them again.

**Governing rule:** every destructive action must be explainable after the fact by a recorded transition and the evidence that justified it. If the system cannot say why it deleted something, the design is wrong, not the log.

### 17.1 Design decisions

**State is persisted, not recomputed.** A subject's state is stored and changed only by recorded transitions. Nothing infers state freshly from live APIs at the moment it acts.

*Why:* derived-on-read state means an API outage silently changes what the system believes, and no deletion can ever be justified because the reasoning is gone by the time anyone asks. Persisting state converts "the API said nothing so the title looks unreleased" from an invisible data-loss bug into an explicit, refused transition.

**State is a composite of four orthogonal axes, not one flat enum.** Release phase, acquisition, presence, and (for TV) production status are independent facts. The user-facing status label is *derived* from the composite by a pure mapping function.

*Why:* a flat enum forces impossible choices — "awaiting download" is an acquisition fact, "releasing tomorrow" is a calendar fact, and a title can be both. Flat enums also grow combinatorially: each new acquisition mode multiplies every phase.

**Transitions require positive evidence.** Absence of data is never evidence. A subject whose evidence could not be refreshed is marked stale, keeps its state, and is barred from destructive transitions.

**No wall-clock timers.** State is a pure function of (persisted state, evaluation clock, evidence). An evaluation that runs six hours late produces the same result as one on time. There are no scheduled callbacks, no "fire at midnight" jobs, no timer state to lose on restart.

**Side effects are intent-logged before they happen.** Creating a placeholder is a file write plus a Plex scan; deleting one is a file delete plus a refresh. Intent is committed first, executed second, confirmed third. Startup reconciles unconfirmed intents.

**One subject per library item and season, reference-counted across collections.** A title wanted by four collections has one placeholder file and one lifecycle record, with four references.

The unit is the pair (library item, season), not the library item alone. A whole-title subject carries no season number; a season subject carries one. §17.2.1 defines the two granularities.

*Why:* per-collection lifecycle records mean four collections racing to create and delete the same file, and "clean up collection A" deleting the placeholder collection B is still using.

### 17.2 The subject

The unit of lifecycle tracking:

```
LifecycleSubject
  id                  ULID
  library             library id
  canonicalId         { tmdb, tvdb, imdb }   -- at least one, tmdb preferred
  mediaType           movie | show
  seasonNumber        nullable integer — null means the whole title; see §17.2.1
  title, year         cached for display and audit legibility
  phase               see §17.3
  acquisition         see §17.4
  presence            see §17.5
  production          see §17.6 (show only, null for movies)
  releaseDate         resolved date, nullable
  releaseDateBasis    digital | physical | theatrical_estimate | first_air | next_episode |
                      season_air | none
  evidenceAt          timestamp of the last successful full evidence refresh
  stale               bool — evidence refresh failed on the most recent pass
  references          set of definition ids that currently want this subject
  placeholderPath     nullable, set only while presence = Placeholder
  plexRatingKey       nullable, discovered after Plex indexes the placeholder
  policyVersion       the lifecycle policy revision that last evaluated this subject
```

`references` is the reference count. It is recomputed each pass from the collections that resolved to this subject; it is never incremented ad hoc.

#### 17.2.1 Two granularities, whole-title by default

A subject tracks either a whole title or a single season. `seasonNumber` carries the distinction: null means the whole title, an integer means that season of that show. Movies are always whole-title. Decided as D-025.

**A whole-title subject always exists for a tracked show.** Season subjects are added beside it, and never replace it. A show whose seasons are tracked individually still has one status on its own poster, because the whole-title subject still computes one.

**Season subjects are opt-in per show.** `seasonGranularity` (§17.11) is `off` by default at instance and definition level, with a per-show override. When it is on, Afisharr creates a season subject for each season whose air date falls inside `countdownWindow`, and no others. A show with nine aired seasons and one upcoming season gets one season subject, not ten.

**The two granularities never contend for the same file.** Placeholder ownership divides by what is absent:

| Condition | Owner of the placeholder |
| --- | --- |
| The show is absent from the library entirely | The whole-title subject |
| The show exists and a tracked season is absent | That season's subject |
| The show is absent and seasons are tracked | The whole-title subject only, until the show becomes `Real` |

The third row is the one that bites. A show that does not exist in Plex cannot hold a season stub inside it, so season subjects stay `Absent` and materialize nothing until the whole-title subject reaches `Real`. Enforced by I-LIFE-4.

**Season subjects have Tier 0 value without season overlays.** Season and episode overlays are Tier 1.5, so nothing renders a season badge at launch. Season tracking still earns its place at Tier 0 through two other consumers: a placeholder for an upcoming season of a show already in the library, and season-level monitoring against Sonarr. Rendering is one consumer of this state, not its purpose.

**Production is not recomputed per season.** A season subject reads the production axis (§17.6) from its parent show. A season does not have its own production status; the show does.

### 17.3 Phase — where the title sits on its own calendar

Computed from the resolved release date `R` and the evaluation date `T`, with windows from policy.

| Phase | Condition |
| --- | --- |
| `Announced` | No reliable `R` (TBA, or only an unreliable estimate the policy rejects) |
| `Scheduled` | `R > T + countdownWindow` |
| `Countdown` | `T + 1 < R ≤ T + countdownWindow` |
| `Tomorrow` | `R = T + 1` |
| `Today` | `R = T` |
| `JustReleased` | `0 < T - R ≤ newReleaseWindow` |
| `Released` | `T - R > newReleaseWindow` |

Phase transitions are **bidirectional**. Release dates move — a delayed film goes `Today → Countdown`, and a title can leave `JustReleased` back into `Countdown` if its date is pushed. Any implementation that assumes monotonic forward progress is wrong.

#### 17.3.1 Resolving the release date

Movies, in priority order — the first available wins, and the choice is recorded in `releaseDateBasis`:

1. Digital release
2. Physical release
3. Theatrical + `theatricalEstimateDays` (default 90), recorded as `theatrical_estimate`

Shows: `first_air_date` for an unaired series; otherwise the next episode's air date; season boundaries use the season's air date when the next episode is episode 1 of a new season.

Season subjects (§17.2.1): the season's own air date, recorded as `season_air`. When the provider gives a season no air date, the season subject stays `Announced` with a null date. It never inherits the show's date — a show that aired last week says nothing about when season 4 arrives, and borrowing that date would put a season into `Released` on evidence about a different thing.

`releaseDateBasis` matters downstream: a `theatrical_estimate` date is a guess, and policy may forbid destructive transitions justified solely by a guessed date. Overlay packs can also render it differently ("expected" vs. a firm date).

### 17.4 Acquisition axis — what the download stack is doing

| State | Meaning |
| --- | --- |
| `Untracked` | Not present in any *arr instance, no outstanding request |
| `Requested` | A request exists and is awaiting approval |
| `Monitored` | Present in *arr, monitored, no file yet |
| `Unmonitored` | Present in *arr, not monitored |
| `Grabbing` | *arr reports an active queue item |
| `Available` | A real file exists |

`Grabbing` is only reachable when queue data is actually available; when it isn't, the subject stays `Monitored`. That is a deliberate under-report: it is honest, and it never justifies an action that `Monitored` wouldn't.

### 17.5 Presence — what Afisharr has put in the library

| State | Meaning |
| --- | --- |
| `Absent` | Nothing in the library for this subject |
| `PlaceholderPending` | Creation intent committed, not yet confirmed |
| `Placeholder` | A materialized stub exists and Plex has indexed it |
| `Real` | Genuine playable media exists |
| `RemovalPending` | Removal intent committed, not yet confirmed |

The two pending states exist solely so a crash mid-write is recoverable (§17.9).

#### 17.5.1 The placeholder marker

See *Data model*, the lifecycle tables. Every materialized item carries a marker so that hub replacement, filters, and orphan sweeps can identify it **without parsing filenames**:

1. A Plex label — the authoritative runtime marker.
2. A database record binding `plexRatingKey` → subject — the authoritative durable marker.
3. An edition tag in the filename — a *hint* for orphan sweeps only.

Correctness never depends on (3). Filename conventions change across versions and leave unrecognized files accumulating forever; a sweep that matches only the current convention silently strands the rest. The sweep therefore treats any unreferenced file under a placeholder root as a candidate and reports it, rather than matching a hardcoded list of past naming schemes.

### 17.6 Production — TV only

An independent axis, not part of the transition machine. Derived from provider status with a recency override:

| Production | Derivation |
| --- | --- |
| `Airing` | Status is continuing **and** an episode aired within `airingWindow` (default 15 days) |
| `Returning` | Status is continuing, no recent episode |
| `InProduction` | Announced or in production, unaired |
| `Ended` | Concluded |
| `Cancelled` | Cancelled |
| `Unknown` | No usable status |

Provider status vocabularies differ and are mapped through a translation table in the field registry, with a secondary provider consulted only when the primary yields `Unknown`. Disagreement between providers resolves to the primary and is logged, never averaged.

### 17.7 Derived status — the overlay contract

The label overlay packs render. A **pure function** of (phase, acquisition, presence, production). No I/O, no clock reads beyond the evaluation clock, exhaustively table-tested.

| Status | Composite condition |
| --- | --- |
| `ComingSoon` | `Announced` or `Scheduled` |
| `RequestNeeded` | Not yet released, `acquisition = Untracked` |
| `CountdownMonitored` | `Countdown`, acquisition ∈ {Monitored, Grabbing, Requested} |
| `CountdownUnmonitored` | `Countdown`, acquisition ∈ {Untracked, Unmonitored} |
| `ReleasingTomorrow` | `Tomorrow` |
| `ReleasingToday` | `Today` |
| `AwaitingDownload` | Released, no file, acquisition ∈ {Monitored, Grabbing} |
| `NewRelease` | `JustReleased`, `presence = Real` |
| `Available` | `Released`, `presence = Real` |
| `Airing` / `Returning` / `Ended` / `Cancelled` | Show-only production overlays, composable with the above |

Exposed to the definition layer under the `lifecycle.*` namespace:

`lifecycle.status`, `lifecycle.phase`, `lifecycle.acquisition`, `lifecycle.presence`, `lifecycle.production`, `lifecycle.releaseDate`, `lifecycle.releaseDateBasis`, `lifecycle.daysUntilRelease`, `lifecycle.daysSinceRelease`, `lifecycle.isStale`, `lifecycle.isPlaceholder`, `lifecycle.seasonNumber`.

These become field-registry entries in the registries pass, with declared types and legal operators.

### 17.8 Transitions, guards, and the destructive-action allowlist

An evaluation pass, per subject, in fixed order:

1. **Refresh evidence.** Metadata provider, *arr instances, Plex presence. Each field records its source and fetch time.
2. **Assess staleness.** Any required evidence missing or failed → `stale = true`; skip to step 6.
3. **Re-evaluate references.** Which collections still want this subject, after filters (filters run every pass — a subject that no longer passes is dereferenced, not grandfathered).
4. **Compute target axes** from evidence.
5. **Emit transitions** for each axis that changed, subject to the guards below.
6. **Execute side effects**, gated by §17.9.

#### 17.8.1 Guards

| Guard | Rule |
| --- | --- |
| G1 | A stale subject may not transition at all. It keeps its state and is skipped. |
| G2 | Presence may only move to `RemovalPending` via an allowlisted trigger (§17.8.2). |
| G3 | `presence = Real` may never be set from provider data alone — it requires positive confirmation of a playable file from Plex. |
| G4 | A destructive transition justified only by a `theatrical_estimate` date is refused when `strictDates` policy is on. |
| G5 | A subject with `references > 0` may not be removed for reasons of departure. |
| G6 | Two transitions of the same axis in one pass is a bug; the pass fails loudly rather than picking one. |
| G7 | A subject whose canonical ID matched more than one library item enters `Ambiguous` and is never acted on until a human resolves it. |

#### 17.8.2 Destructive-action allowlist

A placeholder may be removed **only** by one of these triggers, each of which must carry its evidence into the audit record:

| Trigger | Condition | Evidence recorded |
| --- | --- | --- |
| `Materialized` | Real media confirmed present | Plex item id, media part id, confirmation time |
| `Departed` | `references` reached 0 | The definitions that dropped it and why (filter fail / removed from source / definition deleted) |
| `Retired` | `JustReleased` window expired under a retire policy | Release date, basis, window, evaluation date |
| `FilteredOut` | Fails the placeholder filters on re-evaluation | The specific predicate that failed |
| `Disabled` | Placeholders switched off for every referencing definition | Definition ids and their setting values |
| `Manual` | Operator action | Actor, timestamp |
| `Reaped` | Orphan sweep: file exists, no subject references it | Path, scan time, sweep id |

Anything not on this list **cannot** delete. Notably absent, and deliberately: source fetch failure, missing metadata, unknown state, provider disagreement, ambiguous match, "not seen this pass."

### 17.9 Crash safety

Every side effect is a three-step sequence:

1. **Intend** — write the intent record and move presence to `PlaceholderPending` / `RemovalPending`. Committed transactionally.
2. **Execute** — perform the file operation and notify Plex.
3. **Confirm** — verify the observable result (file exists / is gone; Plex indexed / released) and settle presence to `Placeholder`, `Absent`, or back to its prior state on failure.

On startup, every unconfirmed intent is re-driven from step 2. Both operations are idempotent: creating an existing placeholder is a no-op, deleting an absent one is a no-op. A `kill -9` at any point leaves the system in a state a single startup pass repairs. I-DATA-1 states this as a build-failing property, here applied to the component that touches user files.

### 17.10 Audit record

Append-only. One record per transition and per side effect.

```json
{
  "id": "01J…",
  "at": "2026-08-08T14:03:11Z",
  "subject": { "id": "01J…", "library": "movies", "tmdb": 693134, "title": "Dune: Part Two" },
  "axis": "presence",
  "from": "Placeholder",
  "to": "RemovalPending",
  "trigger": "Materialized",
  "evidence": [
    { "field": "plex.hasMedia", "value": true, "source": "plex", "fetchedAt": "2026-08-08T14:03:09Z" },
    { "field": "plex.ratingKey", "value": "48213", "source": "plex", "fetchedAt": "2026-08-08T14:03:09Z" }
  ],
  "policyVersion": 3,
  "actor": "scheduler:collection-sync",
  "effects": [
    { "kind": "deleteFile", "path": "/media/placeholders/…", "result": "ok" },
    { "kind": "plexRefresh", "target": "movies", "result": "ok" }
  ]
}
```

Retention is bounded by count and age, but records for destructive triggers are retained longer — they are the ones anyone ever asks about.

### 17.11 Configuration surface

| Setting | Default | Meaning |
| --- | --- | --- |
| `countdownWindow` | 360 days | How far ahead to track and materialize |
| `newReleaseWindow` | 7 days | How long a released title stays flagged new |
| `theatricalEstimateDays` | 90 | Theatrical-to-digital estimate when no firm date exists |
| `airingWindow` | 15 days | Recency threshold for `Airing` |
| `includeAllReleased` | true | Track already-released titles regardless of date |
| `strictDates` | true | Forbid destructive transitions justified only by an estimated date |
| `retirePolicy` | keep | What happens when `JustReleased` expires and no real media arrived: `keep` (default) or `remove` |
| `placeholderRoots` | per library | Movie and show placeholder paths |
| `monitoredSources` | — | *arr instances and tag include/exclude filters constraining what is tracked |
| `seasonGranularity` | off | Whether a show also gets one subject per season inside `countdownWindow` (§17.2.1). Per-show override |

All are per-definition with instance-level defaults, and all are recorded in the audit record's `policyVersion` so historical decisions stay interpretable after a settings change.

### 17.12 Edge cases the implementation must handle

| Case | Required behaviour |
| --- | --- |
| Release date moves later | Reverse phase transition, no removal |
| Release date moves earlier, past today | Forward transition through skipped phases; intermediate phases are not fabricated in the audit log |
| Provider unreachable | Stale; no transitions; overlays render last-known state with `lifecycle.isStale` true |
| Provider returns a title as unknown/deleted | Stale, not `Departed`. Deletion upstream is not evidence of anything locally |
| *arr instance offline | Acquisition axis frozen; phase axis may still advance |
| Real media appears while placeholder present | `Materialized` → remove placeholder → `presence = Real` |
| User deletes the real media | `presence` returns to `Absent`; re-materialization is allowed if phase warrants |
| User deletes the placeholder in Plex | Detected on confirm; subject settles to `Absent`; recreated next pass if still wanted |
| Two library items match one canonical ID | `Ambiguous`; no action; surfaced in the doctor page |
| Movie never receives a digital date | Stays on `theatrical_estimate`; under `strictDates` it cannot be retired on that basis alone |
| Show with no next episode and no end status | `Returning` with a null date; phase `Announced` |
| Collection deleted mid-pass | References recomputed next pass; `Departed` fires then, not mid-flight |
| Placeholder root path changes | Old paths become orphan-sweep candidates; sweep reports before deleting |
| Same subject wanted by collections with conflicting policies | Most permissive tracking, most restrictive destruction. Recorded explicitly |
| `seasonGranularity` turned off for a show that has season subjects | Season subjects are retired under `retirePolicy`, exactly as an unwanted subject is. Turning a setting off never bypasses the retirement path |
| A tracked season is renumbered or merged upstream | The old season subject loses its provider evidence and goes stale. It is never silently rebound to a different season number, since `seasonNumber` is part of the identity |
| Season air date exists, show is absent from the library | Season subject stays `Absent` and materializes nothing until the whole-title subject reaches `Real` (§17.2.1) |
| A season subject and its show both want a placeholder | Ownership divides by the table in §17.2.1. The two never write the same path. I-LIFE-4 |

## 18. Acquisition policy

The lifecycle machine decides *whether* a title should be acquired; the acquisition policy decides *how*, and is evaluated on the same pass.

I track unreleased items and their dates, write stub media files so unreleased titles appear, discover existing placeholders on disk and in Plex, clean placeholders up when state changes, and repair title mis-matches on placeholder files. These are Tier 0 capabilities, per-library, with configurable placeholder roots, a look-ahead window, a released-retention window, an include-all-released toggle, independent placeholder filters (year/rating/genre/country/language/keyword, separate from grab filters), and monitored-source filtering that restricts tracking to specific *arr instances and tags in include/exclude mode. A trailer-download feature for placeholder video is cut in favor of a static placeholder video.

I also track what a collection wants but lacks as missing-item records.

**Eligibility gates** (all must pass): position cap, minimum year, minimum ratings (IMDb, RT critics, RT audience — independently), genre / country / language / keyword filters in include or exclude mode, and the collection's own enablement.

**Routing:** requests or direct-to-*arr, never both for one subject. This routes either to creating a request or adding straight to an *arr instance. Instance, quality profile, root folder, tags, monitor mode, search-on-add, and season folder are resolved per subject and recorded. The routing decision itself, and the six per-request overrides in active use, are Tier 0; the full override matrix is Tier 1. Multiple *arr servers of the same kind, selected per collection, are supported.

**TV shaping:** maximum seasons to request, per-show season cap, and grab order (first / latest / airing) resolve to a concrete season list, which is recorded so the decision is reproducible.

Tagging items already present in the library is a Tier 1 capability, deferred. A watchlist-sync capability that mirrors external watchlists into an *arr instance is cut — it is a second product sharing *arr plumbing, unrelated to collections, posters, or overlays.

**Reproducibility obligation:** the audit record for a grab must contain everything needed to recompute the decision — inputs, gates evaluated, policy version, chosen route and overrides. I-ACQ-4 requires a grab decision to be reproducible from its record alone; that is a property of the record's completeness, not of the code.

**I never request or grab on the basis of an unverifiable state (I-ACQ-1).** Acquisition acts only on evidence that has been positively confirmed on the current pass; a stale or unrefreshed subject is barred from triggering a request or a grab, on the same footing as it is barred from a destructive presence transition (§17.8.1, G1).
---

## 19. Data model

Sixty-eight tables. This section carries the conventions, the storage layout, the migration and
concurrency policy, the full DDL, the indexes, the hot paths each index serves, and the retention
rules.

**Verification note.** The count of 68 is stated against the `CREATE TABLE` statements themselves,
not maintained by hand. It excludes the illustrative `t_new` in the table-rebuild example of §19.3.3,
which is a worked example rather than a table the schema defines. Earlier drafts counted it and
therefore said 69; the corrected figure is 68, and the set of tables is unchanged. All DDL below was executed against SQLite 3.45, and three claimed-structural
constraints were exercised behaviourally: duplicate lifecycle identity is rejected by a unique index,
a placeholder presence without a path is rejected by a `CHECK`, and a destructive transition under a
trigger outside the allowlist cannot be inserted.

### 19.1 Verification note and conventions

I use SQLite as the complete persistence layer. This section is authoritative for persistent
structure: table shapes, key choices, derivation rules, migration policy, concurrency policy,
indexing, and retention.

It is the section that must satisfy obligations accumulated across every prior design pass. A later
section in this document checks each one off against the table and column that discharges it. If an
obligation has no concrete target, I treat the schema as incomplete, not merely undecided.

**Verification status.** Every `CREATE TABLE` and `CREATE INDEX` statement below has been executed
against SQLite 3.45 — 144 statements, all accepted. The three constraints I claim are *structural*
rather than conventional were then exercised against the created schema and behave as described: a
second whole-title subject for the same identity is rejected by `ux_lifecycle_subjects__identity`;
`presence = 'Placeholder'` without a path is rejected; and a transition marked destructive under a
trigger outside the allowlist (see *Lifecycle*, §8.2) cannot be inserted. The DDL is copy-ready, not
illustrative.

The four tables I added on 2026-08-09 by CR-5, CR-6, and CR-7 — `reference_datasets`,
`reference_dataset_rows`, `volatile_params`, and `pack_variable_values` — were executed against
SQLite 3.51 and their three claimed constraints exercised: `import_state` outside its allowlist is
rejected, a non-JSON `value_json` is rejected, and deleting a pack cascades its stored variable
values away.

**Table count.** I define 68 tables in this schema. Earlier drafts said 67 while the DDL carried 65,
then said 69 by counting the illustrative `t_new` in the table-rebuild example of §19.3.3; both
discrepancies are corrected here rather than carried forward. The number is the count of
`CREATE TABLE` statements below excluding that worked example, which is the only definition of it
that cannot drift.

**What this section does not do.** It does not define the definition body format or the field
vocabulary (see *The definition layer*), the state machine semantics (see *Lifecycle*), or the ordering algorithm
(see *Placement and ordering*). It stores what those sections decide, and nothing here may contradict them. Where
reading the wire protocol turns up a fact a design section did not anticipate, the fix belongs there,
not here. Per the authority order established for this document set, a conflict is a bug to report
rather than an ambiguity to resolve by choosing. Three such facts were found while writing this
schema; all three were applied to their owning sections on 2026-08-08.

#### Conventions

Every rule below is mandatory. Uniformity is worth more here than local optimisation: a schema this
large is navigated by pattern, and one column that stores time differently is a bug generator for the
life of the product.

**Types.** All tables are declared `STRICT`. SQLite's default type affinity accepts a string into an
integer column and stores it as a string; `STRICT` rejects it at the boundary. The cost is that only
`INTEGER`, `REAL`, `TEXT`, `BLOB`, and `ANY` are legal column types — no `VARCHAR`, no `BOOLEAN`, no
`DATETIME`. That constraint is the point.

| Concept | Storage | Rule |
| --- | --- | --- |
| Identifier | `TEXT` | ULID, 26 characters, Crockford base32, uppercase |
| Instant | `INTEGER` | Milliseconds since the Unix epoch, UTC. Column name ends `_at` |
| Civil date | `TEXT` | `YYYY-MM-DD`. Column name ends `_date` |
| Duration | `INTEGER` | Milliseconds. Column name ends `_ms` |
| Boolean | `INTEGER` | `0` or `1`, with `CHECK (col IN (0,1))`. Column name reads as a predicate (`is_`, `has_`, `enabled`) |
| Enumeration | `TEXT` | Exact case-sensitive token, see the enumerations rule below |
| Structured value | `TEXT` | Canonical JSON, with `CHECK (json_valid(col))`. Column name ends `_json` |
| Content digest | `TEXT` | Lowercase hex. Column name ends `_sha256` or `_hash` |
| Opaque bytes | `BLOB` | Only where byte-exactness matters (asset storage, §19.2) |

**Instants versus civil dates is not a style choice.** A release date is a calendar fact, not a
moment: a film released on 2026-11-06 is released on 2026-11-06 everywhere. The engine's date
operators must be day-aligned in the instance timezone, and the lifecycle model computes phase from a
date difference. Storing a release date as an instant forces a timezone decision at write time and
makes "releases tomorrow" wrong for half the planet. Civil dates are stored as text, compared as
text, and converted to a local civil date exactly once per pass, from the evaluation clock.

**Instants are integers, not text.** They are compared, differenced, and range-scanned constantly;
integer comparison is cheap and unambiguous, and there is no format to get wrong.

**Identifiers.** Every entity I create carries a ULID primary key stored as `TEXT`.

ULIDs are lexicographically ordered by creation time, so `ORDER BY id` is `ORDER BY created_at` with
no extra column and no index. That property is load-bearing rather than incidental: the placement
model breaks ordering ties by ULID ascending, and it does so precisely because the tie-break must be
stable across passes and independent of any mutable field. A `TEXT` ULID with SQLite's default
`BINARY` collation sorts correctly with no special handling.

Identifiers assigned by Plex (`ratingKey`, section keys, machine identifiers, account ids) are
**never** primary keys. They change. They are stored as ordinary columns on the row whose internal
identity is a ULID, and rebinding a changed Plex identifier is an `UPDATE`, not a re-key (see
*Placement and ordering*).

Human handles (`user/trending-now`, library slugs) are unique but mutable-by-rename, and are never
used as foreign keys. References are by ULID, per the engine's identifier rules.

**Naming.** `snake_case` throughout. Tables are plural nouns (`definitions`, `lifecycle_subjects`);
junction tables name both sides (`lifecycle_subject_ids`). Foreign key columns are
`<singular_table>_id`. Indexes are `ix_<table>__<columns>`; unique indexes `ux_<table>__<columns>`;
partial indexes append a predicate hint (`ix_lifecycle_intents__open`).

**Enumerations and `CHECK`.** Enumerated columns store the exact token used in the design
documentation — `Placeholder`, not `placeholder`, because the audit log is read by people and by
tests, and a case fold between the documentation and the database is a translation layer nobody asked
for.

`CHECK` constraints are applied selectively, and the split is deliberate:

| Enumeration class | `CHECK`? | Reason |
| --- | --- | --- |
| State-machine states (lifecycle axes, intent states, participant types, surfaces, escalation rungs) | **Yes** | An illegal value here is a correctness bug that must never reach disk. The value set changes only by a deliberate redesign, which is a migration anyway |
| Registry vocabulary (source types, field types, formatter names, asset kinds) | **No** | The engine's registry permits adding a source or an enum value on a *minor* registry bump. A `CHECK` would turn every minor bump into a table rebuild for no safety gain — the registry already validates these at save time |

Where a `CHECK` is omitted, the legal set is documented in the column comment and enforced by the
engine's validation pipeline.

**JSON columns and the derived-column rule.** Some tables store a canonical JSON body as the source
of truth and additionally extract a few columns for indexing. Those extracted columns are
**derived**, and derived columns obey one rule:

> A derived column is written only by the projection function that reads the body. Nothing else ever
> assigns to it. Dropping every derived column and recomputing it from the bodies must be a no-op.

I enforce this three ways: the projection is a single function per table; `afisharr db reproject`
recomputes all of them; and a test asserts, for every row in the database, that
`project(body_json)` equals the stored derived columns. Without that test, "hot columns for indexing
only" degrades into a second source of truth within about two releases.

Derived columns are marked **⟨derived⟩** in the DDL comments below.

**Foreign keys, and where they are deliberately absent.** `PRAGMA foreign_keys = ON` on every
connection. Cascades are declared explicitly and are almost always `ON DELETE CASCADE` for owned
children, `ON DELETE RESTRICT` for references that imply the parent is in use.

Three tables carry **no foreign key to the entity they describe**, and this is intentional:

- `lifecycle_transitions` — the audit log outlives its subject. A record explaining why a placeholder
  was deleted is worthless if it disappears when the subject is cleaned up. Identity is denormalised
  into the row instead (see the Lifecycle chapter of this document).
- `acquisition_decisions` — same reason, plus a grab decision must be replayable from the record alone
  (see *Lifecycle*).
- `placement_passes` — a pass record references participants that may since have been deleted from
  Plex by the user.

In each case the identifying columns are stored as plain `TEXT` with an index, and the join is a
best-effort left join in the GUI.

---

### 19.2 Storage layout

I split storage across a small number of locations, each chosen for what it stores:

| Data | Location | Why |
| --- | --- | --- |
| All structured records | `afisharr.db` (SQLite) | One file, one backup, transactional |
| Write-ahead log | `afisharr.db-wal`, `afisharr.db-shm` | Consequence of WAL (§19.4) |
| Asset bytes (posters, wallpapers, fonts, icons, rendered output, placeholder video) | `assets/<aa>/<bb>/<sha256>` on disk | See below |
| Application log | `logs/afisharr.log`, rotated | Text log for support; the GUI logs page reads structured run events from the database, not this file |
| Placeholder media files | User-configured library roots | These are the user's library, not mine to store |

**Asset bytes live on disk, not in the database.**

**Decision.** Asset content is stored content-addressed on the filesystem, sharded two levels by the
first four hex characters of its SHA-256. The database stores metadata and the digest.

**Why.** A library of 5,000 items with overlays applied implies a base poster and at least one
rendered poster per item — order of several gigabytes. Holding that in SQLite makes the database file
too large to copy quickly, which in turn makes the pre-migration backup (§19.3) too slow to be
automatic, which is the point at which people start skipping it. Blobs on disk also let the renderer
and the HTTP layer stream bytes without materialising them in memory, and deduplicate for free:
identical base posters across a 1080p and a 4K library are one file.

**The cost, stated plainly.** Backup is now two things that must be captured together, and a database
restored without its asset directory has dangling digests. The schema mitigates rather than hides
this: every asset row carries `verified_at` and `missing_since`, a startup sampling check and a
doctor-page check reconcile rows against files, and a missing base poster is a recoverable condition
(recapture from Plex) rather than a corrupt one. The non-functional requirements own the backup
procedure and must treat the pair as a unit.

**Rejected alternative.** Blobs in SQLite with `SQLITE_MAX_PAGE_COUNT` tuning. Simpler backup story,
but it trades a documented two-directory backup for an unbounded single file, and it forfeits
streaming. If a future deployment target makes filesystem access unreliable, the `assets` table is
already the indirection point at which a blob backend could be added.

---

### 19.3 Migration strategy

**There are two migration systems, and they never touch each other.**

| System | Migrates | Mechanism | Trigger |
| --- | --- | --- | --- |
| **Schema migrations** | Table shape | Numbered SQL files under `backend/crates/afisharr/migrations/`, run by `sqlx migrate` | Application startup |
| **Document migrations** | Definition body JSON | In-code, per `kind`, keyed on `schema_version` (see *The definition layer*) | On load of an individual definition |

**A schema migration never rewrites `body_json`.** This rule exists because the two systems have
incompatible failure modes: a SQL migration is all-or-nothing across every row, while a document
migration must handle one malformed body without failing the other 400. Bulk-rewriting bodies inside
a SQL migration means a single unmigratable definition blocks startup entirely.

The interaction is one-directional and explicit: a document migration that runs on load writes the
upgraded body back through the normal save path, which reprojects derived columns (§19.1). A schema
migration that adds a derived column populates it by calling the projection over existing bodies —
reading `body_json`, never writing it.

**Forward-only.** Migrations are forward-only. No `down` scripts ship.

A down migration that drops a column loses data; a down migration that preserves it is not a down
migration. The honest recovery path from a bad upgrade is *restore the pre-migration backup* (below),
which is a procedure that actually works, rather than a reverse script that has never been run
against real data. Development-time rollback is a fresh database.

**SQLite's `ALTER TABLE` limits.** SQLite supports `ADD COLUMN`, `RENAME`, and `DROP COLUMN` (with
restrictions), and nothing else. Changing a type, adding a `CHECK`, or altering a foreign key requires
the twelve-step rebuild:

```sql
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE t_new (...) STRICT;
INSERT INTO t_new SELECT ... FROM t;
DROP TABLE t;
ALTER TABLE t_new RENAME TO t;
-- recreate every index, trigger, and view on t
PRAGMA foreign_key_check;
COMMIT;
PRAGMA foreign_keys = ON;
```

Two obligations that are routinely forgotten and are therefore migration-review checklist items:
indexes are **not** carried over by `RENAME` and must be recreated explicitly, and
`PRAGMA foreign_key_check` must run before `COMMIT` while foreign keys are off, or a broken reference
is committed silently.

SQLite DDL is transactional, so a migration that fails mid-way rolls back cleanly. Each migration
file is one transaction; `sqlx migrate` provides this.

**Ordering and one-way doors.** Migration `0001` must create the database with the pragmas that can
only be set at creation:

```sql
PRAGMA auto_vacuum = INCREMENTAL;   -- MUST precede the first CREATE TABLE
PRAGMA journal_mode = WAL;          -- persistent, survives reopen
PRAGMA page_size = 8192;            -- only settable before first write or via VACUUM
```

`auto_vacuum` cannot be enabled later without a full `VACUUM` of the whole file, which on a
multi-gigabyte database is a long stall at the worst moment. It costs nothing to set now.

**Startup sequence.**

1. Open the database. If it does not exist, create it and run all migrations.
2. Read the applied-migration table. If it contains a version this binary does not know, **refuse to
   start** with a clear message naming the version and the binary's newest known migration. A
   downgrade running against a newer schema corrupts data quietly; refusing is the only safe answer.
3. If migrations are pending, copy `afisharr.db` to `backups/pre-migration-<version>-<timestamp>.db`
   using SQLite's online backup API (not a file copy — a file copy of a WAL database mid-write is not
   a valid database). Retain the last three.
4. Run pending migrations.
5. Run `PRAGMA foreign_key_check` and `PRAGMA integrity_check` on first start after a migration.
6. Reconcile unconfirmed lifecycle intents (§19.9) and expired leases (§19.4).

---

### 19.4 Concurrency model

**Pragmas.** Set on every pooled connection at acquisition:

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;      -- safe under WAL: survives process crash; a machine
                                  -- power loss can lose the last transactions, which is
                                  -- the correct trade for this workload
PRAGMA cache_size = -32000;       -- 32 MB per connection
PRAGMA temp_store = MEMORY;
PRAGMA foreign_keys = ON;
```

**One writer, structurally.** SQLite in WAL mode permits many concurrent readers and exactly one
writer. I do not discover this by catching `SQLITE_BUSY`; I make concurrent writes impossible by
construction:

- A read pool of N connections (`N = min(4, cores)`), opened read-only where SQLx permits.
- A **single** writer connection, owned by a write actor. All mutations are messages to that actor,
  which executes them serially and returns the result. There is no second path to a write.

`busy_timeout` remains set as a backstop against an external process (a backup tool, a `sqlite3`
shell) holding the lock, but the application never contends with itself. The alternative — a write
pool plus retry-on-busy — works, and then fails under load in a way that only reproduces on the
user's machine.

**Leases: preventing two passes, not two writes.** Serialising writes does not prevent two *logical*
operations interleaving — a scheduled collection sync and a manual "sync now" both running against the
same definition, each writing valid rows that together mean nothing. That is prevented by leases.

```sql
CREATE TABLE leases (
    name           TEXT    PRIMARY KEY,        -- 'pass:placement:lib_01J9Z…', 'job:overlay-sweep'
    owner          TEXT    NOT NULL,           -- process instance id + task id
    acquired_at    INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    heartbeat_at   INTEGER NOT NULL
) STRICT;
```

Acquisition is a single conditional insert-or-update, which is atomic because the writer is
serialised:

```sql
INSERT INTO leases (name, owner, acquired_at, expires_at, heartbeat_at)
VALUES (?1, ?2, ?3, ?4, ?3)
ON CONFLICT(name) DO UPDATE SET
    owner = excluded.owner, acquired_at = excluded.acquired_at,
    expires_at = excluded.expires_at, heartbeat_at = excluded.heartbeat_at
WHERE leases.expires_at < ?3;                  -- only steal an expired lease
```

Long tasks heartbeat; a task whose lease has expired must abort rather than complete, because another
holder may have started. Lease names are hierarchical so scope is explicit:
`pass:collection:<definition_id>`, `pass:placement:<library_id>` (placement is serialised per
library, per the placement model), `pass:lifecycle:<library_id>`, `job:<job_id>`, `setup:claim`.

Startup clears leases whose `owner` names this process instance (they are ours, from before the
crash) and leaves the rest to expire.

**`setup:claim` is a lease held by a browser rather than by a task.** The setup wizard is exclusive
to one browser for ten minutes at a time (§19.6.1), and that is the same property this table already
provides: one holder, an expiry, and a steal that only fires once the expiry passes. Its `owner`
column carries the SHA-256 of the claim cookie value rather than a process instance id, so startup's
"clear leases owned by this process" step never matches it and the claim survives a restart until it
expires on its own. Renewal is a heartbeat that also moves `expires_at`. No second table exists for
this, and no plaintext credential is stored: the row proves a claim the same way `sessions` proves a
session.

**No transaction spans network I/O.**

**Rule.** A database transaction may not remain open across an HTTP call, a filesystem write to a
library root, or any other external I/O.

A collection sync takes minutes and makes hundreds of calls. Holding a write transaction across it
blocks every other writer for the duration, and a hung socket becomes a hung application. Passes
therefore run as: read a consistent snapshot, do the I/O, commit results in short transactions at
defined checkpoints. Partial progress is the normal case and every pass is designed to be resumable —
which is the same property the lifecycle model already requires for crash safety, so the cost is
already paid.

**Optimistic concurrency on definitions.** A pass reads a definition, works for two minutes, and
writes results. Meanwhile the user edits it in the GUI. Resolution:

- `definitions.body_hash` is the concurrency token.
- The GUI save is a compare-and-swap: `UPDATE … WHERE id = ? AND body_hash = ?`. Zero rows affected
  means someone else saved first, and the GUI is told so, showing a diff rather than silently
  clobbering.
- A pass records the `body_hash` it read. At commit, if the definition's hash has changed, the pass
  **discards its results for that definition** and re-queues it. It does not merge.

**The user's write always wins.** A background pass losing two minutes of work is an inconvenience; a
background pass overwriting an edit the user just made is the kind of bug that makes people stop
trusting the tool.

---

### 19.5 Instance, settings, secrets, and versioned policy

**Instance identity.**

```sql
CREATE TABLE instance (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    instance_id         TEXT    NOT NULL,             -- ULID, this installation
    client_identifier   TEXT    NOT NULL,             -- X-Plex-Client-Identifier. IMMUTABLE.
    device_name         TEXT    NOT NULL,
    timezone            TEXT    NOT NULL,             -- IANA, e.g. 'Europe/London'
    locale              TEXT    NOT NULL DEFAULT 'en',
    app_version         TEXT    NOT NULL,             -- last binary that opened this database
    first_started_at    INTEGER NOT NULL,
    setup_completed_at  INTEGER,                      -- NULL until the wizard finishes
    setup_acked_steps   TEXT    NOT NULL DEFAULT '[]' -- JSON array of acknowledged wizard steps
                        CHECK (json_valid(setup_acked_steps)),
    updated_at          INTEGER NOT NULL
) STRICT;
```

`client_identifier` is generated once and **never regenerated**. plex.tv binds tokens to it; a new
value makes every existing token belong to a device the user has never seen, and orphans the old
device in their account's device list. I call it out here, in the schema, because it is the kind of
value a well-meaning "reset configuration" feature deletes.

`timezone` is instance-level because the engine's date operators are day-aligned in it, and the
lifecycle model computes phase from a civil-date difference. Changing it changes what existing
definitions mean, so a change is recorded in `settings_history`.

`setup_completed_at` and `setup_acked_steps` carry onboarding state, and they are here rather than in
`settings` for two reasons. They are installation facts, not configuration the operator tunes, and
the startup path reads `setup_completed_at` before the settings body is deserialised — the console
banner in §19.6.1 depends on it. `setup_acked_steps` exists because two wizard steps complete by
acknowledgement rather than by writing configuration: choosing no starter packs is a valid choice,
and the existing-collections report writes nothing at all (D-026). Without a recorded
acknowledgement, a resume derived from state alone could never move past either one. The column is a
JSON array of step names, never a partial-progress percentage.

**Settings.**

```sql
CREATE TABLE settings (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    version     INTEGER NOT NULL,
    body_json   TEXT    NOT NULL CHECK (json_valid(body_json)),
    updated_at  INTEGER NOT NULL,
    updated_by  TEXT
) STRICT;

CREATE TABLE settings_history (
    version     INTEGER PRIMARY KEY,
    body_json   TEXT    NOT NULL CHECK (json_valid(body_json)),
    changed_at  INTEGER NOT NULL,
    changed_by  TEXT,
    diff_json   TEXT    CHECK (diff_json IS NULL OR json_valid(diff_json))
) STRICT;
```

One row, one JSON body, deserialised into a typed Rust struct with `#[serde(deny_unknown_fields)]`. A
single body rather than a key-value table because settings are read as a unit at pass start, are
written as a unit by the settings page, and validate as a unit (several settings are only meaningful
together — a placeholder root is meaningless without the library it belongs to). Key-value settings
tables produce partial writes that no validator ever sees.

Per-definition overrides live in the definition body — all are per-definition with instance-level
defaults, per the lifecycle model's policy rules. Resolution is: definition value if present, else
instance value. Never a third layer.

**Secrets.**

```sql
CREATE TABLE secrets (
    name        TEXT PRIMARY KEY,               -- 'plex.token', 'tmdb.apiKey', 'trakt.refresh'
    ciphertext  BLOB    NOT NULL,
    nonce       BLOB    NOT NULL,
    algorithm   TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    last_used_at INTEGER
) STRICT;
```

Credentials are **not** in `settings.body_json`. Three reasons that each independently justify the
split: the settings body is diffed into `settings_history` and would preserve rotated secrets
forever; the settings body is a candidate for export and support bundles; and encryption at rest
applies to a narrow, well-defined table rather than to a blob that also holds a hundred harmless
booleans. Key management belongs to the non-functional requirements; the isolation is this schema's
job.

**Versioned policy and registry.** The lifecycle model requires that `policyVersion` on an audit
record keeps historical decisions interpretable after a settings change, and the engine requires
`registryVersion` to distinguish a definition that is wrong from one written against an older
vocabulary. Both promises are empty unless the old policy and the old registry are still readable.

```sql
CREATE TABLE lifecycle_policies (
    version     INTEGER PRIMARY KEY,            -- monotonic, bumped on any lifecycle-policy change
    body_json   TEXT    NOT NULL CHECK (json_valid(body_json)),
    created_at  INTEGER NOT NULL,
    created_by  TEXT
) STRICT;

CREATE TABLE registry_versions (
    version         INTEGER PRIMARY KEY,
    app_version     TEXT    NOT NULL,
    manifest_hash   TEXT    NOT NULL,           -- digest of the full static registry
    manifest_json   TEXT    NOT NULL CHECK (json_valid(manifest_json)),
    activated_at    INTEGER NOT NULL
) STRICT;
```

These tables are append-only and never trimmed. They are small — one row per policy change, one per
registry revision — and pruning them destroys the interpretability the version numbers exist to
provide.

`lifecycle_policies` holds the *effective* policy snapshot (instance defaults resolved), so a recorded
`policy_version` identifies exactly the values a decision was made under, without a join back through
per-definition overrides that may themselves have changed.

Whether `registry_versions.manifest_json` is populated from compiled-in constants or from a loaded
data file is open (D-016). The table is identical either way, which is why the schema does not block
that decision.

---

### 19.6 Identity and access

Tier 0 is an admin-only surface. The obligation I carry forward is nevertheless concrete: **the
schema must support per-user targeting on day one, so Tier 1 is a widening rather than a migration.**
I-DATA-5 is the test for that claim, and it ships in the launch release even though nothing uses the
capability.

**Principals.**

```sql
CREATE TABLE principals (
    id               TEXT PRIMARY KEY,                  -- ULID; three seeded rows have fixed ULIDs
    kind             TEXT NOT NULL CHECK (kind IN ('Everyone','Owner','SharedAll','PlexUser','LocalUser')),
    plex_account_id  INTEGER,                           -- plex.tv numeric account id, PlexUser only
    plex_uuid        TEXT,                              -- plex.tv account uuid, PlexUser only
    user_id          TEXT REFERENCES users(id) ON DELETE CASCADE,   -- LocalUser only
    label            TEXT NOT NULL,
    created_at       INTEGER NOT NULL,
    CHECK ((kind = 'PlexUser')  = (plex_account_id IS NOT NULL)),
    CHECK ((kind = 'LocalUser') = (user_id IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_principals__plex ON principals(plex_account_id) WHERE plex_account_id IS NOT NULL;
```

Three rows are seeded by migration `0002` with fixed identifiers: `Everyone`, `Owner`, `SharedAll`.
These are the whole-audience values the Tier 0 GUI may write. `PlexUser` and `LocalUser` rows are
*creatable from day one* — the machinery exists and is exercised by tests — but no Tier 0 GUI control
produces one.

This is what makes the per-user-targeting claim testable. The Tier 1 per-user feature adds `PlexUser`
rows and `placement_visibility` rows referencing them. If it needs `ALTER TABLE`, the claim was not
honoured, and I-DATA-5 is the test that catches it.

**Users, sessions, API keys.**

```sql
CREATE TABLE users (
    id                TEXT PRIMARY KEY,
    kind              TEXT NOT NULL CHECK (kind IN ('Local','Plex')),
    username          TEXT NOT NULL,
    email             TEXT,
    display_name      TEXT,
    password_hash     TEXT,                       -- Argon2id PHC string; Local only
    plex_account_id   INTEGER,                    -- Plex only
    plex_uuid         TEXT,
    avatar_url        TEXT,
    is_admin          INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0,1)),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    last_login_at     INTEGER,
    disabled_at       INTEGER,
    CHECK ((kind = 'Local') = (password_hash IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_users__username ON users(username);
CREATE UNIQUE INDEX ux_users__plex     ON users(plex_account_id) WHERE plex_account_id IS NOT NULL;

CREATE TABLE sessions (
    id              TEXT PRIMARY KEY,             -- SHA-256 of the cookie value, never the value
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL,
    last_seen_at    INTEGER NOT NULL,
    user_agent      TEXT,
    ip              TEXT,
    revoked_at      INTEGER
) STRICT;

CREATE INDEX ix_sessions__user    ON sessions(user_id);
CREATE INDEX ix_sessions__expiry  ON sessions(expires_at);

CREATE TABLE api_keys (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    key_hash      TEXT NOT NULL,                  -- SHA-256 of the key
    prefix        TEXT NOT NULL,                  -- first 8 chars, for display and lookup
    created_at    INTEGER NOT NULL,
    created_by    TEXT REFERENCES users(id) ON DELETE SET NULL,
    last_used_at  INTEGER,
    revoked_at    INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_api_keys__hash ON api_keys(key_hash);
```

Session identifiers and API keys are stored hashed. The plaintext is shown once at creation and is
not recoverable; a database read must not yield a working credential.

**Plex PIN login.** The plex.tv PIN and OAuth flows both create a pin resource, present a code or a
URL to the user, and poll until a token appears or the pin expires. That is a multi-request flow with
server-side state, so it needs a row:

```sql
CREATE TABLE plex_pin_logins (
    id                TEXT PRIMARY KEY,
    plex_pin_id       TEXT NOT NULL,              -- id returned by plex.tv
    code              TEXT NOT NULL,              -- 4-character link code
    mode              TEXT NOT NULL CHECK (mode IN ('Pin','OAuth')),
    client_identifier TEXT NOT NULL,              -- must equal instance.client_identifier
    created_at        INTEGER NOT NULL,
    expires_at        INTEGER NOT NULL,
    consumed_at       INTEGER,
    result            TEXT CHECK (result IS NULL OR result IN ('Success','Expired','Aborted'))
) STRICT;
```

The returned token goes to `secrets`, never here. Rows are deleted an hour after `expires_at`.
`client_identifier` is stored per-row and checked against the instance value, because a pin issued
under a different client identifier yields a token that will not work and the failure is otherwise
opaque.

#### 19.6.1 First-run bootstrap and the setup claim

D-029 assumes the instance may be reachable from the internet. On such an instance, "the first person
to load the page becomes the administrator" is not a first-run flow — it is a race, and the operator
loses it to anyone who scans the port first. The wizard then hands that stranger a Plex token, which
§21.4.1 names the crown jewel because it authorises deletion. Two mechanisms close this, recorded as
D-045 and D-046.

**The bootstrap token proves console access.** The claim converts that one-time proof into an
exclusive, time-boxed lease on the wizard, bound to one browser.

**The token.**

| Property | Value |
| --- | --- |
| Shape | `xxxx-xxxx-xxxx` — three segments of four characters |
| Alphabet | 36 characters, lowercase ASCII letters and digits |
| Entropy | 12 × log₂(36) ≈ 62 bits |
| Source | OS CSPRNG, with rejection sampling |
| Storage | Process memory only |
| Lifetime | 15 minutes |

It is generated on startup whenever `instance.setup_completed_at` is `NULL`, printed to stdout with
the setup URL, and held in memory for fifteen minutes. It never reaches the database, never reaches
`logs/afisharr.log`, and never reaches the client. Only one token is live at a time; minting replaces
any predecessor.

**Rejection sampling is not a detail.** Bytes at or above `252` — the largest multiple of 36 that
fits in a byte — are discarded and redrawn rather than reduced modulo 36. Without that, the first
four characters of the alphabet appear more often than the rest, and the 62 bits above are a claim
the generator does not honour.

**Three events end a token's life:** its fifteen minutes elapse; the process restarts, since the
value lives in memory and a restart prints a new banner; or setup completes, which clears it. The
banner states all three. Horizontal replicas are not a supported deployment (D-037, and the write
actor in §19.4 already assumes one process), so there is no case where one replica prints a token a
second replica cannot verify.

**Validation, not consumption.** A submitted token is checked and left live. The check is ordered:
a token must exist, it must be unexpired, its length must match, and only then is it compared in
constant time against the live value. Length is checked first because it bounds the work a caller can
force; the comparison is constant-time because a byte-at-a-time compare leaks the mismatch position.
Leaving the token live is what makes a lost claim cookie recoverable inside the fifteen-minute
window; consuming it would strand the operator on their own console.

**The claim.** One browser holds the wizard at a time, for ten minutes, sliding on every gated
request:

| Where | What | Meaning |
| --- | --- | --- |
| `leases` row named `setup:claim` | `owner` = SHA-256 of the cookie value | the claim itself (§19.4) |
| | `expires_at` | ten minutes from the last gated request |
| Browser cookie `afisharr_setup_claim` | the cookie value | the holder's half of the pair |

A claim is active when the lease row is unexpired **and** the request's cookie hashes to its `owner`.
Both halves are required, so a stolen database row proves nothing and a stolen cookie outlives its
lease by nothing.

Cookie flags are `HttpOnly`, `Secure` (over HTTPS, judged by the trusted-proxy list of §21.4.3),
`SameSite=Lax`, `Path=/api/setup`, `Max-Age=600`. `Lax` rather than `Strict` because the Plex OAuth
variant may return the operator by top-level navigation, and `Strict` would withhold the cookie on
exactly that request; CSRF protection is always on regardless (§21.4.2), so `Lax` costs nothing here.

**Ten minutes, against the token's fifteen, is deliberate.** The claim must expire while the token
that created it is still usable. An operator whose browser died at step 3 waits out the claim and
re-enters a token they already have. Reverse the two and they wait for the claim, then discover the
token expired while they waited, and must restart the container to get another.

**Renewal is not a separate mechanism.** Every claim-gated request that succeeds moves `expires_at`
ten minutes out and re-sets the cookie's `Max-Age`. An operator who keeps working never meets the
timeout; one who walks away releases the wizard without doing anything.

**Recovery once an admin exists.** From the moment the admin account is created, the wizard accepts
a second credential: those admin credentials. Presenting them when setup is incomplete and no claim
is active mints a claim without the token. This is what makes an interrupted setup survive a restart
— the token is gone, but the account is not.

**Release.** Completing the wizard writes `instance.setup_completed_at`, deletes the `setup:claim`
lease, clears the in-memory token, and expires the cookie. From then on the banner prints nothing on
restart and the setup endpoints return 404 rather than a form.

**Where setup events land.** Each wizard step appends a `job_run_events` row under one `job_runs` row
whose `trigger` is `Api` (§19.15) — claim taken, admin created, Plex connected, libraries selected,
integrations configured, packs chosen, collections reported, setup completed. They are **not** written
to the lifecycle audit record: §21.4.8 states that the audit exists to explain what the engine did,
not as forensics against the operator, and setup steps are operator actions. The logs page therefore
reads them with the filters it already has, and no new surface is invented for them.

**What this does not protect against.** An attacker who can read the server's console output can
claim the instance, and so can one who can read the process's memory. Both already imply a
compromised host, which §21.4.5 states is outside what encryption at rest defends. The mechanism
raises the bar from "reachable" to "console-readable", which is the boundary that actually separates
the operator from everyone else.

---

### 19.7 Plex topology and the library cache

**Server.**

```sql
CREATE TABLE plex_server (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    machine_identifier  TEXT    NOT NULL,
    friendly_name       TEXT    NOT NULL,
    version             TEXT    NOT NULL,       -- drives discovered-field invalidation (§19.8)
    platform            TEXT,
    base_url            TEXT    NOT NULL,
    owner_account_id    INTEGER,
    first_seen_at       INTEGER NOT NULL,
    last_seen_at        INTEGER NOT NULL,
    last_version_change_at INTEGER
) STRICT;
```

One row. A changed `machine_identifier` means the user pointed the installation at a *different
server*, which invalidates every rating key, every discovered field, and every adoption in the
database. That is not silently reconciled: the server is treated as unrecognised, all Plex-bound
state is marked suspect, and the doctor page requires an explicit "this is a new server, rebind" or
"restore a backup" decision. Auto-healing across a server swap would rewrite the user's library based
on identifiers that mean something else entirely.

**Libraries.**

```sql
CREATE TABLE libraries (
    id                    TEXT PRIMARY KEY,
    handle                TEXT NOT NULL,          -- stable slug; what definitions reference
    section_key           TEXT NOT NULL,          -- Plex section id; may change
    section_uuid          TEXT,                   -- Plex section uuid; stable across key change
    type                  TEXT NOT NULL CHECK (type IN ('movie','show')),
    title                 TEXT NOT NULL,
    agent                 TEXT,
    language              TEXT,
    is_managed            INTEGER NOT NULL DEFAULT 0 CHECK (is_managed IN (0,1)),
    item_count            INTEGER NOT NULL DEFAULT 0,
    scanned_at            INTEGER,                -- Plex's last scan we observed
    cache_refreshed_at    INTEGER,                -- our last full item-cache pass
    fields_discovered_at  INTEGER,
    fields_plex_version   TEXT,
    created_at            INTEGER NOT NULL,
    last_seen_at          INTEGER NOT NULL,
    missing_since         INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_libraries__handle ON libraries(handle);
CREATE UNIQUE INDEX ux_libraries__uuid   ON libraries(section_uuid) WHERE section_uuid IS NOT NULL;
CREATE INDEX ix_libraries__managed       ON libraries(is_managed) WHERE is_managed = 1;
```

Definitions reference libraries by `handle` (the engine shows `"libraries": ["movies", "movies-4k"]`
as the example shape), so the handle is immutable once created — renaming the library in Plex changes
`title`, never `handle`. Rebinding after a section-key change matches on `section_uuid` first, then on
`(type, title)` with confirmation.

`music` and `photo` library types are non-goals and are not representable. They are filtered at
discovery and never inserted, rather than inserted and ignored — an unrepresentable state cannot be
reached by a bug.

**Library items.**

```sql
CREATE TABLE library_items (
    id                TEXT PRIMARY KEY,
    library_id        TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    rating_key        TEXT NOT NULL,
    parent_item_id    TEXT REFERENCES library_items(id) ON DELETE CASCADE,   -- season → show, episode → season
    type              TEXT NOT NULL CHECK (type IN ('movie','show','season','episode')),
    guid              TEXT,                                -- Plex primary guid
    title             TEXT NOT NULL,
    sort_title        TEXT,
    year              INTEGER,
    index_number      INTEGER,                             -- season/episode number
    originally_available_date TEXT,                        -- civil date
    added_at          INTEGER,
    plex_updated_at   INTEGER,
    has_media         INTEGER NOT NULL DEFAULT 0 CHECK (has_media IN (0,1)),
    is_placeholder    INTEGER NOT NULL DEFAULT 0 CHECK (is_placeholder IN (0,1)),
    metadata_hash     TEXT NOT NULL,                       -- digest of tracked metadata fields
    first_seen_at     INTEGER NOT NULL,
    last_seen_at      INTEGER NOT NULL,
    missing_since     INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_library_items__key    ON library_items(library_id, rating_key);
CREATE INDEX ix_library_items__parent        ON library_items(parent_item_id) WHERE parent_item_id IS NOT NULL;
CREATE INDEX ix_library_items__placeholder   ON library_items(library_id) WHERE is_placeholder = 1;
CREATE INDEX ix_library_items__live          ON library_items(library_id, type) WHERE missing_since IS NULL;
```

**`is_placeholder` is the schema-level marker the lifecycle model requires at §5.1.** Hub replacement
filters on it, orphan sweeps start from it, and the `item.isPlaceholder` registry field reads it. The
partial index makes "every placeholder in this library" a scan of exactly the placeholders. Note what
it is *not*: it is not derived from the filename, and it is not derived from the Plex label. It is set
when the item is materialised and cleared when the item becomes real, both inside the transaction that
records the lifecycle transition. The Plex label is the runtime marker for things that read Plex
directly; this column is the durable one, and where they disagree the doctor page reports it.

Items are **soft-deleted** via `missing_since`. An item that vanishes from Plex for one pass may be
mid-scan; hard deletion on first absence destroys base posters and lifecycle bindings for a title that
is about to reappear. A row missing for longer than the reaping window is hard-deleted by the
maintenance job.

**External identifiers.**

```sql
CREATE TABLE library_item_ids (
    library_item_id TEXT NOT NULL REFERENCES library_items(id) ON DELETE CASCADE,
    id_space        TEXT NOT NULL,                   -- 'tmdb','tvdb','imdb','anidb','mal','anilist','plex'
    id_value        TEXT NOT NULL,
    source          TEXT NOT NULL,                   -- 'plexGuid','agent','mapping','manual'
    recorded_at     INTEGER NOT NULL,
    PRIMARY KEY (library_item_id, id_space, id_value)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_library_item_ids__lookup ON library_item_ids(id_space, id_value);
```

`WITHOUT ROWID` because the table is a pure composite-key mapping, read by key, and never joined on a
rowid; it saves the redundant index and roughly a third of the space.

Resolution of an external identifier to a library item is a lookup on `ix_library_item_ids__lookup`
joined to `library_items` filtered by library. **More than one match within a library is the
ambiguity condition** described for the lifecycle model's guardrail G7, recorded rather than guessed:

```sql
CREATE TABLE ambiguous_matches (
    id                  TEXT PRIMARY KEY,
    library_id          TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    id_space            TEXT NOT NULL,
    id_value            TEXT NOT NULL,
    candidates_json     TEXT NOT NULL CHECK (json_valid(candidates_json)),
    detected_at         INTEGER NOT NULL,
    last_seen_at        INTEGER NOT NULL,
    resolved_item_id    TEXT REFERENCES library_items(id) ON DELETE SET NULL,
    resolved_at         INTEGER,
    resolved_by         TEXT
) STRICT;

CREATE UNIQUE INDEX ux_ambiguous__key ON ambiguous_matches(library_id, id_space, id_value);
```

An unresolved row blocks the subject from every action (guardrail G7). A resolved row pins the choice:
the next pass reads `resolved_item_id` and proceeds without re-detecting. Where the operator resolves
it — doctor page or inline in the editor — is settled by D-013; the schema serves both, and
`resolved_by` records which surface was used.

**Cross-provider identifier mapping.** Anime sources speak AniList and MAL identifiers while Plex
speaks TVDB or TMDB. The mapping is bulk-imported reference data, not per-item state:

```sql
CREATE TABLE id_mappings (
    from_space   TEXT NOT NULL,
    from_value   TEXT NOT NULL,
    to_space     TEXT NOT NULL,
    to_value     TEXT NOT NULL,
    season       INTEGER NOT NULL DEFAULT -1,       -- -1 is the whole title; a PK column of a
                                                    -- STRICT, WITHOUT ROWID table is NOT NULL
    dataset      TEXT NOT NULL,                    -- which mapping dataset supplied this
    imported_at  INTEGER NOT NULL,
    PRIMARY KEY (from_space, from_value, to_space, season)
) STRICT, WITHOUT ROWID;
```

Stored separately from `library_item_ids` because it is a fact about the world, refreshed wholesale on
a schedule, while `library_item_ids` is a fact about this user's library. Mixing them means a dataset
refresh rewrites per-item state.

**Item state snapshot.** The overlay render key depends on a state snapshot, and the filter engine
reads `media.*` fields constantly. Both want a precomputed, hashable projection:

```sql
CREATE TABLE library_item_state (
    library_item_id TEXT PRIMARY KEY REFERENCES library_items(id) ON DELETE CASCADE,
    facts_json      TEXT    NOT NULL CHECK (json_valid(facts_json)),  -- resolved media.* / item.* values
    facts_hash      TEXT    NOT NULL,
    ratings_json    TEXT    CHECK (ratings_json IS NULL OR json_valid(ratings_json)),
    ratings_hash    TEXT,
    ratings_fetched_at INTEGER,
    state_hash      TEXT    NOT NULL,             -- ⟨derived⟩ digest over facts+ratings+lifecycle
    computed_at     INTEGER NOT NULL
) STRICT;

CREATE INDEX ix_library_item_state__stale ON library_item_state(computed_at);
```

`ratings_*` are separated from `facts_*` because they have a different refresh cadence and a different
availability class (`integration`, not `always`, per the engine's field classes). A rating fetch
failure must leave `facts_json` untouched, and a `NULL` `ratings_json` means *unavailable* while a
JSON `null` inside it means *known to have no value* — a distinction the engine requires to be
preserved, expressed in storage rather than reconstructed.

---

### 19.8 Discovered field cache

The engine's field registry is two-layered: a static core compiled with the app, and a layer
discovered from Plex per library. Only the discovered layer is persisted — the static core is the same
for every installation of a given release and is identified by `registry_versions.manifest_hash`.

```sql
CREATE TABLE discovery_snapshots (
    id             TEXT PRIMARY KEY,
    library_id     TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    plex_version   TEXT NOT NULL,
    fetched_at     INTEGER NOT NULL,
    status         TEXT NOT NULL CHECK (status IN ('Ok','Partial','Failed')),
    error          TEXT,
    is_current     INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0,1))
) STRICT;

CREATE UNIQUE INDEX ux_discovery__current ON discovery_snapshots(library_id) WHERE is_current = 1;

CREATE TABLE discovered_fields (
    snapshot_id   TEXT NOT NULL REFERENCES discovery_snapshots(id) ON DELETE CASCADE,
    libtype       TEXT NOT NULL,                  -- movie, show, season, episode, collection
    field_key     TEXT NOT NULL,                  -- Plex's key, e.g. 'genre', 'audioLanguage'
    title         TEXT,
    field_type    TEXT NOT NULL,                  -- Plex's declared type
    sub_type      TEXT,
    ops_json      TEXT NOT NULL CHECK (json_valid(ops_json)),   -- operator keys legal for this type
    has_choices   INTEGER NOT NULL DEFAULT 0 CHECK (has_choices IN (0,1)),
    PRIMARY KEY (snapshot_id, libtype, field_key)
) STRICT, WITHOUT ROWID;

CREATE TABLE discovered_field_choices (
    snapshot_id  TEXT NOT NULL REFERENCES discovery_snapshots(id) ON DELETE CASCADE,
    libtype      TEXT NOT NULL,
    field_key    TEXT NOT NULL,
    value        TEXT NOT NULL,
    title        TEXT,
    fast_key     TEXT,                            -- Plex's direct-match key for this choice
    PRIMARY KEY (snapshot_id, libtype, field_key, value)
) STRICT, WITHOUT ROWID;

CREATE TABLE discovered_sorts (
    snapshot_id  TEXT NOT NULL REFERENCES discovery_snapshots(id) ON DELETE CASCADE,
    libtype      TEXT NOT NULL,
    sort_key     TEXT NOT NULL,
    title        TEXT,
    default_direction TEXT,
    PRIMARY KEY (snapshot_id, libtype, sort_key)
) STRICT, WITHOUT ROWID;
```

**Snapshot-scoped, not library-scoped.** Discovery writes a new snapshot and flips `is_current` in one
transaction. A failed or partial discovery therefore cannot leave the cache half-rewritten, and the
previous snapshot remains usable — which matters because the inclination on discovery failure (D-017)
is warn-and-fall-back rather than block, and falling back requires something to fall back to. The two
most recent non-current snapshots are retained for diagnosis; older ones are deleted, which cascades
to their fields and choices.

**Invalidation** (D-017) is driven by three observable events, each recorded: a library scan
(`libraries.scanned_at` advances), a Plex version change (`plex_server.version` differs from
`discovery_snapshots.plex_version`), and an explicit doctor-page refresh. There is no TTL — a TTL
either refetches constantly or serves stale data, and both events that actually change the vocabulary
are already observable.

**A definition referencing a discovered field records the library it was authored against**, per the
engine's field-authoring rule. That record is `definition_field_uses`:

```sql
CREATE TABLE definition_field_uses (
    definition_id   TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    field_key       TEXT NOT NULL,
    layer           TEXT NOT NULL CHECK (layer IN ('Static','Discovered')),
    authored_library_id TEXT REFERENCES libraries(id) ON DELETE SET NULL,
    json_pointer    TEXT NOT NULL,               -- where in the body, for precise GUI highlighting
    PRIMARY KEY (definition_id, json_pointer)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_definition_field_uses__field ON definition_field_uses(field_key);
```

⟨derived⟩ in full — reprojected on every save. It answers, cheaply, the two questions that are
otherwise full-table JSON scans: "which definitions break if this field disappears?" (asked after
every discovery) and "which definitions must fall back to local evaluation on this library?" (asked
every pass). `json_pointer` is what lets the GUI highlight the offending control rather than showing a
paragraph, as the engine's error-surfacing rule requires.

**User-defined computed fields.** Decided on 2026-08-08, in the restricted form D-018 describes: **one
arithmetic operation over two registered numeric fields.** This is a capability the frozen ledger does
not contain and therefore needed a dated change request: CR-1, decided as D-018.

A computed field is a registry entry the user creates, so unlike the static core it is user data and
must be stored, versioned, and validated like any other definition-adjacent record.

```sql
CREATE TABLE computed_fields (
    id                TEXT PRIMARY KEY,
    key               TEXT NOT NULL,             -- always 'user.<slug>'
    label             TEXT NOT NULL,
    description       TEXT,
    operation         TEXT NOT NULL CHECK (operation IN ('add','subtract','multiply','divide')),
    left_field_key    TEXT NOT NULL,             -- a registered numeric field, never computed
    right_field_key   TEXT NOT NULL,
    result_type       TEXT NOT NULL CHECK (result_type IN ('number','integer')),
    null_policy       TEXT NOT NULL CHECK (null_policy IN ('Null','Zero')),
    availability      TEXT NOT NULL CHECK (availability IN ('always','integration','derived')),
    registry_version  INTEGER NOT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    created_by        TEXT,
    deleted_at        INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_computed_fields__key ON computed_fields(key);
CREATE INDEX ix_computed_fields__live       ON computed_fields(key) WHERE deleted_at IS NULL;
```

Six constraints keep this from becoming the string DSL the engine's design forbids. Each is a
validation rule, and each exists because its absence is a step down that road:

1. **Operands must be registered, non-computed numeric fields.** A computed field may not reference
   another computed field. This is the load-bearing rule: permit nesting and users compose arbitrary
   expression trees one field at a time, arriving at a DSL by increments with no decision ever having
   been made.
2. **Exactly one operation.** No parentheses, no third operand, no constants — a constant operand is
   the next request and the first genuine step toward an expression language, so it is refused now
   rather than argued about later.
3. **The `user.` namespace is closed to computed fields.** Collision with the static core or the
   `plex.*` discovered layer becomes impossible by construction rather than by precedence rules.
4. **`null_policy` is explicit and defaults to `Null`.** If either operand is null or unavailable, the
   result is null and any element bound to it is skipped, per the engine's null-handling rule. `Zero`
   is offered because it is occasionally what a user means, but it is never the default: a rating gap
   computed on a server where Rotten Tomatoes is not configured must not render as `0`, which is a
   plausible-looking wrong answer rather than a visible absence. Division by zero yields null under
   both policies.
5. **`availability` is derived from the operands** — `integration` if either operand is, so pack
   `requiresFields` resolution and the degraded-install warning keep working through a computed field.
6. **Deletion is a tombstone.** The engine forbids reusing a removed key, ever. The row is retained
   with `deleted_at` set, and the unique index covers deleted rows so the key cannot come back meaning
   something else. Deleting a computed field with inbound uses requires the same cascade choice as any
   other reference; `ix_definition_field_uses__field` already answers "who uses this".

There is no `registry_fields` table for the static core, and none for the discovered layer beyond this
section's snapshot cache. The three layers stay physically separate because they have different
lifetimes and different authorities: the core is fixed per release, the discovered layer is cache and
is thrown away on invalidation, and only this table holds anything a user would lose.

---

### 19.9 Definitions

**The definitions table.**

```sql
CREATE TABLE definitions (
    id                 TEXT PRIMARY KEY,
    kind               TEXT NOT NULL,                          -- ⟨derived⟩ see the engine's kind list
    handle             TEXT NOT NULL,                          -- ⟨derived⟩ 'namespace/slug'
    name               TEXT NOT NULL,                          -- ⟨derived⟩
    schema_version     INTEGER NOT NULL,                       -- ⟨derived⟩
    registry_version   INTEGER NOT NULL,                       -- ⟨derived⟩
    body_json          TEXT NOT NULL CHECK (json_valid(body_json)),
    body_hash          TEXT NOT NULL,                          -- ⟨derived⟩ digest of canonical body
    body_version       INTEGER NOT NULL DEFAULT 1,             -- increments on each accepted save
    origin_kind        TEXT NOT NULL CHECK (origin_kind IN ('User','Pack')),   -- ⟨derived⟩
    origin_pack        TEXT REFERENCES packs(namespace) ON DELETE SET NULL,    -- ⟨derived⟩
    origin_pack_version TEXT,                                  -- ⟨derived⟩
    forked_from        TEXT REFERENCES definitions(id) ON DELETE SET NULL,
    is_enabled         INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    updated_by         TEXT
) STRICT;

CREATE UNIQUE INDEX ux_definitions__handle ON definitions(handle);
CREATE INDEX ix_definitions__kind          ON definitions(kind, is_enabled);
CREATE INDEX ix_definitions__pack          ON definitions(origin_pack) WHERE origin_pack IS NOT NULL;
```

`body_json` is the single source of truth, exactly as the engine's document model promises. Every
other column except `id`, `body_version`, `is_enabled`, and the audit columns is ⟨derived⟩ under the
derived-column rule (§19.1) — extracted for indexing, never authoritative, fully recomputable.

`is_enabled` is **not** derived. It is operational state (the user paused this collection) rather than
document content, and putting it in the body would make pausing a collection a document edit that
dirties history and forks pack-origin definitions.

`kind` carries no `CHECK` despite being a closed set, because adding a kind is a schema-version event
already governed by the engine's versioning rules and would otherwise force a table rebuild for a
value the validator already rejects.

**History.**

```sql
CREATE TABLE definition_history (
    definition_id  TEXT NOT NULL,
    body_version   INTEGER NOT NULL,
    body_json      TEXT NOT NULL CHECK (json_valid(body_json)),
    body_hash      TEXT NOT NULL,
    changed_at     INTEGER NOT NULL,
    changed_by     TEXT,
    change_note    TEXT,
    PRIMARY KEY (definition_id, body_version)
) STRICT, WITHOUT ROWID;
```

No foreign key to `definitions`: a deleted definition's history is retained for the retention window
so "what did that collection do before I deleted it" is answerable. The last 20 versions per
definition are kept.

**Reference graph and library targeting.**

```sql
CREATE TABLE definition_refs (
    from_definition_id TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    to_definition_id   TEXT NOT NULL,
    json_pointer       TEXT NOT NULL,
    pinned_version     INTEGER,
    PRIMARY KEY (from_definition_id, json_pointer)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_definition_refs__inbound ON definition_refs(to_definition_id);

CREATE TABLE definition_libraries (
    definition_id TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    library_id    TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    PRIMARY KEY (definition_id, library_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_definition_libraries__library ON definition_libraries(library_id);
```

Both ⟨derived⟩. `ix_definition_refs__inbound` is what makes "deleting a definition with inbound
references requires an explicit cascade choice" (a rule the engine states) a single indexed query
rather than a scan of every body in the database.

`definition_libraries` is the structural replacement for the deleted linked-collections concept from
the frozen ledger. Multi-library fan-out is a row per library, and "which definitions target this
library" — asked at the start of every pass — is an index seek.

**Validation state.**

```sql
CREATE TABLE definition_validations (
    definition_id     TEXT PRIMARY KEY REFERENCES definitions(id) ON DELETE CASCADE,
    body_hash         TEXT NOT NULL,              -- which body this verdict applies to
    registry_version  INTEGER NOT NULL,
    status            TEXT NOT NULL CHECK (status IN ('Valid','Degraded','Invalid')),
    issues_json       TEXT NOT NULL CHECK (json_valid(issues_json)),
    checked_at        INTEGER NOT NULL
) STRICT;

CREATE INDEX ix_definition_validations__status ON definition_validations(status) WHERE status <> 'Valid';
```

`Degraded` is the state that carries weight. The engine requires that a definition referencing an
unavailable server-discovered field falls back to local evaluation **and is flagged, never silently
dropped**, and that a pack needing an unconfigured integration installs visibly degraded rather than
appearing broken. Both are the same durable state, and it is stored rather than recomputed so the GUI
can list every degraded definition without revalidating the world.

`body_hash` on the verdict is what stops a stale verdict outliving the body it judged: a mismatch
means the verdict is unknown, not valid.

**Managed collections — the definition-to-Plex binding.** A definition is not a collection. One
definition produces one Plex collection per targeted library, and for the multi-collection modes
(per-franchise, per-person) several per library. That binding is where self-healing lives.

```sql
CREATE TABLE managed_collections (
    id                  TEXT PRIMARY KEY,
    definition_id       TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    library_id          TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    variant_key         TEXT NOT NULL DEFAULT '',   -- '' for single; franchise/person id otherwise
    title               TEXT NOT NULL,
    rating_key          TEXT,                       -- NULL until created; changes on Plex re-key
    plex_guid           TEXT,
    is_smart            INTEGER NOT NULL DEFAULT 0 CHECK (is_smart IN (0,1)),
    smart_filter_uri    TEXT,                       -- Plex 'content' attribute for smart collections
    collection_mode     INTEGER,                    -- -1 default, 0 hide, 1 hideItems, 2 showItems
    collection_sort     INTEGER,                    -- 0 release, 1 alpha, 2 custom
    is_published        INTEGER NOT NULL DEFAULT 0 CHECK (is_published IN (0,1)),
    item_count          INTEGER NOT NULL DEFAULT 0,
    created_at          INTEGER NOT NULL,
    last_reconciled_at  INTEGER,
    last_result         TEXT CHECK (last_result IS NULL OR last_result IN ('Ok','Failed','Skipped','Frozen')),
    last_error          TEXT,
    missing_since       INTEGER,                    -- observed absent in Plex
    healed_at           INTEGER,
    heal_count          INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE UNIQUE INDEX ux_managed_collections__slot ON managed_collections(definition_id, library_id, variant_key);
CREATE UNIQUE INDEX ux_managed_collections__key  ON managed_collections(library_id, rating_key) WHERE rating_key IS NOT NULL;
CREATE INDEX ix_managed_collections__missing     ON managed_collections(missing_since) WHERE missing_since IS NOT NULL;
```

`collection_mode` and `collection_sort` are stored as the integers Plex uses rather than as tokens,
with the mapping documented here so nobody has to rediscover it: mode is `-1` library default, `0`
hide collection, `1` hide items in this collection, `2` show this collection and its items; sort is
`0` by release date, `1` alphabetical, `2` custom. `collection_sort` is **not writable on a smart
collection** — its order is a property of the filter — which is the same constraint the engine already
enforces at save time, restated here because the column exists and would otherwise look writable.

`heal_count` is deliberately a counter rather than a boolean. A collection healed forty times is not
self-healing successfully; it is fighting something, and a rising count is the signal that surfaces it
on the doctor page.

```sql
CREATE TABLE managed_collection_items (
    managed_collection_id TEXT NOT NULL REFERENCES managed_collections(id) ON DELETE CASCADE,
    library_item_id       TEXT NOT NULL REFERENCES library_items(id) ON DELETE CASCADE,
    ordinal               INTEGER NOT NULL,
    added_at              INTEGER NOT NULL,
    PRIMARY KEY (managed_collection_id, library_item_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_managed_collection_items__ordinal ON managed_collection_items(managed_collection_id, ordinal);
CREATE INDEX ix_managed_collection_items__item    ON managed_collection_items(library_item_id);
```

The last reconciled membership. It exists so that reconciliation is a diff against a known previous
state rather than a full rewrite, which is what makes the invariant "a second run with unchanged
inputs performs no writes" achievable rather than aspirational. `ix_..._items__item` answers mutual
exclusion ("which collections already contain this item?") without a scan.

**Exclusions.**

```sql
CREATE TABLE exclusions (
    id            TEXT PRIMARY KEY,
    scope         TEXT NOT NULL CHECK (scope IN ('Global','Definition')),
    definition_id TEXT REFERENCES definitions(id) ON DELETE CASCADE,
    id_space      TEXT NOT NULL,
    id_value      TEXT NOT NULL,
    reason        TEXT,
    created_at    INTEGER NOT NULL,
    created_by    TEXT,
    CHECK ((scope = 'Definition') = (definition_id IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_exclusions__global ON exclusions(id_space, id_value) WHERE scope = 'Global';
CREATE UNIQUE INDEX ux_exclusions__def    ON exclusions(definition_id, id_space, id_value) WHERE scope = 'Definition';
```

Global exclusions are operational state, not document content, which is why they are a table rather
than a definition: they are edited from a list page, apply across every definition, and must not fork
when a pack updates. Mutual-exclusion *groups*, by contrast, are document content and live in the
collection body (`reconcile.mutualExclusionGroup`, per the engine).

---

### 19.10 Packs

```sql
CREATE TABLE packs (
    namespace        TEXT PRIMARY KEY,
    version          TEXT NOT NULL,
    title            TEXT NOT NULL,
    manifest_json    TEXT NOT NULL CHECK (json_valid(manifest_json)),
    source           TEXT NOT NULL CHECK (source IN ('Builtin','File','Url','Repository')),
    source_ref       TEXT,
    installed_at     INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    is_enabled       INTEGER NOT NULL DEFAULT 0 CHECK (is_enabled IN (0,1)),
    state            TEXT NOT NULL CHECK (state IN ('Ok','Degraded','Broken')),
    state_reasons_json TEXT NOT NULL CHECK (json_valid(state_reasons_json))
) STRICT;

CREATE TABLE pack_assets (
    pack_namespace TEXT NOT NULL REFERENCES packs(namespace) ON DELETE CASCADE,
    slug           TEXT NOT NULL,
    asset_id       TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    PRIMARY KEY (pack_namespace, slug)
) STRICT, WITHOUT ROWID;

CREATE TABLE pack_variable_values (
    pack_namespace TEXT NOT NULL REFERENCES packs(namespace) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    value_json     TEXT NOT NULL CHECK (json_valid(value_json)),
    set_at         INTEGER NOT NULL,
    PRIMARY KEY (pack_namespace, name)
) STRICT, WITHOUT ROWID;
```

`is_enabled` defaults to `0`: installing a pack never enables anything without consent, per the
engine's install rules. `state = 'Degraded'` is the installed-but-missing-a-required-integration
condition, with the specific unmet `requiresFields` entries in `state_reasons_json`.

`pack_assets.asset_id` is `ON DELETE RESTRICT`: an asset referenced by an installed pack cannot be
garbage-collected, which is what keeps the mark-sweep retention job from deleting a font a pack needs
but no definition currently uses.

Pack-origin definitions are ordinary `definitions` rows with `origin_kind = 'Pack'`. Forking to
`user/` inserts a new row with `forked_from` set, and pack upgrade replaces rows where
`origin_kind = 'Pack'` and leaves forks untouched — the "which forks are now behind upstream" report
is a join on `forked_from` comparing `body_hash`.

`pack_variable_values` holds the answers the operator gave in the install dialog for the variables the
manifest declares, per the engine's pack-variable rules. It exists for exactly one reason: pack
upgrade re-runs substitution and expansion against the new templates, and without the stored answers
an upgrade either discards the operator's choices or cannot regenerate the unforked definitions at
all.

The substitution itself leaves no trace in `definitions`. Every row holds a concrete, fully resolved
document — no variable references, no conditionals, no unexpanded templates — which is what keeps
validation-at-save a guarantee rather than a guess, and what keeps two documents that differ from
diffing as different (D-044, per the engine's document-identity rules). A definition row containing
substitution syntax is a bug, and I-DEF-8 is the test that says so.

---

### 19.11 Sources, health, and contribution freezing

**Circuit-breaker state is persisted.**

```sql
CREATE TABLE source_health (
    id                TEXT PRIMARY KEY,
    source_type       TEXT NOT NULL,               -- 'tmdb.chart', 'imdb.list', 'radarr.tag', …
    instance_ref      TEXT NOT NULL DEFAULT '',    -- *arr instance id, or '' for global services
    state             TEXT NOT NULL CHECK (state IN ('Closed','Open','HalfOpen')),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    opened_at         INTEGER,
    cooldown_until    INTEGER,
    last_success_at   INTEGER,
    last_failure_at   INTEGER,
    last_error_kind   TEXT,                        -- 'Timeout','Http4xx','Http5xx','Challenge','Parse','Auth','RateLimit'
    last_error        TEXT,
    updated_at        INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX ux_source_health__key ON source_health(source_type, instance_ref);
```

Breaker state survives restart deliberately. An in-memory breaker resets on every restart, so a
crash-loop or a routine upgrade turns into a burst of requests at a service that is already failing —
which is exactly the behaviour that gets a scraped source blocked and an API key rate-limited.

`last_error_kind` is an enumerated classification rather than free text because the doctor page and
the retry policy both branch on it, and because `Challenge` must be distinguishable from `Parse`: a
challenge page must never reach the parser and get counted as zero items, and that requirement is only
checkable if the classification is recorded.

**Frozen contributions.** A failed source freezes its contribution at last-known-good rather than
emptying the collection — an invariant carried from the product's earliest design pass. Freezing
requires the last-known-good to still exist.

```sql
CREATE TABLE source_contributions (
    id               TEXT PRIMARY KEY,
    definition_id    TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    source_index     INTEGER NOT NULL,            -- position in the definition's sources[]
    source_type      TEXT NOT NULL,
    params_hash      TEXT NOT NULL,               -- digest of the resolved source parameters
    status           TEXT NOT NULL CHECK (status IN ('Ok','Failed','Frozen')),
    affirmed_empty   INTEGER NOT NULL DEFAULT 0 CHECK (affirmed_empty IN (0,1)),
    item_count       INTEGER NOT NULL,
    items_json       TEXT NOT NULL CHECK (json_valid(items_json)),
    fetched_at       INTEGER NOT NULL,
    duration_ms      INTEGER,
    error            TEXT,
    is_last_good     INTEGER NOT NULL DEFAULT 0 CHECK (is_last_good IN (0,1))
) STRICT;

CREATE UNIQUE INDEX ux_source_contributions__lastgood
    ON source_contributions(definition_id, source_index) WHERE is_last_good = 1;
CREATE INDEX ix_source_contributions__recent
    ON source_contributions(definition_id, source_index, fetched_at);
```

`items_json` is an ordered array of `{idSpace, id, title, position}` stored as one blob rather than
normalised into rows. The rule applied here, and generally in this schema: **normalise what is
queried; keep as JSON what is only ever read and written whole.** A contribution is fetched whole,
merged whole, and replaced whole; there is no query that selects one member of it. Normalising it
would add several hundred thousand rows and a join to every pass, in exchange for a capability nothing
uses.

At most two rows survive per `(definition_id, source_index)`: the most recent, and the most recent
with `is_last_good = 1`. When a fetch succeeds and is trustworthy — `status = 'Ok'`, and either
`item_count > 0` or `affirmed_empty = 1` — the new row becomes last-known-good and the old one is
deleted, in the same transaction. `params_hash` guards the freeze: a frozen contribution whose
parameters have since changed is not reused, because it is last-known-good for a *different question*.

**External HTTP cache.**

```sql
CREATE TABLE http_cache (
    cache_key      TEXT PRIMARY KEY,             -- digest of method+url+relevant headers+parser_version
    source_type    TEXT NOT NULL,
    parser_version INTEGER NOT NULL,             -- the rung parser that keyed this entry
    status         INTEGER NOT NULL,
    body           BLOB NOT NULL,
    content_type   TEXT,
    etag           TEXT,
    last_modified  TEXT,
    fetched_at     INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    hit_count      INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX ix_http_cache__expiry ON http_cache(expires_at);
CREATE INDEX ix_http_cache__source ON http_cache(source_type, parser_version);
```

Response bodies live in the database rather than the asset store: they are small, short-lived, and
purged by TTL, and putting them in the content-addressed store would fill it with garbage that the
mark-sweep then has to walk. `etag` and `last_modified` enable conditional revalidation, which is what
keeps a poster-metadata refresh cheap against providers that support it.

**`parser_version` is inside the key, not merely stored beside it** (D-043). The digest is computed
over the request *and* the version of the code that will interpret the response, so a bumped parser
cannot read an entry the previous parser keyed — it misses, refetches, and reparses. Storing the
version as a plain column and filtering on read would leave the old rows matching, which is the
failure this closes.

This is the same render-key argument used for asset caching, applied to a second cache. A parser fix
changes the parsed result for identical bytes; without the version in the key, every cached body keeps
parsing the old way until its TTL expires, and the fix reaches nobody. The version comes from the
source registry's per-rung `parserVersion`, so it is bumped by the adapter author in the same commit
as the fix. Tested by I-DATA-12.

**Expiries are spread on write, never clustered.** `expires_at` is set to
`fetched_at + ttl - random(0, ttl / 4)`, so entries written together do not expire together. Without
the spread, a first run populates the cache in one burst and every subsequent expiry arrives in the
same burst, which turns a cold start into a recurring simultaneous refetch against every provider —
the shape that gets keys revoked and addresses blocked, and the same failure I-SRC-4 protects against
from the other direction. The subtraction is deliberate: entries expire early rather than late, so the
spread never extends a TTL past what the provider's caching headers allow. Tested by I-PERF-4.

**Bulk reference datasets.** Some providers publish their whole dataset as a periodic file rather than
answering per title — ratings and genres for every title in existence, refreshed daily,
unauthenticated. Importing one file is both cheaper and more complete than asking per item, and at the
target scale it is the only viable shape (D-042).

```sql
CREATE TABLE reference_datasets (
    dataset       TEXT PRIMARY KEY,              -- e.g. 'imdb.title.ratings'
    generation    INTEGER NOT NULL,              -- monotonic; the live generation
    row_count     INTEGER NOT NULL,
    source_etag   TEXT,
    imported_at   INTEGER NOT NULL,
    import_state  TEXT NOT NULL CHECK (import_state IN ('Live','Staging','Failed')),
    failure_reason TEXT
) STRICT;

CREATE TABLE reference_dataset_rows (
    dataset     TEXT    NOT NULL,
    generation  INTEGER NOT NULL,
    key_space   TEXT    NOT NULL,                -- identifier space of `key`, e.g. 'imdb'
    key         TEXT    NOT NULL,
    value_json  TEXT    NOT NULL CHECK (json_valid(value_json)),
    PRIMARY KEY (dataset, generation, key_space, key)
) STRICT, WITHOUT ROWID;
```

**Why not an existing table.** The HTTP cache above is keyed per request and purged by TTL, and a
20 MB body is not a cache entry. `id_mappings` (§19.7) is bulk reference data of the right shape but is
scoped to identifier mapping and refreshed on its own cadence; widening it would make one dataset's
refresh rewrite another's rows.

**The import is all-or-nothing.** Rows are written at `generation + 1` with `import_state = 'Staging'`,
the row count and a spot-check are verified, and only then does a single transaction promote the new
generation to `Live` and delete the old one. A truncated download, a changed column layout, or a parse
failure leaves the previous generation live and records `Failed` with a reason. The alternative —
merging rows into the live table — makes a half-finished import indistinguishable at read time from a
complete one whose provider dropped half its rows, and the engine would then treat truncation as a
fact about the world. That is a known failure pattern (P1, see *Invariants*) arriving through a new
door. Tested by I-DATA-13.

**Streamed, never loaded.** The file is decompressed and inserted in batches. Holding a several-
million-row table in memory to answer a lookup a join answers would breach a resource budget for no
gain (see *Non-functional requirements*).

**These are fields, not sources.** A dataset supplies values for registry fields of availability class
`integration`. It never contributes items to a collection, so it sits outside the source registry, the
circuit breakers, and `affirmativeEmpty` entirely. A missing dataset makes its fields unavailable,
which the engine's rendering rule already defines.

**Volatile source parameters.** Where an endpoint authenticates a query by a hash or a path the
provider rotates on its own schedule, that value is not compiled into the binary. It arrives through a
signed feed the running instance fetches (D-041), so a rotation is a one-file repair rather than a
release to every installed copy — which matters because upgrade is forward-only with a mandatory
pre-migration backup (D-023), the most expensive repair path available.

```sql
CREATE TABLE volatile_params (
    name             TEXT PRIMARY KEY,           -- must exist in the shipped registry
    value            TEXT NOT NULL,
    feed_generation  INTEGER NOT NULL,
    fetched_at       INTEGER NOT NULL,
    last_good_value  TEXT NOT NULL,
    last_good_at     INTEGER NOT NULL,
    reject_count     INTEGER NOT NULL DEFAULT 0,
    last_reject_reason TEXT
) STRICT;
```

**Four constraints make this safe, and they are the decision rather than the table.**

1. **Values only.** The feed changes a declared parameter's value. It cannot add a parameter, change a
   type, or carry anything the engine executes. An unconstrained remote configuration feed is remote
   code by a slower route, which D-001 already rejected.
2. **The registry ships with the binary.** A `name` absent from the shipped source registry
   (`volatileParams`) is rejected, not inserted. The set of things the feed can influence is fixed at
   build time.
3. **Every value is checked against its declared constraint** before it is stored, and a failing value
   increments `reject_count` and leaves `last_good_value` in force. A malformed or hostile feed
   therefore degrades to the previous value, which is where a feed outage would have left us anyway.
4. **The feed is signed and verified before use.** An unverifiable feed is not applied and is reported
   on the doctor page.

`last_good_value` is a separate column rather than a history table because exactly one fallback is
ever wanted: the last value known to satisfy its constraint. Keeping a chain would invite falling back
further, and a value two rotations old is no more likely to work than a rejected one. Tested by
I-SEC-7.
### 19.12 Lifecycle tables

The component with the strictest correctness obligations, and therefore the one where the schema
does the most enforcing.

#### 19.12.1 Subjects

```sql
CREATE TABLE lifecycle_subjects (
    id                  TEXT PRIMARY KEY,
    library_id          TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    media_type          TEXT NOT NULL CHECK (media_type IN ('movie','show')),
    season_number       INTEGER,                  -- NULL = whole title; integer = that season. D-025

    -- identity
    primary_id_space    TEXT NOT NULL,            -- 'tmdb' preferred, else 'tvdb', else 'imdb'
    primary_id_value    TEXT NOT NULL,
    title               TEXT NOT NULL,            -- cached for audit legibility
    year                INTEGER,

    -- the four axes
    phase               TEXT NOT NULL CHECK (phase IN
                          ('Announced','Scheduled','Countdown','Tomorrow','Today','JustReleased','Released')),
    acquisition         TEXT NOT NULL CHECK (acquisition IN
                          ('Untracked','Requested','Monitored','Unmonitored','Grabbing','Available')),
    presence            TEXT NOT NULL CHECK (presence IN
                          ('Absent','PlaceholderPending','Placeholder','Real','RemovalPending')),
    production          TEXT CHECK (production IS NULL OR production IN
                          ('Airing','Returning','InProduction','Ended','Cancelled','Unknown')),

    -- release
    release_date        TEXT,                     -- civil date, nullable
    release_date_basis  TEXT NOT NULL CHECK (release_date_basis IN
                          ('digital','physical','theatrical_estimate','first_air','next_episode','none')),

    -- evidence and policy
    evidence_at         INTEGER,                  -- last successful full evidence refresh
    is_stale            INTEGER NOT NULL DEFAULT 0 CHECK (is_stale IN (0,1)),
    is_ambiguous        INTEGER NOT NULL DEFAULT 0 CHECK (is_ambiguous IN (0,1)),
    policy_version      INTEGER NOT NULL REFERENCES lifecycle_policies(version),

    -- bindings
    reference_count     INTEGER NOT NULL DEFAULT 0,
    placeholder_path    TEXT,
    library_item_id     TEXT REFERENCES library_items(id) ON DELETE SET NULL,
    plex_rating_key     TEXT,

    -- scheduling
    next_evaluation_at  INTEGER NOT NULL,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,

    CHECK ((production IS NULL) = (media_type = 'movie')),
    CHECK ((presence = 'Placeholder') <= (placeholder_path IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_lifecycle_subjects__identity
    ON lifecycle_subjects(library_id, primary_id_space, primary_id_value, IFNULL(season_number, -1));
CREATE INDEX ix_lifecycle_subjects__due        ON lifecycle_subjects(next_evaluation_at) WHERE is_stale = 0;
CREATE INDEX ix_lifecycle_subjects__pending    ON lifecycle_subjects(presence)
    WHERE presence IN ('PlaceholderPending','RemovalPending');
CREATE INDEX ix_lifecycle_subjects__placeholder ON lifecycle_subjects(library_id) WHERE presence = 'Placeholder';
CREATE INDEX ix_lifecycle_subjects__item       ON lifecycle_subjects(library_item_id) WHERE library_item_id IS NOT NULL;
CREATE INDEX ix_lifecycle_subjects__attention  ON lifecycle_subjects(library_id)
    WHERE is_stale = 1 OR is_ambiguous = 1;
```

**The unique index is §17.1.6 made structural.** One subject per library item and
season, reference-counted across collections, is not a convention the evaluator maintains — it is a
constraint the database enforces. The failure it prevents (per-collection records with
global deletion, so one collection's cleanup strands another's) becomes unrepresentable.

`IFNULL(season_number, -1)` in the index expression is required because SQL treats `NULL` as
distinct from `NULL` in unique indexes; without it, two whole-title subjects for the same title
would both be permitted.

`season_number` is nullable and carries the subject's granularity: `NULL` is a whole title, an
integer is that season of that show (§17.2.1). Both granularities ship at Tier 0
under D-025, with whole-title as the default and season subjects opt-in per show. Having the column
inside the unique key from the start is what makes that a no-migration decision: adding a column to
a unique index later is a table rebuild on the largest lifecycle table.

`next_evaluation_at` deserves a note, because it looks like the wall-clock timer that
§17.1.4 forbids. It is not. It is a *scheduling hint* — an index that makes "which
subjects are worth looking at" cheap — and nothing branches on it. A pass that runs six hours late
evaluates the same subjects to the same states, because state is a pure function of (persisted state,
evaluation clock, evidence). Deleting the column would cost performance and change no outcome, which
is the test for whether a hint has quietly become a timer.

`reference_count` is a denormalised cache of `COUNT(*)` over `lifecycle_references`, maintained in
the same transaction and rebuildable by `afisharr db reproject`. It is denormalised because guard G5
("a subject with `references > 0` may not be removed for reasons of departure") is checked on every
destructive path, and a count aggregate on the hot path of the most safety-critical check in the
product is the wrong trade.

#### 19.12.2 References

```sql
CREATE TABLE lifecycle_references (
    subject_id     TEXT NOT NULL REFERENCES lifecycle_subjects(id) ON DELETE CASCADE,
    definition_id  TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    first_seen_at  INTEGER NOT NULL,
    last_seen_at   INTEGER NOT NULL,
    PRIMARY KEY (subject_id, definition_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_lifecycle_references__definition ON lifecycle_references(definition_id);
```

Recomputed each pass from the collections that resolved to the subject, never incremented ad hoc
(§17.2). The reason a reference was dropped — filter fail, removed from source,
definition deleted — is known by the evaluator at drop time and written directly into the transition
record's evidence (§19.12.4), so there is no separate drop table to keep consistent.

#### 19.12.3 Alternate identifiers

```sql
CREATE TABLE lifecycle_subject_ids (
    subject_id  TEXT NOT NULL REFERENCES lifecycle_subjects(id) ON DELETE CASCADE,
    id_space    TEXT NOT NULL,
    id_value    TEXT NOT NULL,
    PRIMARY KEY (subject_id, id_space)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_lifecycle_subject_ids__lookup ON lifecycle_subject_ids(id_space, id_value);
```

The primary identity is on the subject and is what uniqueness is enforced against; alternates are
here for matching sources that speak a different identifier space. One value per space per subject —
a title with two TMDB ids is a data problem to surface, not a set to store.

#### 19.12.4 The transition log

Append-only. One row per axis transition and per side effect group.

```sql
CREATE TABLE lifecycle_transitions (
    id                TEXT PRIMARY KEY,           -- ULID: ordered by time with no index needed
    at                INTEGER NOT NULL,
    subject_id        TEXT NOT NULL,              -- NO foreign key, deliberately (§19.1.6)
    library_id        TEXT NOT NULL,
    subject_json      TEXT NOT NULL CHECK (json_valid(subject_json)),  -- denormalised identity
    axis              TEXT NOT NULL CHECK (axis IN ('phase','acquisition','presence','production')),
    from_state        TEXT NOT NULL,
    to_state          TEXT NOT NULL,
    trigger           TEXT NOT NULL,
    is_destructive    INTEGER NOT NULL DEFAULT 0 CHECK (is_destructive IN (0,1)),
    evidence_json     TEXT NOT NULL CHECK (json_valid(evidence_json)),
    effects_json      TEXT CHECK (effects_json IS NULL OR json_valid(effects_json)),
    policy_version    INTEGER NOT NULL,
    registry_version  INTEGER,
    actor             TEXT NOT NULL,              -- 'scheduler:collection-sync', 'user:01J…', 'startup:reconcile'
    pass_id           TEXT,
    CHECK (is_destructive = 0 OR trigger IN
        ('Materialized','Departed','Retired','FilteredOut','Disabled','Manual','Reaped'))
) STRICT;

CREATE INDEX ix_lifecycle_transitions__subject     ON lifecycle_transitions(subject_id, at);
CREATE INDEX ix_lifecycle_transitions__destructive ON lifecycle_transitions(at) WHERE is_destructive = 1;
CREATE INDEX ix_lifecycle_transitions__pass        ON lifecycle_transitions(pass_id) WHERE pass_id IS NOT NULL;
```

**The `CHECK` on `is_destructive` is §17.8.2's allowlist written into the
database.** A destructive transition recorded under any trigger outside the allowlist cannot be
inserted. It does not prevent the deletion itself — code does that — but it makes an unauditable
deletion impossible to *record*, so the property test in §17.14.3 has something to
assert against that no code path can quietly bypass.

`is_destructive` is a stored column rather than a derived predicate because retention differentiates
on it (§19.17.1) and a partial index on a stored column is far cheaper than one on an expression over a
value set that may widen.

Identity is denormalised into `subject_json` for the reason given in §19.1.6: this log's entire purpose
is explaining what happened to things that no longer exist.

#### 19.12.5 Intents

§17.9's intend / execute / confirm sequence, and the startup reconciliation that
makes a `kill -9` recoverable.

```sql
CREATE TABLE lifecycle_intents (
    id             TEXT PRIMARY KEY,
    subject_id     TEXT NOT NULL REFERENCES lifecycle_subjects(id) ON DELETE CASCADE,
    kind           TEXT NOT NULL CHECK (kind IN
                     ('CreatePlaceholder','RemovePlaceholder','RepairPlaceholderTitle','RestoreBasePoster')),
    state          TEXT NOT NULL CHECK (state IN
                     ('Intended','Executing','Executed','Confirmed','Failed','Abandoned')),
    payload_json   TEXT NOT NULL CHECK (json_valid(payload_json)),   -- path, source video, target title
    prior_presence TEXT NOT NULL,                 -- to restore on failure
    attempts       INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    executed_at    INTEGER,
    confirmed_at   INTEGER,
    last_error     TEXT,
    owner          TEXT,                          -- process instance that holds it
    lease_expires_at INTEGER
) STRICT;

CREATE INDEX ix_lifecycle_intents__open ON lifecycle_intents(created_at)
    WHERE state NOT IN ('Confirmed','Abandoned');
CREATE INDEX ix_lifecycle_intents__subject ON lifecycle_intents(subject_id);
```

Startup re-drives every row matched by `ix_lifecycle_intents__open` from step 2, which is safe
because both operations are idempotent by design. `prior_presence` is what makes step 3's "back to
its prior state on failure" a fact rather than a reconstruction — after a crash there is no memory
of what the state was before the intent, unless it was written down.

`attempts` bounds the retry: an intent that has failed `maxIntentAttempts` times moves to
`Abandoned`, leaves the subject in its prior presence, and raises a doctor finding. Retrying forever
against a read-only filesystem produces an unbounded log and no progress.

#### 19.12.6 Placeholder roots, files, and the orphan sweep

```sql
CREATE TABLE placeholder_roots (
    id          TEXT PRIMARY KEY,
    library_id  TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    media_type  TEXT NOT NULL CHECK (media_type IN ('movie','show')),
    path        TEXT NOT NULL,
    is_enabled  INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
    created_at  INTEGER NOT NULL,
    retired_at  INTEGER                            -- kept after removal; see below
) STRICT;

CREATE UNIQUE INDEX ux_placeholder_roots__path ON placeholder_roots(library_id, media_type, path);

CREATE TABLE orphan_candidates (
    id             TEXT PRIMARY KEY,
    path           TEXT NOT NULL,
    root_id        TEXT REFERENCES placeholder_roots(id) ON DELETE SET NULL,
    size_bytes     INTEGER,
    first_seen_at  INTEGER NOT NULL,
    last_seen_at   INTEGER NOT NULL,
    sweep_id       TEXT NOT NULL,
    resolution     TEXT NOT NULL DEFAULT 'Reported'
                     CHECK (resolution IN ('Reported','Claimed','Deleted','Ignored')),
    resolved_at    INTEGER,
    resolved_by    TEXT
) STRICT;

CREATE UNIQUE INDEX ux_orphan_candidates__path ON orphan_candidates(path);
```

A retired root is **retained, not deleted**. §17.12 requires that when a
placeholder root path changes, the old paths become orphan-sweep candidates — which is impossible if
the record of the old path was deleted along with the setting.

`orphan_candidates` implements §17.5.1's central rule: the sweep treats any unreferenced file under a
placeholder root as a candidate and **reports it** rather than matching a list of past filename
conventions. `Reported` is the default resolution and nothing deletes from that state without an
operator action or a policy that explicitly permits it. `Claimed` is the good outcome — a candidate
that a subject turned out to reference after all, which happens when a sweep races a creation.

#### 19.12.7 Acquisition decisions

```sql
CREATE TABLE acquisition_decisions (
    id                TEXT PRIMARY KEY,
    at                INTEGER NOT NULL,
    subject_id        TEXT NOT NULL,              -- NO foreign key (§19.1.6)
    subject_json      TEXT NOT NULL CHECK (json_valid(subject_json)),
    definition_id     TEXT NOT NULL,
    route             TEXT NOT NULL CHECK (route IN ('Request','Direct','None')),
    instance_ref      TEXT,
    overrides_json    TEXT NOT NULL CHECK (json_valid(overrides_json)),   -- profile, root, tags, monitor, search, season folder
    seasons_json      TEXT CHECK (seasons_json IS NULL OR json_valid(seasons_json)),
    gates_json        TEXT NOT NULL CHECK (json_valid(gates_json)),       -- every gate, its inputs, its verdict
    policy_version    INTEGER NOT NULL,
    outcome           TEXT NOT NULL CHECK (outcome IN ('Submitted','Skipped','Rejected','Failed')),
    external_ref      TEXT,                        -- *arr or Overseerr id of the created record
    error             TEXT
) STRICT;

CREATE INDEX ix_acquisition_decisions__subject ON acquisition_decisions(subject_id, at);
CREATE INDEX ix_acquisition_decisions__time    ON acquisition_decisions(at);
```

A separate table from the transition log because §18 sets an exit criterion
that grab decisions are **reproducible from the audit log alone**, and reproducibility is a property
of record completeness. Every input the decision consumed is here: the gates and their verdicts, the
resolved overrides, the concrete season list, and the policy version that resolves any default not
explicitly recorded. The replay test loads one row and recomputes; if it needs anything not in the
row, the row is wrong.

`gates_json` records **every** gate evaluated, including the ones that passed. A record of only the
failing gate cannot distinguish "passed all gates" from "no gates configured."

---

### 19.13 Placement tables

Three participant types in one ordering space, an API with no absolute positions, and a finite
hidden precision budget. The schema's job is to make the accounting in §15.4.4
possible and the sort-title round trip in §15.6 exact.

#### 19.13.1 Participants

```sql
CREATE TABLE placement_participants (
    id                TEXT PRIMARY KEY,
    type              TEXT NOT NULL CHECK (type IN ('Managed','Adopted','NativeHub','Unknown')),
    library_id        TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    managed_collection_id TEXT REFERENCES managed_collections(id) ON DELETE CASCADE,
    rating_key        TEXT,                        -- Managed and Adopted
    hub_identifier    TEXT,                        -- NativeHub, e.g. 'home.continue'
    plex_hub_id       TEXT,                        -- ManagedHub id used by the move/promote endpoints
    title             TEXT NOT NULL,
    is_deletable      INTEGER NOT NULL CHECK (is_deletable IN (0,1)),   -- Plex's own 'deletable'
    first_seen_at     INTEGER NOT NULL,
    last_seen_at      INTEGER NOT NULL,
    missing_since     INTEGER,
    CHECK ((type = 'Managed')   <= (managed_collection_id IS NOT NULL)),
    CHECK ((type = 'NativeHub') <= (hub_identifier IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_placement_participants__key
    ON placement_participants(library_id, plex_hub_id) WHERE plex_hub_id IS NOT NULL;
CREATE INDEX ix_placement_participants__anchor
    ON placement_participants(library_id) WHERE is_deletable = 0;
```

**`is_deletable` is read from Plex, not inferred.** The managed-hub resource reports it directly, and
it is exactly the anchor test §15.1 needs: a participant with `is_deletable = 0`
cannot be unpromoted, so the unpromote/re-promote recovery does not exist for it. Deriving anchorhood
from "has no rating key", which is how the constraint tends to be discovered: as an edge case in an
error handler — is an approximation that Plex will eventually falsify. The partial index makes
"which participants in this library are anchors" free, and rung 1 and rung 2 of the escalation ladder
both need it on every pass.

`type = 'Unknown'` is the row for a participant Afisharr does not recognise. §15.8
requires that such participants are left alone and recorded, never evicted. Giving them a row is what
makes "left alone" auditable: the pass can show that it saw the participant, planned around it, and
wrote nothing to it.

#### 19.13.2 Desired placement

```sql
CREATE TABLE placement_desired (
    participant_id  TEXT NOT NULL REFERENCES placement_participants(id) ON DELETE CASCADE,
    surface         TEXT NOT NULL CHECK (surface IN ('Home','Library')),
    definition_id   TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    position        INTEGER NOT NULL,
    zone            TEXT CHECK (zone IS NULL OR zone IN ('Promoted','Alphabetical')),
    randomize       INTEGER NOT NULL DEFAULT 0 CHECK (randomize IN (0,1)),
    updated_at      INTEGER NOT NULL,
    PRIMARY KEY (participant_id, surface),
    CHECK ((surface = 'Library') = (zone IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_placement_desired__plan ON placement_desired(surface, position);
```

⟨derived⟩ from `Placement` definition bodies. The ordering pass reads the whole desired sequence for
a surface sorted by `(position, participant ULID)` — the deterministic order and tie-break of
§15.4.1 — and materialising it removes a JSON extraction over every placement
definition from the hot path of the highest-risk subsystem.

Positions are `INTEGER` and need be neither dense nor unique; the tie-break makes duplicates
well-defined rather than merely tolerated.

#### 19.13.3 Visibility as a principal set

```sql
CREATE TABLE placement_visibility (
    participant_id  TEXT NOT NULL REFERENCES placement_participants(id) ON DELETE CASCADE,
    surface         TEXT NOT NULL CHECK (surface IN ('Home','Recommended')),
    principal_id    TEXT NOT NULL REFERENCES principals(id) ON DELETE CASCADE,
    PRIMARY KEY (participant_id, surface, principal_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_placement_visibility__principal ON placement_visibility(principal_id);
```

**This table discharges the obligation recorded in §2.6.** Visibility is a set of principals from day
one. At Tier 0 the GUI writes only rows whose `principal_id` is one of the three seeded whole-audience
principals; Tier 1 per-user targeting inserts `PlexUser` principals and rows referencing them, with
no DDL. I-DATA-5 is the test that keeps this honest.

Note the surface vocabulary here differs from §19.13.2's: visibility distinguishes `Home` from
`Recommended` because Plex exposes home visibility and recommendations visibility as separate
controls, while ordering distinguishes `Home` from `Library` because those are the two sequences
being ordered. They are genuinely different axes and conflating them into one enum would force one of
the two to be wrong. §12.7 was amended on 2026-08-08 to separate them.

#### 19.13.4 Gap-budget accounting

§15.4.4: Afisharr cannot read Plex's stored position values, so it estimates the
remaining precision headroom by tracking insertions into each adjacent pair.

```sql
CREATE TABLE placement_gaps (
    surface           TEXT NOT NULL CHECK (surface IN ('Home','Library')),
    library_id        TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    before_participant_id TEXT NOT NULL,          -- '' denotes the head of the sequence
    after_participant_id  TEXT NOT NULL,          -- '' denotes the tail
    insertions        INTEGER NOT NULL DEFAULT 0, -- direct insertions into this exact pair
    depth             INTEGER NOT NULL DEFAULT 0, -- inherited subdivision depth; see below
    last_refreshed_at INTEGER NOT NULL,
    PRIMARY KEY (surface, library_id, before_participant_id, after_participant_id)
) STRICT, WITHOUT ROWID;
```

Two counters, because they answer different questions and the design document names the first while
the *mechanism* demands the second.

`insertions` is §15.4.4 as written: how many times this pair has been split.

`depth` tracks something the raw count misses. Inserting C between A and B does not leave the pair
(A,B) with one insertion — it **destroys** that pair and creates (A,C) and (C,B), each of which has
roughly half the numeric headroom the original had. If each new pair starts at zero, the budget check
never fires: a caller can subdivide the same region indefinitely, always into a "fresh" pair, and
exhaust precision while every counter reads 1. So on insertion both child pairs inherit
`depth = parent.depth + 1`, and `depth` resets to 0 for a pair whose right-hand participant was just
re-promoted with fresh spacing. The budget check compares `depth` against `gapBudget`; `insertions`
is retained because it is the number the design document specifies and because a pair with high
`insertions` and low `depth` is a different and interesting diagnostic.

This is a refinement of §4.4's mechanism, not a change to its policy, and is raised as an amendment
as an amendment to §15.4.4, which now carries it.

`''` as a sentinel for the sequence head and tail rather than `NULL`, because `NULL` in a `WITHOUT
ROWID` primary key is permitted by SQLite and would allow duplicate head rows.

#### 19.13.5 Sort-title originals

§15.6.2 and §6.4: original values recorded before the first mutation, and promote
→ demote restores the exact original, byte for byte.

```sql
CREATE TABLE sort_title_originals (
    id                TEXT PRIMARY KEY,
    library_id        TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    rating_key        TEXT NOT NULL,
    participant_id    TEXT REFERENCES placement_participants(id) ON DELETE SET NULL,
    was_present       INTEGER NOT NULL CHECK (was_present IN (0,1)),
    was_locked        INTEGER NOT NULL CHECK (was_locked IN (0,1)),
    original_value    BLOB,                        -- NULL only when was_present = 0
    original_sha256   TEXT,
    recorded_at       INTEGER NOT NULL,
    consent_id        TEXT REFERENCES adoption_consents(id) ON DELETE SET NULL,
    restored_at       INTEGER,
    restore_verified  INTEGER NOT NULL DEFAULT 0 CHECK (restore_verified IN (0,1)),
    CHECK ((was_present = 1) = (original_value IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_sort_title_originals__item ON sort_title_originals(library_id, rating_key);
```

Four columns here exist because of facts about the Plex metadata protocol that only surfaced from
reading it, and each of them is the difference between a round trip that works and one that appears
to work:

**`was_present` is separate from the value.** Plex's own clients default a missing sort title to the
item's title when parsing. That means an item with no sort title and an item whose sort title happens
to equal its title are indistinguishable in the parsed object. Restoration must therefore record
presence from the *raw attribute*, not from a value that may have been defaulted, or "restore" writes
an explicit sort title onto an item that never had one — permanently changing how Plex's own agents
treat that field.

**`was_locked` is recorded and restored.** Every editable Plex metadata field carries a `locked`
flag, and the edit endpoint writes `field.value` and `field.locked` together; writing a value locks
the field by default. An item whose sort title was unlocked before Afisharr touched it must be unlocked
again afterwards, because a locked field is one that Plex's metadata agents will never refresh. A
round trip that restores the string and leaves the lock set has quietly disabled metadata refresh for
that item, forever, and nothing will ever report it.

**`original_value` is `BLOB`, not `TEXT`.** "Byte for byte" is the obligation. A `TEXT` column in a
`STRICT` table will accept and return the bytes faithfully in practice, but a blob makes the
guarantee independent of any encoding assumption about what Plex returned, and `original_sha256`
makes the restoration verifiable rather than merely attempted. `restore_verified` records that the
read-back matched.

Rows are retained after restoration, not deleted. The next promotion of the same item must record the
original again; keeping the history makes a drifting sort title diagnosable rather than a mystery.

#### 19.13.6 Adoption consent

```sql
CREATE TABLE adoption_consents (
    id              TEXT PRIMARY KEY,
    scope           TEXT NOT NULL CHECK (scope IN ('Global','Library','Participant')),
    library_id      TEXT REFERENCES libraries(id) ON DELETE CASCADE,
    participant_id  TEXT REFERENCES placement_participants(id) ON DELETE CASCADE,
    granted         INTEGER NOT NULL CHECK (granted IN (0,1)),
    granted_at      INTEGER NOT NULL,
    granted_by      TEXT,
    revoked_at      INTEGER,
    CHECK ((scope = 'Library')     = (library_id IS NOT NULL AND participant_id IS NULL)),
    CHECK ((scope = 'Participant') = (participant_id IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX ux_adoption_consents__global      ON adoption_consents(scope) WHERE scope = 'Global';
CREATE UNIQUE INDEX ux_adoption_consents__library     ON adoption_consents(library_id) WHERE scope = 'Library';
CREATE UNIQUE INDEX ux_adoption_consents__participant ON adoption_consents(participant_id) WHERE scope = 'Participant';
```

D-014 settles the granularity at per library, with a per-collection override. All three scopes stay
representable here, and resolution is most-specific-wins: participant, then library, then global,
defaulting to *not granted*.

**Resolved 2026-08-08: per library is the default granularity, with a per-collection override.** The
GUI's primary consent control is one per library; a per-collection control exists for the cases where
a user wants a single collection excluded or included against the library's setting. No global
control ships. The reasoning is recorded because it is the kind of decision that gets "simplified"
later: per-collection consent makes onboarding a wall of ticks at exactly the moment someone is
deciding whether the product is worth the trouble, while a global toggle is consent given once that
silently extends to collections the user adopts months later and has never seen. The library is the
unit the user already thinks in and the unit Afisharr already orders.

Because all three scopes remain storable, adding a global control later is a policy change and a GUI
change, never a migration.

Revocation is a timestamp rather than a delete, so a sort title written under a consent that was
later revoked is still explainable.

#### 19.13.7 Observed state, passes, and randomisation

```sql
CREATE TABLE placement_surface_state (
    surface           TEXT NOT NULL CHECK (surface IN ('Home','Library')),
    library_id        TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    desired_hash      TEXT,                        -- hash(desired sequence, visibility set)
    verified_hash     TEXT,                        -- hash of the order actually read back
    verified_at       INTEGER,
    observed_json     TEXT CHECK (observed_json IS NULL OR json_valid(observed_json)),
    rung_reached      INTEGER NOT NULL DEFAULT 0 CHECK (rung_reached BETWEEN 0 AND 3),
    is_non_convergent INTEGER NOT NULL DEFAULT 0 CHECK (is_non_convergent IN (0,1)),
    non_convergent_json TEXT CHECK (non_convergent_json IS NULL OR json_valid(non_convergent_json)),
    last_pass_id      TEXT,
    PRIMARY KEY (surface, library_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE placement_passes (
    id                 TEXT PRIMARY KEY,
    surface            TEXT NOT NULL,
    library_id         TEXT NOT NULL,              -- NO foreign key (§19.1.6)
    started_at         INTEGER NOT NULL,
    finished_at        INTEGER,
    participant_count  INTEGER NOT NULL DEFAULT 0,
    moves_planned      INTEGER NOT NULL DEFAULT 0,
    moves_applied      INTEGER NOT NULL DEFAULT 0,
    rebalances         INTEGER NOT NULL DEFAULT 0,
    rung_reached       INTEGER NOT NULL DEFAULT 0,
    verification       TEXT CHECK (verification IS NULL OR verification IN ('Ok','Mismatch','Skipped')),
    gap_pressure       INTEGER NOT NULL DEFAULT 0, -- max depth encountered
    plan_json          TEXT CHECK (plan_json IS NULL OR json_valid(plan_json)),
    error              TEXT
) STRICT;

CREATE INDEX ix_placement_passes__surface ON placement_passes(library_id, surface, started_at);

CREATE TABLE randomization_epochs (
    surface     TEXT NOT NULL,
    library_id  TEXT NOT NULL DEFAULT '',          -- '' for the Home surface
    epoch       INTEGER NOT NULL DEFAULT 0,
    advanced_at INTEGER NOT NULL,
    PRIMARY KEY (surface, library_id)
) STRICT, WITHOUT ROWID;
```

`desired_hash` versus `verified_hash` is §15.4.7's idempotency check made cheap:
equal hashes mean the pass issues **no API calls at all**, not merely no moves.

`plan_json` on the pass is what makes "why did this move?" answerable after the fact — it records the
computed plan, including which items the longest-increasing-subsequence left in place. Retained for
30 days (§19.17.1); a full plan is small and the question is always asked about something recent.

`randomization_epochs` gives §15.7's seed its durable half. The shuffle is seeded
by `(epoch, surface)`, so repeated passes within one epoch produce an identical order and therefore
zero moves and zero precision burned. Advancing the epoch is an explicit act — the schedule, or a
user-forced re-roll — and it is timestamped so the audit can explain a reshuffle.

---

### 19.14 Assets and rendering

#### 19.14.1 The content-addressed asset store

```sql
CREATE TABLE assets (
    id             TEXT PRIMARY KEY,
    sha256         TEXT NOT NULL,
    size_bytes     INTEGER NOT NULL,
    mime           TEXT NOT NULL,
    kind           TEXT NOT NULL,                 -- Poster, Wallpaper, Font, Icon, Video, Render, Upload
    width          INTEGER,
    height         INTEGER,
    original_name  TEXT,
    origin         TEXT NOT NULL CHECK (origin IN ('Upload','Pack','LocalScan','Provider','Render','Builtin')),
    origin_ref     TEXT,                          -- provider URL, scan path, template id
    created_at     INTEGER NOT NULL,
    last_used_at   INTEGER NOT NULL,
    verified_at    INTEGER,
    missing_since  INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_assets__sha256  ON assets(sha256);
CREATE INDEX ix_assets__gc             ON assets(last_used_at);
CREATE INDEX ix_assets__missing        ON assets(missing_since) WHERE missing_since IS NOT NULL;
```

One row per distinct byte sequence, at `assets/<sha[0:2]>/<sha[2:4]>/<sha>` on disk (§19.2.1). The
unique index on `sha256` is what makes deduplication automatic: the same base poster shared by a 1080p
and a 4K library is inserted once and referenced twice.

`missing_since` is set by the reconciliation check that samples rows against the filesystem. A missing
base poster is recoverable — recapture from Plex — so it is a doctor finding, not an error. A missing
render is trivially recoverable: drop the cache row.

#### 19.14.2 Base posters

```sql
CREATE TABLE base_posters (
    library_item_id  TEXT PRIMARY KEY REFERENCES library_items(id) ON DELETE CASCADE,
    asset_id         TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    source           TEXT NOT NULL CHECK (source IN ('Plex','Tmdb','Local')),
    source_ref       TEXT,
    plex_thumb_key   TEXT,                        -- the thumb key at capture time
    captured_at      INTEGER NOT NULL,
    verified_at      INTEGER,
    is_suspect       INTEGER NOT NULL DEFAULT 0 CHECK (is_suspect IN (0,1))
) STRICT;

CREATE INDEX ix_base_posters__asset   ON base_posters(asset_id);
CREATE INDEX ix_base_posters__suspect ON base_posters(is_suspect) WHERE is_suspect = 1;
```

**_Spec_ §6: the original poster is sacred.** Captured once, before any overlay is applied,
and every subsequent render composites from it. The primary key on `library_item_id` enforces the
"once" — there is no second base poster for an item, so there is no path by which an already-overlaid
poster becomes the base and overlays compound.

`ON DELETE RESTRICT` on `asset_id` means the garbage collector cannot remove an asset that is some
item's pristine original, which is the one asset in the store that is genuinely unrecoverable if the
provider has since changed the artwork.

`is_suspect` marks a capture Afisharr is not confident is pristine — captured after Afisharr had already
written a poster, or with a thumb key that does not match what was recorded. A suspect base is not
used for compositing until the operator resolves it on the doctor page, because compositing over an
already-overlaid base is the single most visible failure this subsystem has.

#### 19.14.3 Render cache

```sql
CREATE TABLE render_cache (
    render_key        TEXT PRIMARY KEY,           -- see composition below
    target_kind       TEXT NOT NULL CHECK (target_kind IN ('Item','Collection')),
    library_item_id   TEXT REFERENCES library_items(id) ON DELETE CASCADE,
    managed_collection_id TEXT REFERENCES managed_collections(id) ON DELETE CASCADE,
    template_id       TEXT NOT NULL,
    template_version  INTEGER NOT NULL,
    base_asset_id     TEXT REFERENCES assets(id) ON DELETE SET NULL,
    state_hash        TEXT NOT NULL,
    renderer_version  INTEGER NOT NULL,
    output_asset_id   TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    rendered_at       INTEGER NOT NULL,
    uploaded_at       INTEGER,
    upload_thumb_key  TEXT,                       -- Plex thumb key after upload
    last_used_at      INTEGER NOT NULL,
    CHECK ((target_kind = 'Item') = (library_item_id IS NOT NULL))
) STRICT;

CREATE INDEX ix_render_cache__item       ON render_cache(library_item_id) WHERE library_item_id IS NOT NULL;
CREATE INDEX ix_render_cache__collection ON render_cache(managed_collection_id) WHERE managed_collection_id IS NOT NULL;
CREATE INDEX ix_render_cache__template   ON render_cache(template_id, template_version);
CREATE INDEX ix_render_cache__gc         ON render_cache(last_used_at);
```

The key composition:

```
render_key = blake3(
      base_asset.sha256          -- or a sentinel for collection posters with no base
   || template_id
   || template_version
   || state_hash
   || renderer_version
)
```

**_Spec_ §6 owns the definition of the key; this is its concrete composition.** The four
definition-layer terms cover everything a definition controls. The fifth, `renderer_version`, covers
the renderer: a change to font shaping, a resvg upgrade, or a fix to how a drop shadow is composited
all produce different pixels from identical inputs. Without it an improvement to rendering ships to
nobody, because every cache entry still matches; with it a renderer bump invalidates the cache
exactly once. I-RENDER-6 asserts that mutating any one of the five changes the key.

`ix_render_cache__template` is what makes "this template changed, invalidate its renders" an index
seek rather than a scan of the largest table in the database.

#### 19.14.4 Asset roots and local scanning

```sql
CREATE TABLE asset_roots (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL,
    purpose     TEXT NOT NULL CHECK (purpose IN ('LocalPosters','Fonts','Icons','Browse')),
    is_enabled  INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
    created_at  INTEGER NOT NULL,
    scanned_at  INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_asset_roots__path ON asset_roots(path, purpose);

CREATE TABLE local_asset_files (
    id             TEXT PRIMARY KEY,
    root_id        TEXT NOT NULL REFERENCES asset_roots(id) ON DELETE CASCADE,
    relative_path  TEXT NOT NULL,
    sha256         TEXT NOT NULL,
    size_bytes     INTEGER NOT NULL,
    matched_item_id TEXT REFERENCES library_items(id) ON DELETE SET NULL,
    matched_collection_id TEXT REFERENCES managed_collections(id) ON DELETE SET NULL,
    match_basis    TEXT,                          -- how the match was made, for the doctor page
    scanned_at     INTEGER NOT NULL,
    missing_since  INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_local_asset_files__path ON local_asset_files(root_id, relative_path);
CREATE INDEX ix_local_asset_files__item        ON local_asset_files(matched_item_id) WHERE matched_item_id IS NOT NULL;
```

`asset_roots` is also the jail for the server filesystem browser (§2: "T0, **jailed to
configured root paths**"). The browser resolves a requested path, canonicalises it, and requires the
result to be a descendant of an enabled root with `purpose = 'Browse'`. Storing the roots in the
database rather than in a config constant is what makes the jail auditable and the enforcement a
single query.

---

### 19.15 Jobs, scheduling, and observability

```sql
CREATE TABLE jobs (
    id                   TEXT PRIMARY KEY,        -- stable name for built-ins, ULID for per-definition
    kind                 TEXT NOT NULL,
    definition_id        TEXT REFERENCES definitions(id) ON DELETE CASCADE,
    cron                 TEXT NOT NULL,
    jitter_seconds       INTEGER NOT NULL DEFAULT 0,
    is_enabled           INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
    next_run_at          INTEGER,
    last_run_at          INTEGER,
    last_status          TEXT CHECK (last_status IS NULL OR last_status IN
                            ('Ok','Failed','Cancelled','Skipped','PartialSuccess')),
    last_duration_ms     INTEGER,
    last_error           TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL
) STRICT;

CREATE INDEX ix_jobs__due        ON jobs(next_run_at) WHERE is_enabled = 1;
CREATE INDEX ix_jobs__definition ON jobs(definition_id) WHERE definition_id IS NOT NULL;

CREATE TABLE job_runs (
    id            TEXT PRIMARY KEY,
    job_id        TEXT NOT NULL,                  -- NO foreign key: runs outlive deleted jobs
    trigger       TEXT NOT NULL CHECK (trigger IN ('Schedule','Manual','Api','Startup','Dependency')),
    actor         TEXT,
    started_at    INTEGER NOT NULL,
    finished_at   INTEGER,
    status        TEXT NOT NULL CHECK (status IN
                    ('Running','Ok','Failed','Cancelled','Skipped','PartialSuccess')),
    summary_json  TEXT CHECK (summary_json IS NULL OR json_valid(summary_json)),
    error         TEXT
) STRICT;

CREATE INDEX ix_job_runs__job    ON job_runs(job_id, started_at);
CREATE INDEX ix_job_runs__active ON job_runs(started_at) WHERE status = 'Running';

CREATE TABLE job_run_events (
    id           TEXT PRIMARY KEY,
    run_id       TEXT NOT NULL,
    at           INTEGER NOT NULL,
    level        TEXT NOT NULL CHECK (level IN ('Trace','Debug','Info','Warn','Error')),
    scope        TEXT,                            -- definition id, library id, source type
    message      TEXT NOT NULL,
    context_json TEXT CHECK (context_json IS NULL OR json_valid(context_json))
) STRICT;

CREATE INDEX ix_job_run_events__run ON job_run_events(run_id, at);
```

Job progress is delivered live over SSE. `job_run_events` is the durable, queryable record behind it,
and it is what the GUI logs page reads — not the text log file. Filtering "everything that happened to
this collection during last night's run" is a two-column index seek here, and a regular expression
over a rotated text file otherwise.

`consecutive_failures` on the job drives backoff and the doctor page. `status = 'Running'` rows with
no live lease are the crash residue that startup marks `Cancelled`.

```sql
CREATE TABLE doctor_findings (
    id             TEXT PRIMARY KEY,
    check_id       TEXT NOT NULL,                 -- stable identifier of the check that raised it
    severity       TEXT NOT NULL CHECK (severity IN ('Info','Warning','Error')),
    subject_kind   TEXT NOT NULL,                 -- 'library','definition','subject','participant','asset','source'
    subject_id     TEXT,
    title          TEXT NOT NULL,
    detail_json    TEXT NOT NULL CHECK (json_valid(detail_json)),
    first_seen_at  INTEGER NOT NULL,
    last_seen_at   INTEGER NOT NULL,
    acknowledged_at INTEGER,
    acknowledged_by TEXT,
    resolved_at    INTEGER
) STRICT;

CREATE UNIQUE INDEX ux_doctor_findings__key ON doctor_findings(check_id, subject_kind, IFNULL(subject_id, ''))
    WHERE resolved_at IS NULL;
CREATE INDEX ix_doctor_findings__open       ON doctor_findings(severity) WHERE resolved_at IS NULL;
```

Findings are durable and deduplicated: a check that raises the same problem on every pass updates
`last_seen_at` on one row rather than producing a hundred. `first_seen_at` is what turns "this is
broken" into "this has been broken since Tuesday," which is usually the more useful sentence.
Acknowledgement suppresses a finding in the GUI without resolving it, so a known-and-accepted
condition stops generating noise without becoming invisible.

---

### 19.16 Hot paths and the index that serves each

Every index declared above exists for a named query. If a query in this table changes, the index is
re-justified; if an index has no row here, it is a candidate for deletion.

| Hot path | Frequency | Index |
| --- | --- | --- |
| Resolve an external id to a library item | Per source item, per pass — the highest-volume lookup in the product | `ix_library_item_ids__lookup` |
| Which definitions target this library | Pass start | `ix_definition_libraries__library` |
| Definitions of a kind, enabled | GUI list, pass start | `ix_definitions__kind` |
| Subjects due for evaluation | Pass start | `ix_lifecycle_subjects__due` |
| Unconfirmed intents at startup | Startup | `ix_lifecycle_intents__open` |
| Every placeholder in a library | Hub replacement, orphan sweep | `ix_library_items__placeholder`, `ix_lifecycle_subjects__placeholder` |
| Subjects needing attention (stale, ambiguous) | Doctor page | `ix_lifecycle_subjects__attention` |
| Anchors in a library | Every placement pass, rungs 1 and 2 | `ix_placement_participants__anchor` |
| Desired sequence for a surface | Every placement pass | `ix_placement_desired__plan` (then ULID tie-break, no index needed) |
| Destructive transitions in a window | Audit, retention | `ix_lifecycle_transitions__destructive` |
| History for one subject | Doctor page, support | `ix_lifecycle_transitions__subject` |
| Which collections contain this item | Mutual exclusion | `ix_managed_collection_items__item` |
| Collections missing in Plex | Self-healing | `ix_managed_collections__missing` |
| Inbound references to a definition | Delete-with-cascade prompt | `ix_definition_refs__inbound` |
| Definitions using a field | After discovery, on registry change | `ix_definition_field_uses__field` |
| Renders for a template version | Template edit invalidation | `ix_render_cache__template` |
| Cache and asset GC candidates | Nightly | `ix_render_cache__gc`, `ix_assets__gc`, `ix_http_cache__expiry` |
| Jobs due | Scheduler tick | `ix_jobs__due` |
| Events for a run | Logs page | `ix_job_run_events__run` |
| Open findings by severity | Doctor page, dashboard badge | `ix_doctor_findings__open` |

Two general notes. Partial indexes are used wherever the interesting rows are a small minority —
placeholders, anchors, missing collections, open findings — because a partial index over 2% of a
table is 2% of the size and stays in cache. And no index is created on a column that only appears in
a `SELECT` list; SQLite's query planner is served by `ANALYZE` (§19.17.4), not by speculative indexes.

---

### 19.17 Retention, vacuum, and garbage collection

#### 19.17.1 Retention policy

Run by the nightly maintenance job, each in its own short transaction.

| Table | Retained | Reason for the asymmetry |
| --- | --- | --- |
| `lifecycle_transitions` where `is_destructive = 0` | 90 days, or 200,000 rows, whichever is smaller | Routine phase advances are high-volume and low-value after the fact |
| `lifecycle_transitions` where `is_destructive = 1` | **730 days, floor of 10,000 rows** | These are the records anyone ever asks about. "Why did it delete my placeholder" is asked months later |
| `acquisition_decisions` | 365 days | Long enough to answer "why was this grabbed at that quality" |
| `definition_history` | Last 20 bodies per definition, plus 30 days after the definition is deleted | Undo depth versus size |
| `settings_history`, `lifecycle_policies`, `registry_versions` | **Never trimmed** | Trimming these makes recorded version numbers uninterpretable |
| `placement_passes` | 30 days | The question is always about something recent |
| `job_runs` | 30 days, or 100 runs per job | |
| `job_run_events` | 14 days, and always deleted with their run | Highest-volume table after the item cache |
| `http_cache` | Purged at `expires_at`; rows whose `parser_version` is below the registry's current value for their `source_type` are purged on startup | The expiry is spread on write (§19.11.3), so this row never runs as one burst |
| `reference_dataset_rows` | Live generation only; the previous generation is dropped in the promoting transaction | §19.11.4. Keeping a spare generation would double the largest table in the database to protect against a failure the staging state already prevents |
| `volatile_params` | Never trimmed | One row per declared parameter, bounded by the shipped registry |
| `source_contributions` | Current plus last-known-good per source slot | §19.11.2 |
| `discovery_snapshots` | Current plus two | §19.8 |
| `plex_pin_logins` | 1 hour past expiry | |
| `sessions` | Deleted 7 days past `expires_at` | |
| `library_items` with `missing_since` older than the reaping window | Hard-deleted | §19.7.3 |
| `orphan_candidates` with `resolution = 'Deleted'` | 180 days | Proof of what was removed and when |

The destructive/non-destructive split is the single most important row in this table. It is why
`is_destructive` is a stored column and why the partial index exists: retention must be able to keep
one class and trim the other cheaply, forever.

#### 19.17.2 Vacuum

`auto_vacuum = INCREMENTAL` is set at creation (§19.3.4). The nightly job runs
`PRAGMA incremental_vacuum(N)` with a bounded page count so the reclaim never becomes a long stall,
and a full `VACUUM` is offered only as an explicit operator action from the doctor page, with a
warning that it needs free disk space equal to the database size and blocks writes for its duration.

A full `VACUUM` is never automatic. It is exactly the kind of long, exclusive, disk-hungry operation
that should not start on its own at 3am on a machine with 2 GB free.

#### 19.17.3 Asset garbage collection

Mark-and-sweep, not reference counting.

1. Mark: walk every table that can reference an asset — `base_posters`, `render_cache`,
   `pack_assets`, `local_asset_files`, and asset references inside definition bodies — and set
   `last_used_at` on each asset reached.
2. Sweep: delete `assets` rows whose `last_used_at` is older than the grace window (default 7 days)
   and which no `RESTRICT` constraint protects, then unlink the files.
3. Reconcile: sample the filesystem for files with no `assets` row and delete them after the same
   grace window.

Reference counting was rejected because a counter that drifts either leaks storage silently or
deletes a live asset, and there is no way to tell which without a full walk — at which point the walk
is the mechanism and the counter is decoration. The grace window makes the race between "asset
created" and "reference committed" harmless.

`ON DELETE RESTRICT` on `base_posters.asset_id`, `render_cache.output_asset_id`, and
`pack_assets.asset_id` is the backstop: even a buggy sweep cannot delete a pristine original, a
render still bound to a Plex upload, or a font a pack requires.

#### 19.17.4 Statistics

`ANALYZE` runs after the initial library cache build and after any bulk import; `PRAGMA optimize`
runs on a schedule and at clean shutdown. SQLite's planner makes poor choices on a large table it has
no statistics for, and the item cache goes from empty to 50,000 rows during onboarding — which is
precisely when a first-time user is forming an opinion about whether the product is slow.

---

### 19.18 Traceability: what this schema discharges

Every commitment made in another document that this schema was required to discharge, and where it
is discharged. A row without a concrete target is a defect in this document.

This table survives consolidation because the seam it bridges does. The schema is deliberately one
document rather than scattered into the subsystems it serves (§19.1.1), so something has to show that
each subsystem's demands are met — and a reader checking one demand should not have to read 2,300
lines to find out. It is a record of where things are, never a second statement of what they are.

#### From the scope ledger

| Obligation | Discharged by |
| --- | --- |
| §2.6 — visibility stored as a set of principals from day one; per-user must not need a migration | `principals` (§19.6.1), `placement_visibility` (§19.13.3); tested by I-DATA-5 |
| §2.6 — whole-audience values the only ones the launch GUI can write | Three seeded principal rows; enforced in the API layer, not the schema — the schema must remain able to store the others |
| §2.6 — placeholder-marked items identifiable at schema level for hub filtering | `library_items.is_placeholder` + `ix_library_items__placeholder` (§19.7.3) |
| §2 — linked collections deleted; multi-library targeting structural | `definition_libraries` (§19.9.3); no grouping or link table exists |
| §2 — multi-collection configs (per-user, per-franchise) | `managed_collections.variant_key` (§19.9.5) |
| §2 — self-healing rating keys | `managed_collections.rating_key` nullable + `missing_since` + `heal_count` (§19.9.5) |
| §2 — server filesystem browser jailed to configured roots | `asset_roots` with `purpose = 'Browse'` (§19.14.4) |
| §2 — permission model: admin surface, per-user schema | `users`, `principals` (§19.6) |

#### From *Lifecycle*

| Obligation | Discharged by |
| --- | --- |
| §17.2 — subject with four axes, reference set, `evidenceAt`, `stale`, `policyVersion` | `lifecycle_subjects` (§19.12.1) |
| §17.1.6 — one subject per library item, reference-counted | `ux_lifecycle_subjects__identity`; `lifecycle_references` (§19.12.2) |
| §17.10 — append-only transition log with the evidence array | `lifecycle_transitions` (§19.12.4) |
| §17.10 — longer retention for destructive triggers | `is_destructive` column + §19.17.1 |
| §17.8.2 — destructive-action allowlist | `CHECK` on `lifecycle_transitions` (§19.12.4) |
| §17.9 — intend / execute / confirm, reconciled at startup | `lifecycle_intents` (§19.12.5) |
| §17.5.1 — marker independent of filenames | `library_items.is_placeholder`; `orphan_candidates` reports rather than pattern-matches (§19.12.6) |
| §17.3.1 — `releaseDateBasis` recorded | `lifecycle_subjects.release_date_basis` (§19.12.1) |
| §18 — grab decisions reproducible from the record alone | `acquisition_decisions` (§19.12.7) |
| §17.11 — `policyVersion` keeps historical decisions interpretable | `lifecycle_policies`, never trimmed (§19.5.4, §19.17.1) |
| §17.12 — placeholder root path changes leave old paths sweepable | `placeholder_roots.retired_at`, rows retained (§19.12.6) |
| G7 — ambiguous match never acted on until resolved | `lifecycle_subjects.is_ambiguous`, `ambiguous_matches` (§19.7.4) |

#### From *Placement and ordering*

| Obligation | Discharged by |
| --- | --- |
| §15.1 — three participant types in one space | `placement_participants.type` (§19.13.1) |
| §15.1 — anchors identified exactly, not inferred | `is_deletable` read from Plex (§19.13.1) |
| §15.4.4 — per-library, per-adjacent-pair insertion accounting | `placement_gaps` (§19.13.4) |
| §15.4.7 — idempotency check before any API call | `placement_surface_state.desired_hash` / `verified_hash` (§19.13.7) |
| §15.6.2 — original sort titles recorded before first mutation | `sort_title_originals` (§19.13.5) |
| §15.6.4 — byte-exact restoration | `original_value BLOB` + `original_sha256` + `restore_verified` (§19.13.5) |
| §15.7 — randomisation seeded by rotation epoch, not per pass | `randomization_epochs` (§19.13.7) |
| §15.8 — unknown participants recorded, never evicted | `type = 'Unknown'` rows (§19.13.1) |
| §15.9 — moves and rebalances per pass surfaced | `placement_passes` (§19.13.7) |
| §15.4.5 — non-convergence is a durable, visible state | `placement_surface_state.is_non_convergent` (§19.13.7) |
| Consent is per library, with a per-collection override (D-014) | `adoption_consents.scope` supports all three scopes (§19.13.6) |

#### From *The definition layer*

| Obligation | Discharged by |
| --- | --- |
| §13.2.4 — cache for the server-discovered layer, per library | `discovery_snapshots` and children (§19.8) |
| Invalidation on library scan and Plex version change (D-017) | `libraries.scanned_at`, `plex_server.version` vs `discovery_snapshots.plex_version` (§19.8) |
| §13.2.4 — a definition records the library a discovered field was authored against | `definition_field_uses.authored_library_id` (§19.8) |
| §13.8 — `registryVersion` on definitions, resolvable to a vocabulary | `definitions.registry_version` + `registry_versions` (§19.9.1, §19.5.4) |
| §13.2.2 — null distinguished from unavailable | `library_item_state.ratings_json` NULL vs JSON null (§19.7.6) |
| §13.6.2 — `affirmativeEmpty` decides whether zero items is failure | `source_contributions.affirmed_empty` (§19.11.2) |
| §13.6.1 — per-source circuit breaker state | `source_health` (§19.11.1) |

#### From *Product overview* and *The definition layer*

| Obligation | Discharged by |
| --- | --- |
| §12.9 — canonical JSON body as single source of truth, hot columns for indexing only | `definitions` + the derived-column rule (§19.1.5, §19.9.1) |
| §12.9 — last N versions of each body retained | `definition_history` (§19.9.2) |
| §12.10 — deleting with inbound references requires a cascade choice | `definition_refs` + `ix_definition_refs__inbound` (§19.9.3) |
| §12.10 — a definition referencing an unavailable field is flagged, never dropped | `definition_validations.status = 'Degraded'` (§19.9.4) |
| §12.8 — packs install disabled; degraded state shown explicitly | `packs.is_enabled` default 0, `packs.state` (§19.10) |
| §11.1 — failed sources freeze at last-known-good | `source_contributions.is_last_good` (§19.11.2) |
| §11.2 — reconciliation idempotent | `managed_collection_items` as the diff baseline (§19.9.5) |
| §16 — base art captured once, content-addressed, deduplicated | `base_posters` PK on item, `assets` unique on sha256 (§19.14.1, §19.14.2) |
| §16 — render key skips unchanged uploads | `render_cache.render_key` (§19.14.3) |
| §10 — persisted job state, per-collection schedules | `jobs`, `job_runs` (§19.15) |
| §10 — doctor page findings | `doctor_findings`, §19.15 |
| §2 — library item cache, canonical ID mapping | `library_items`, `library_item_ids`, `id_mappings` (§19.7) |

---

### 19.19 Invariants the schema cannot express

Recorded here so they are not assumed to be enforced by the database. Each needs a test.

1. **Derived columns equal their projection.** `CHECK` constraints cannot call application code
   (§19.1.5).
2. **`reference_count` equals `COUNT(*)` over `lifecycle_references`.** A trigger could maintain it,
   but triggers that fire during bulk reconciliation are a performance cliff and a debugging hazard;
   the invariant is maintained in the writing transaction and asserted by a test and by
   `afisharr db reproject`.
3. **At most one `is_last_good` contribution per source slot** is enforced by a unique partial index,
   but "the last-good row's `params_hash` matches the current parameters" is not (§19.11.2).
4. **A subject's `presence` agrees with the filesystem and with Plex.** Only confirmable by observing
   both; that is what the confirm step and the doctor page do.
5. **`placement_gaps` estimates track reality.** They cannot, exactly — Plex's stored positions are
   unreadable. This is why §19.13.4 is accounting rather than truth, and why verification by read-back
   remains mandatory.
6. **Definitions in `definition_history` are valid documents.** History is retained verbatim,
   including bodies that a later registry version would reject. That is deliberate: rewriting history
   to satisfy a current validator destroys the forensic value.
7. **Asset files exist for asset rows.** Cross-store, checked by sampling (§19.14.1).
8. **Enum tokens outside a `CHECK`ed column are members of the registry.** Enforced by the validation
   pipeline (§19.1.4).
## 20. Invariants

This section is the single home for every test obligation in the product. The design work behind
this plan previously carried its own obligation lists — one each for lifecycle, placement, the data
model, and the interface — and those four lists are absorbed here. §20.17 records where each one
landed, so the rationale each list carried is traceable rather than lost.

That absorption removes a drift risk rather than managing one. While the same obligation was stated
in two places, a check had to assert that each cited section still existed. One statement per
obligation needs no such check. What each invariant keeps is its *Source* line — the section it
derives from — because that is the context a reviewer needs and the only part that was ever worth
duplicating.

Invariants are my own requirements, stated in the first person. The audit evidence that motivated
them lives in internal working notes that are never published; each finding there names the
invariants it produced, so the reasoning is one lookup away without being restated here.

### 20.1 How to read an invariant

```
I-<GROUP>-<n> — <one-line requirement, first person, present tense>
  Statement   what must always be true
  Prevents    the concrete failure this exists to make impossible
  Test        the regression test that fails if it is violated
  Source      the design section this derives from
```

**Severity is not a field, deliberately.** Every invariant here is a build-failing property. A
property that is merely desirable is a requirement, and requirements live elsewhere in this plan. If
an invariant below turns out not to be worth failing a build over, the correct response is to delete
it, not to downgrade it — a list with a "nice to have" tier becomes a list where nothing is
mandatory.

**Tests named here are obligations, not implementations.** Several are property tests over generated
input rather than examples; where that is the case the statement says so, because "test with a few
cases" and "test with ten thousand generated cases" are different commitments.

### 20.2 The seven recurring failure patterns

The invariants from §20.3 onward are individually unremarkable. What makes them worth writing down is
that they are not independent: almost all of them are instances of seven patterns, and each pattern
produced multiple distinct real-world bugs. Naming the patterns matters more than any single
invariant, because a reviewer who has internalised seven patterns catches the eighth instance; a
reviewer holding fifty rules catches only the fifty.

**P1 — Absence of evidence used as evidence**

A fetch fails, a lookup returns nothing, a service is unreachable — and the empty result is fed into
a decision as though it were a fact about the world. The system then acts: deletes, re-adds,
re-requests, empties.

This is the single most productive bug pattern in this domain, because the code that does it always
looks defensive. It is written in a `catch` block, with a comment explaining that the fallback is the
safe one.

**Counter-rule:** a failed observation produces *no* fact. Code paths distinguish three states —
present, known-absent, and unobservable — and only the first two are inputs to a decision.

**P2 — "Safe" defined from the tool's perspective**

Closely related and worth separating, because the fix is different. When a fallback is chosen, the
question "safe for whom?" is usually answered, implicitly, as *safe for the tool's job*. Cannot tell
whether this is already downloaded? Assume not, so we do not miss it — which floods the download
client. Cannot tell whether this collection has the right type? Recreate it — which destroys its
rating key and every piece of placement state attached to it.

**Counter-rule:** the safe direction is always the one that changes least in the user's library. When
a fallback is written, the comment must name whose interest it protects, and the answer must be the
user's.

**P3 — Reversibility approximated rather than exact**

Undo that produces something *like* the original rather than the original. A restored poster
re-encoded and cropped; a restored sort title without its lock state; a cleanup that removes the
record but not the artefact.

These are invisible in testing because the result looks right. They surface as a slow accumulation of
drift across a library, and by the time anyone notices, the originals are gone.

**Counter-rule:** anything I overwrite is captured byte-exactly first, and restoration is verified by
comparison against a digest, not by the write succeeding.

**P4 — Identity carried by a mutable or derived value**

Filenames, URLs, rating keys, titles. Each is stable enough to work in development and unstable
enough to fail in production, and the failure is silent because the lookup does not error — it
matches the wrong thing or nothing at all.

**Counter-rule:** identity is an immutable internal identifier. Everything Plex or a provider assigns
is a *binding* that is expected to change and is rebound by reconciliation.

**P5 — Recovery running as an error handler**

The most destructive operation in a subsystem executing at the moment the system understands the
situation least — inside a `catch`, after something has already gone wrong, with no record of what
was about to be lost.

**Counter-rule:** destructive recovery is planned from accounting, recorded before execution, and
bounded. If an operation is dangerous enough to need a preview, it is too dangerous to run from a
`catch`.

**P6 — Silent fallback to a different target**

A configured instance is missing, so the default is used. A configured library is gone, so the first
one is used. The operation succeeds, against the wrong thing, and reports success.

**Counter-rule:** a missing configured target is an error that names what was configured and what was
found. I never substitute a target the user did not choose.

**P7 — Two code paths for one rule**

A quick path and a full path; a create path and an update path; a preview renderer and a production
renderer. They agree on the day they are written and diverge thereafter, and the divergence is
discovered as a bug report that reproduces in one mode only.

**Counter-rule:** one implementation per rule, with the variation expressed as data rather than as a
second branch.

### 20.3 Evidence and destruction

The group with the strictest obligations, because these govern the code that deletes user data.

**I-EVID-1 — I never treat a failed observation as a fact.**
- *Statement:* every value consumed by a decision is one of present, known-absent, or unobservable.
  Unobservable values are inputs to no decision except the decision to mark the subject stale.
- *Prevents:* a provider outage causing mass deletion, mass re-request, or mass collection emptying.
- *Test:* property test. For every generated combination of provider failures — timeout, 5xx, 404,
  malformed body, challenge page, empty-but-well-formed — assert that zero destructive actions are
  emitted and that stored state is byte-identical before and after the pass.
- *Source:* *lifecycle and placeholders*, §1.3, G1.

**I-EVID-2 — A placeholder is deleted only under an allowlisted trigger carrying its evidence.**
- *Statement:* the seven triggers enumerated in *lifecycle and placeholders*, §8.2, are exhaustive. No
  other condition deletes, and every deletion records the evidence that justified it.
- *Prevents:* deletions nobody can explain months later, which is the failure that makes a tool of
  this kind untrustworthy in a way no feature compensates for.
- *Test:* two tests. (a) Property test over generated evidence sequences: no placeholder is deleted
  without an allowlisted trigger and a complete evidence array. (b) Database-level test: an insert
  into `lifecycle_transitions` with `is_destructive = 1` and an off-list trigger is rejected by the
  schema, not by application code.
- *Source:* *lifecycle and placeholders*, §8.2; *the data model*, §12.4.

**I-EVID-3 — A 404 from a provider is not evidence about my library.**
- *Statement:* an item that a metadata provider no longer serves keeps its state and is marked stale.
  Upstream deletion, merging, or reindexing is a fact about the provider.
- *Prevents:* a provider's catalogue cleanup propagating into deletions or recreations locally.
- *Test:* a provider fake returns 404 for a subject that previously resolved. Assert: state
  unchanged, `is_stale` set, no intent created, no side effect executed.
- *Source:* *lifecycle and placeholders*, §13.

**I-EVID-4 — I handle every error class the same way unless a difference is specified and justified.**
- *Statement:* error-handling branches do not vary by status code or exception type unless the
  variation is a stated rule with a recorded reason. Two adjacent branches of one `catch` may not
  reach opposite conclusions about whether to act.
- *Prevents:* a subtle asymmetry — act on one failure class, do nothing on another — that nobody
  designed and nobody can defend.
- *Test:* code review checklist item plus a fault-injection matrix test: for each error class, record
  the actions emitted; assert the action set is identical across classes except where a documented
  rule says otherwise.
- *Source:* §20.2, P1.

**I-EVID-5 — `presence = Real` requires positive confirmation of a playable file.**
- *Statement:* provider or `*arr` data never establishes that real media exists. Only Plex reporting a
  media part does.
- *Prevents:* a placeholder deleted because a provider claimed availability, leaving nothing in the
  library.
- *Test:* assert `Real` is unreachable from any evidence set lacking a Plex media-part confirmation,
  across the generated transition suite.
- *Source:* *lifecycle and placeholders*, G3.

**I-EVID-6 — A subject with live references is never removed for departure.**
- *Statement:* reference count reaching zero is the only departure trigger, and the count is
  recomputed from the collections that resolved to the subject, never adjusted incrementally.
- *Prevents:* one collection's cleanup removing a placeholder another collection still wants.
- *Test:* N collections referencing one subject, interleaved add and remove operations in generated
  order. Assert exactly one file exists throughout and removal occurs only at zero.
- *Source:* *lifecycle and placeholders*, §1.6, G5.

**I-EVID-7 — A stale subject transitions on no axis.**
- *Statement:* staleness is a full stop, not a partial one. A subject whose evidence could not be
  refreshed keeps every axis.
- *Prevents:* a partially-refreshed subject advancing on the axis that happened to resolve, producing
  a composite state that no evidence supports.
- *Test:* for each single-provider failure, assert all four axes are unchanged.
- *Source:* *lifecycle and placeholders*, G1.

**I-EVID-8 — An ambiguous canonical match is acted on by nothing.**
- *Statement:* a canonical identifier resolving to more than one library item blocks every action on
  that subject until a human resolves it, and the resolution is persisted.
- *Prevents:* acting on the wrong one of two matching items — including overwriting its poster or
  deleting it as a placeholder.
- *Test:* seed two items sharing a TMDB id. Assert no writes of any kind, a durable
  `ambiguous_matches` row, and that a recorded resolution unblocks the subject on the next pass
  without re-detection.
- *Source:* *lifecycle and placeholders*, G7; *the data model*, §7.4.

### 20.4 Reversibility

Everything I write into a user's library must be removable, and removal must restore what was there —
not something equivalent to it.

**I-REV-1 — I capture the original before the first modification, byte-exactly.**
- *Statement:* no field or artefact is overwritten until its prior value is stored with a digest. If
  capture fails, the modification does not happen.
- *Prevents:* an unrecoverable overwrite, which is the one failure class no later fix repairs.
- *Test:* fault-inject a capture failure; assert the modification is not attempted and the item is
  untouched.
- *Source:* *placement and ordering*, §6.2; *the data model*, §13.5, §14.2.

**I-REV-2 — Restoring a poster returns the original bytes, not a re-render of them.**
- *Statement:* removing overlays uploads the stored base poster exactly as captured. No resize, no
  re-encode, no format conversion, no crop.
- *Prevents:* a library that has been through an overlay cycle differing permanently from one that
  has not — cropped to a different aspect, re-encoded at a lower quality, converted to a format the
  user did not choose. This degrades silently and is not recoverable once the originals are gone.
- *Test:* capture a base poster of an unusual aspect ratio and a non-default format. Apply overlays,
  then reset. Assert the bytes Plex serves afterwards hash equal to the captured original.
- *Source:* *the product spec*, §6; Appendix A2.

**I-REV-3 — A sort-title round trip restores the value, its presence, and its lock state.**
- *Statement:* promote followed by demote restores the exact original string byte for byte; an item
  that had no sort title has none afterwards; an item whose sort title was unlocked is unlocked
  afterwards.
- *Prevents:* two silent, permanent failures — writing an explicit sort title onto an item that never
  had one, and leaving a metadata field locked so the server's own agents can never refresh it again.
  Neither is visible in the GUI and neither is ever reported.
- *Test:* three fixtures — no sort title, a locked sort title, an unlocked sort title. Round trip
  each; assert value, presence, and lock flag independently.
- *Source:* *placement and ordering*, §6.4; *the data model*, §13.5; Appendix A1.

**I-REV-4 — Uninstalling leaves the library as it was found.**
- *Statement:* a first-class teardown operation restores every base poster, strips every applied
  sort-title prefix and restores its lock state, removes every applied label, deletes every managed
  collection and placeholder, and restores native hub placement. It is resumable after a crash or a
  cancel, and it reports everything it could not restore rather than skipping silently or aborting.
- *Prevents:* the library being permanently marked by having run the tool — the strongest possible
  disincentive to trying it, and the failure that no later feature compensates for.
- *Test:* integration test against a fake Plex. Snapshot server state, run a full sync cycle
  including overlays, placeholders, and placement, run teardown, assert the snapshot matches — with
  an explicit allowlist of legitimately changed fields, and an assertion that the allowlist is empty
  for artwork bytes, sort titles, lock states, and labels. A second variant kills the process midway
  and asserts that a resumed teardown reaches the same end state.
- *Source:* D-022 (decided 2026-08-08); journey owned by *the interface design*, §4.6.

**I-REV-5 — Label removal failures are reported, never ignored.**
- *Statement:* labels are a functional marker that filtering and hub replacement read. A failed label
  write leaves the item in a recorded inconsistent state and raises a finding.
- *Prevents:* an item that has been reset but still carries the marker, so every later filter
  disagrees with reality — and no log line exists to explain why.
- *Test:* fault-inject a label-removal failure; assert a doctor finding exists, the item is not
  recorded as reset, and the next pass retries.
- *Source:* *the data model*, §15; Appendix A2.

**I-REV-6 — Consent is required before I modify an object I did not create, and is recorded.**
- *Statement:* an adopted collection's sort title is modified only under a consent record resolvable
  at the time of the write, and the consent that authorised it is stored alongside the captured
  original.
- *Prevents:* modifying a user's own metadata on the basis of a setting nobody remembers granting.
- *Test:* attempt a sort-title write on an adopted collection with no consent row; assert refusal, a
  finding, and no write. Then grant library-scope consent and assert the write proceeds and records
  the consent id.
- *Source:* *placement and ordering*, §6.3; *the data model*, §13.6.

### 20.5 Identity and binding

**I-ID-1 — Correctness never depends on a filename.**
- *Statement:* no decision reads, parses, or pattern-matches a filename. Filenames are a hint for
  human legibility and for narrowing an orphan sweep, never an input to an action.
- *Prevents:* files stranded forever because they were written under a naming convention a later
  version no longer recognises — an accumulation that grows with every convention change and is
  invisible until someone audits disk usage.
- *Test:* rename every placeholder file on disk to a random string. Assert the pass still identifies
  every placeholder correctly, deletes nothing it should not, and reports the renamed files as sweep
  candidates rather than acting on them.
- *Source:* *lifecycle and placeholders*, §5.1; *the data model*, §12.6.

**I-ID-2 — Correctness never depends on a URL.**
- *Statement:* whether an artwork asset is my own output or a pristine original is determined by
  content digest and a recorded provenance row, never by comparing the server's artwork URL against a
  remembered one.
- *Prevents:* the worst failure in the render subsystem — capturing an already-overlaid poster as the
  base, so overlays composite on top of overlays and the original is gone. URL comparison fails in the
  unsafe direction: an unrecognised URL format yields "no match", and "no match" means "this is a new
  original, capture it".
- *Test:* feed the base-poster capture path an artwork reference in an unrecognised format whose bytes
  are a known internal render. Assert it is refused as a base, flagged suspect, and raises a finding —
  and specifically assert it is *not* captured.
- *Source:* *the product spec*, §6; *the data model*, §14.2; Appendix A3.

**I-ID-3 — Internal identity is immutable; every external identifier is a rebindable binding.**
- *Statement:* rating keys, section keys, hub identifiers, and provider ids are columns, never primary
  keys. Reconciliation rebinds them and records the heal.
- *Prevents:* state stranded by a server-side re-key — a routine consequence of removing and re-adding
  media — orphaning base posters, placement, and lifecycle records for items that still exist.
- *Test:* re-key every item in a fake library between two passes. Assert every binding is recovered,
  no base poster is orphaned, no placement is lost, and each recovery is recorded.
- *Source:* *placement and ordering*, §8; *the data model*, §1.2.

**I-ID-4 — Repeated self-healing is a reported condition, not a steady state.**
- *Statement:* heal counts are recorded per binding, and a binding healing repeatedly raises a
  finding rather than continuing quietly.
- *Prevents:* fighting another process indefinitely — recreating what something else deletes — while
  every individual pass reports success.
- *Test:* a fake that deletes a managed collection after each pass. Assert a finding appears within
  the configured threshold and that the heal count is visible.
- *Source:* *the data model*, §9.5.

**I-ID-5 — A different server is a different world.**
- *Statement:* a changed machine identifier suspends all Plex-bound state and requires an explicit
  operator decision. Nothing is auto-rebound across it.
- *Prevents:* rating keys from server A being applied to server B, which would write collections,
  posters, and placement onto arbitrary unrelated items.
- *Test:* change the machine identifier between passes. Assert zero writes and a blocking finding.
- *Source:* *the data model*, §7.1.

### 20.6 Convergence, ordering, and idempotency

**I-CONV-1 — Ordering emits the minimum number of moves.**
- *Statement:* given an actual and a desired sequence, the planner emits exactly `n − LIS` moves.
- *Prevents:* consuming a non-renewable precision budget faster than necessary. Move count here is a
  safety metric, not an efficiency one.
- *Test:* property test over random permutation pairs. Fewer than `n − LIS` is impossible; more is a
  failure.
- *Source:* *placement and ordering*, §4.2.

**I-CONV-2 — Every applied ordering plan is verified by reading back the result.**
- *Statement:* a move reporting success is not evidence that it happened. The resulting order is read
  and compared.
- *Prevents:* silent no-op moves — the documented consequence of precision exhaustion — leaving the
  system believing an order it does not have.
- *Test:* a fake whose moves silently no-op past a gap budget. Assert the mismatch is detected on the
  same pass, not the next one.
- *Source:* *placement and ordering*, §3, §4.5.

**I-CONV-3 — Rebalancing is planned from accounting, never triggered from a failure handler.**
- *Statement:* gap-budget accounting schedules a rebalance before an insertion that would exhaust a
  gap. The escalation ladder is bounded, each rung is recorded before it executes, and no rung is
  skipped.
- *Prevents:* the most destructive operation in the subsystem running inside a `catch`, at the moment
  the system has least information.
- *Test:* simulated precision exhaustion. Assert a rebalance is scheduled from accounting before any
  move fails, and that every rung entered has a recorded reason.
- *Source:* *placement and ordering*, §4.4, §4.5.

**I-CONV-4 — Anchors are never unpromoted, on any rung.**
- *Statement:* participants the server reports as non-deletable are positioned around, never removed
  and re-added, including during a full rebalance.
- *Prevents:* an unrecoverable ordering space — an unpromoted native hub cannot be restored.
- *Test:* run every rung against a library whose participants are majority anchors. Assert zero
  unpromote calls against any non-deletable participant.
- *Source:* *placement and ordering*, §1, §4.3.

**I-CONV-5 — I never evict a participant I do not manage.**
- *Statement:* an unrecognised participant in an ordering space is recorded and planned around. It is
  never removed, reordered into oblivion, or reset.
- *Prevents:* making the product unusable alongside any other process or a newer server version.
- *Test:* inject unrecognised participants at random positions. Assert they survive every rung and
  that their presence is recorded.
- *Source:* *placement and ordering*, §8.

**I-CONV-6 — Non-convergence is a visible state, not a retry loop.**
- *Statement:* a surface that cannot be ordered within the ladder stays in its last verified state,
  is marked non-convergent with the specific items that would not settle, and is surfaced.
- *Prevents:* a library churning forever, burning precision on every pass, while the UI reports
  success.
- *Test:* an unconvergeable fixture. Assert the ladder terminates, the state is recorded, and the next
  pass does not re-enter the ladder from rung 0 without new inputs.
- *Source:* *placement and ordering*, §4.5.

**I-CONV-7 — Ordering ties break deterministically.**
- *Statement:* participants sharing a position order by immutable identifier, so the desired sequence
  is identical across passes.
- *Prevents:* two participants swapping places on alternate runs — a flip-flop that consumes precision
  forever while presenting as an ordering bug.
- *Test:* many equal positions; assert the desired sequence is byte-identical across repeated
  computations and that a second pass emits zero moves.
- *Source:* *placement and ordering*, §4.1.

**I-CONV-8 — A reorder made in the interface survives a full sync unchanged.**
- *Statement:* a position edited through the home screen board is written to Plex, verified by
  read-back, and is still in place after the next scheduled pass runs against unchanged inputs.
- *Prevents:* the operator's own edit being treated as drift by the next pass and reverted — which
  makes the ordering interface untrustworthy in the one way no error message can repair.
- *Test:* reorder through the board, run a full sync, assert the resulting order is identical and
  that the pass emitted no compensating moves.
- *Source:* *placement and ordering*, §4.7; *the interface design*, §4.4.

**I-IDEM-1 — A second pass with unchanged inputs performs no writes.**
- *Statement:* not "no moves after computing a plan" — no writes at all, and for ordering, no API
  calls beyond the cheap verification read.
- *Prevents:* churn that looks harmless and is not: every unnecessary write consumes precision,
  invalidates caches, and produces audit noise that buries real events.
- *Test:* run every pass type twice against an unchanged fixture. Assert the second run's write count
  is zero, at the HTTP client boundary rather than at the application's own accounting.
- *Source:* *the product spec*, §5.2; *placement and ordering*, §4.7.

**I-IDEM-2 — Randomisation is stable within a rotation epoch.**
- *Statement:* the shuffle is seeded by epoch and surface, so repeated passes within one epoch produce
  an identical order.
- *Prevents:* a schedule that runs hourly reshuffling hourly, consuming the precision budget for
  nothing.
- *Test:* three passes within one epoch produce zero moves; advancing the epoch produces a different,
  reproducible order.
- *Source:* *placement and ordering*, §7.

**I-IDEM-3 — Randomisation never displaces a pinned participant.**
- *Statement:* flagged participants are shuffled among the positions they collectively occupy; the
  position set is preserved.
- *Prevents:* a randomised row walking through positions the user pinned deliberately.
- *Test:* assert the multiset of positions occupied by randomised participants is identical before and
  after, and that no unflagged participant moves.
- *Source:* *placement and ordering*, §7.

### 20.7 Sources and collection contents

**I-SRC-1 — Zero items is a failure unless the source affirms emptiness.**
- *Statement:* an empty result from a source that cannot distinguish "no results" from "request
  failed" freezes that source's contribution at last-known-good.
- *Prevents:* a challenge page, a rate-limit response, or a layout change emptying a user's
  collection.
- *Test:* per source adapter, feed a challenge page, a rate-limit body, and a genuine empty response.
  Assert the first two freeze and the third empties only where the adapter declares it can affirm.
- *Source:* *the product spec*, §5.1; *the engine design*, §16.2.

**I-SRC-2 — A challenge page never reaches a parser.**
- *Statement:* response validation runs before parsing and classifies the response. A challenge is a
  distinct, recorded error class.
- *Prevents:* an anti-bot page parsing to zero items and being counted as a legitimate empty list.
- *Test:* feed captured challenge-page fixtures to every scraped adapter. Assert classification as
  `Challenge` and that the parser is never invoked.
- *Source:* *the product spec*, §8; *the data model*, §11.1.

**I-SRC-3 — A frozen contribution is only reused for the same question.**
- *Statement:* last-known-good is keyed to the resolved source parameters. Changed parameters
  invalidate it.
- *Prevents:* a collection silently continuing to show results for a list the user has since
  repointed.
- *Test:* freeze a contribution, change a parameter, fail the fetch. Assert the stale contribution is
  not reused and the collection reports degraded rather than wrong.
- *Source:* *the data model*, §11.2.

**I-SRC-4 — Breaker state survives restart.**
- *Statement:* circuit-breaker state is persisted, so a restart does not reset the breaker.
- *Prevents:* a crash loop or a routine upgrade producing a burst of requests at a service that is
  already failing — the behaviour that gets keys revoked and addresses blocked.
- *Test:* open a breaker, restart the process, assert the breaker is still open and its cooldown is
  respected.
- *Source:* *the data model*, §11.1.

**I-SRC-5 — Ordering modes that require order are rejected against unordered sources.**
- *Statement:* validation refuses a definition combining source-position ordering with a source that
  does not declare a meaningful sequence, at save time.
- *Prevents:* a collection whose order is arbitrary but presents as intentional.
- *Test:* attempt the combination; assert a structured save-time error naming the offending node.
- *Source:* *the engine design*, §16.2, §7.

**I-SRC-6 — Membership reconciliation never destroys a collection to change it.**
- *Statement:* a managed collection is modified in place. It is deleted and recreated only when the
  definition is deleted or the user explicitly requests it — never as a means of correcting type,
  emptiness, or a suspected mismatch.
- *Prevents:* destroying a rating key, and with it every piece of placement, adoption, artwork, and
  hub state bound to it, in order to fix something the API could have changed.
- *Test:* fixtures for an empty collection, a wrong-subtype collection, and a collection whose
  membership fully turns over. Assert the rating key is unchanged in all three and that placement
  state survives.
- *Source:* *placement and ordering*, §8; Appendix A5.

**I-SRC-7 — One pipeline.**
- *Statement:* there is exactly one collection pipeline and one overlay pipeline. Incrementality is a
  cache concern inside them, never a second code path.
- *Prevents:* two implementations of one rule drifting, so behaviour depends on which entry point ran.
- *Test:* architectural test — assert no second reconciliation or render entry point exists, by
  asserting the call graph into the pipeline has a single root.
- *Source:* §11; §2.

**I-SRC-8 — A fallback rung never inherits the capabilities of the rung above it.**
- *Statement:* when a source falls through its endpoint ladder, the engine applies the
  `affirmativeEmpty`, `ordered`, and `deterministic` flags declared by the rung that produced the
  result, not by the source's highest rung. Falling through is itself recorded as degraded health.
- *Prevents:* the exact case the empty-result safeguard exists for. A source whose structured
  endpoint returns a typed "not found" and whose page-embedded fallback returns an empty document
  would, under a single per-source flag, carry `affirmativeEmpty: true` — correct while the primary
  worked, and collection-emptying the first time it did not. Also prevents a source running for
  weeks on a silently degraded rung, which turns a one-file repair into a permanent loss.
- *Test:* a two-rung fixture source. Force the primary to fail, return an unaffirmed empty result
  from the fallback, and assert the contribution freezes rather than empties. Repeat with the primary
  healthy and assert the same empty result is honoured. Assert both runs record the rung that
  answered, and that the fallback run reports degraded.
- *Source:* *the product spec*, §8; *the engine design*, §16.1, §16.2; D-040.

### 20.8 Acquisition

**I-ACQ-1 — I never request or grab on the basis of an unverifiable state.**
- *Statement:* if the download stack cannot be queried, the acquisition axis is frozen and no request
  or add is issued. Inability to confirm that something is already present is not permission to add
  it again.
- *Prevents:* an outage producing a flood of duplicate adds across every collection at once — the
  most user-visible way this product could misbehave, and one that reaches an external service the
  user also depends on.
- *Test:* make the download stack unreachable mid-pass. Assert zero add or request calls, the
  acquisition axis unchanged, and a recorded degradation. Explicitly assert the *count* of outbound
  acquisition calls is zero rather than asserting on internal state.
- *Source:* *lifecycle and placeholders*, §13; Appendix A4.

**I-ACQ-2 — Partial availability is not availability.**
- *Statement:* "already present" means the specific thing being requested exists. A series with some
  episodes is not a series that is present; the season list resolves to what is actually missing.
- *Prevents:* a show never completing because the presence of one episode marks the whole series
  satisfied.
- *Test:* a series fixture with 1 of 60 episodes. Assert the missing seasons are requested and the
  present one is not.
- *Source:* *lifecycle and placeholders*, §10; Appendix A4.

**I-ACQ-3 — A missing configured instance is an error, never a fallback.**
- *Statement:* when a definition names an instance, quality profile, root folder, or library that no
  longer exists, the operation fails with a message naming what was configured and what was found.
  The default is not substituted.
- *Prevents:* content silently grabbed to the wrong instance, at the wrong quality, into the wrong
  root — succeeding, reporting success, and being discovered weeks later.
- *Test:* remove each configured target in turn. Assert refusal, a finding naming both values, and
  zero outbound calls to any substitute.
- *Source:* §20.2, P6; Appendix A4.

**I-ACQ-4 — A grab decision is reproducible from its record alone.**
- *Statement:* the record contains every input, every gate and its verdict, the policy version, and
  the resolved route and overrides. Replaying it recomputes the same decision.
- *Prevents:* "why did it grab this" being unanswerable, which makes acquisition policy untunable.
- *Test:* replay every recorded decision in a generated corpus from the record alone, with no access
  to live state. Assert identical outcomes.
- *Source:* *lifecycle and placeholders*, §10; *the data model*, §12.7.

**I-ACQ-5 — Every gate is recorded, including the ones that passed.**
- *Statement:* the decision record lists all gates evaluated with their inputs and verdicts.
- *Prevents:* "passed all gates" being indistinguishable from "no gates configured".
- *Test:* assert the recorded gate set equals the configured gate set for every decision.
- *Source:* *the data model*, §12.7.

### 20.9 Rendering

**I-RENDER-1 — An overlay is never applied over an overlay.**
- *Statement:* every render composites the stored pristine base with the current template and current
  state. My own output is never an input to a render.
- *Prevents:* compounding overlays — the most visible possible failure of this subsystem, and one that
  destroys the original irreversibly once the base is lost.
- *Test:* apply overlays, change the template, apply again. Assert the second render's input digest
  equals the stored base digest, and that the output is identical to rendering the new template once
  from a clean library.
- *Source:* *the product spec*, §6.

**I-RENDER-2 — A base poster of uncertain provenance is quarantined, not used.**
- *Statement:* a capture that cannot be established as pristine is marked suspect, excluded from
  compositing, and raised as a finding requiring an operator decision.
- *Prevents:* the failure in I-RENDER-1 arriving through the back door — a restored backup, a
  reinstall, an item touched by another process.
- *Test:* construct each provenance-uncertain scenario. Assert quarantine, no compositing, and a
  finding.
- *Source:* *the data model*, §14.2.

**I-RENDER-3 — Preview and production use the same renderer.**
- *Statement:* one renderer implementation serves the editor preview and the applied output.
- *Prevents:* a template that previews correctly and applies wrongly, which makes the editor useless
  precisely for the templates that need it most.
- *Test:* render a corpus of templates through both entry points; assert byte equality.
- *Source:* *the engine design*, §6.

**I-RENDER-4 — An unresolved variable skips its element.**
- *Statement:* an element whose bound value is null or unavailable is not drawn. It is never drawn
  blank and never drawn with a placeholder string.
- *Prevents:* empty badges and literal `undefined` text on posters.
- *Test:* render every element type with null and with unavailable inputs; assert the element is
  absent from the output and that the render audit records why.
- *Source:* *the engine design*, §12.2.

**I-RENDER-5 — Formatters are pure.**
- *Statement:* a formatter is a function of value, locale, and arguments. No clock, no I/O, no
  randomness.
- *Prevents:* an unsound render cache key — if output can vary independently of the hashed inputs, the
  key does not identify the output and stale posters persist indefinitely.
- *Test:* property test — every formatter invoked twice with identical inputs produces identical
  output; an architectural test asserts no formatter reaches the clock or the network.
- *Source:* *the engine design*, §15; *the product spec*, §6.

**I-RENDER-6 — The render key covers everything that can change the output.**
- *Statement:* the key includes the base digest, template identity and version, the state snapshot,
  and the renderer version.
- *Prevents:* two opposite failures — a renderer improvement that never reaches any user because every
  cache entry still matches, and a stale poster that survives a template change.
- *Test:* mutate each key component in turn; assert the key changes and a re-render occurs. Assert
  that changing nothing produces a cache hit and no upload.
- *Source:* *the data model*, §14.3; *the product spec*, §6.

**I-RENDER-7 — A mapped icon without a fallback fails validation.**
- *Statement:* a value-to-asset table must declare a fallback, because a later release may add an
  enumeration value the table has never seen.
- *Prevents:* packs silently rendering nothing for new values.
- *Test:* attempt to save a mapping without a fallback; assert a structured save-time error.
- *Source:* *the engine design*, §6.

### 20.10 Storage, crash safety, and concurrency

**I-DATA-1 — A single startup pass repairs any crash.**
- *Statement:* every side effect follows intend, execute, confirm, and startup re-drives every
  unconfirmed intent. Both directions of every operation are idempotent.
- *Prevents:* a hard kill mid-write leaving a half-created placeholder that no pass ever resolves.
- *Test:* crash injection between every pair of steps, for every intent kind. One startup pass reaches
  a consistent state.
- *Source:* *lifecycle and placeholders*, §9; *the data model*, §12.5.

**I-DATA-2 — No transaction spans external I/O.**
- *Statement:* a database transaction is never open across an HTTP call or a library filesystem write.
- *Prevents:* a hung socket blocking every writer for the duration of a timeout, which presents as the
  whole application freezing.
- *Test:* architectural test asserting no transaction guard is live across a client call; plus a
  runtime assertion in debug builds.
- *Source:* *the data model*, §4.4.

**I-DATA-3 — A user's write always beats a background pass.**
- *Statement:* a pass whose definition changed underneath it discards its results for that definition
  and re-queues. It never merges and never overwrites.
- *Prevents:* an edit silently reverted by a job that started before it — the failure that makes
  people stop trusting an interface.
- *Test:* save a definition mid-pass; assert the pass discards, the saved body survives, and the
  re-queue occurs.
- *Source:* *the data model*, §4.5.

**I-DATA-4 — Derived columns are recomputable and are never a second source of truth.**
- *Statement:* dropping every derived column and reprojecting from canonical bodies is a no-op.
- *Prevents:* the "hot columns for indexing only" arrangement degrading into two authorities that
  disagree.
- *Test:* CI assertion over a seeded database that projection of each body equals the stored columns.
- *Source:* *the data model*, §1.5.

**I-DATA-5 — Per-user targeting requires no schema migration.**
- *Statement:* against the shipped launch schema, a per-user principal and a visibility row
  referencing it can be inserted successfully.
- *Prevents:* the "modelled for per-user, admin-only surface" commitment turning out to be an
  intention, discovered at the point where fixing it is a migration on live data.
- *Test:* insert a user principal and a visibility row against the launch schema; assert success. This
  test ships in the launch release even though nothing uses the capability.
- *Source:* D-007 (§17.8).

**I-DATA-6 — Destructive audit records outlive everything.**
- *Statement:* retention trims routine records aggressively and keeps records of destructive actions
  for the long window, independent of volume.
- *Prevents:* the record of why something was deleted being trimmed away by the volume of records
  about things that were not.
- *Test:* generate a million routine transitions and one destructive one; run retention; assert the
  destructive record survives.
- *Source:* *the data model*, §17.1.

**I-DATA-7 — A newer schema is never opened by an older binary.**
- *Statement:* a database at an unknown migration version refuses to start, naming both versions.
- *Prevents:* a downgrade silently corrupting data it does not understand.
- *Test:* open an N+1 database with an N binary; assert refusal and a message naming both.
- *Source:* *the data model*, §3.5.

**I-DATA-8 — Migrations run only after a backup exists.**
- *Statement:* a pending migration triggers an online backup first. A failed backup blocks the
  migration.
- *Prevents:* forward-only migrations having no recovery path, which is the trade forward-only makes
  and only survives if the backup is real.
- *Test:* make the backup path unwritable; assert the migration does not run and startup fails
  loudly.
- *Source:* *the data model*, §3.2, §3.5.

**I-DATA-9 — Garbage collection cannot remove an irreplaceable asset.**
- *Statement:* pristine base posters, renders bound to a live upload, and pack-required assets are
  protected by database constraints, not only by the sweep's logic.
- *Prevents:* a bug in a background sweep destroying the one asset class that cannot be re-fetched.
- *Test:* run the sweep against a database where every asset appears unreferenced; assert protected
  classes survive and the constraint, not the sweep, is what stops it.
- *Source:* *the data model*, §17.3.

**I-DATA-10 — One lifecycle subject exists per identity, enforced by the database.**
- *Statement:* the identity tuple — library, id space, id value, season — is unique at the schema
  level. Two writers racing to create the same subject produce one row and one constraint violation.
- *Prevents:* two subjects for one title, each with its own placeholder file and its own reference
  count, deleting each other's work.
- *Test:* concurrent inserts of the same identity tuple. Assert exactly one row and one violation,
  raised by the index rather than by application code.
- *Source:* *the data model*, §12.1.

**I-DATA-11 — Every released schema migrates forward to head.**
- *Statement:* a database created by any released version reaches the current schema by running
  migrations, and passes an integrity and foreign-key check afterwards.
- *Prevents:* an upgrade path that works only from the previous release, discovered by the user who
  skipped one.
- *Test:* a fixture database per released version. Migrate each to head, then run
  `PRAGMA integrity_check` and `PRAGMA foreign_key_check`.
- *Source:* *the data model*, §3.2, §3.3.

**I-DATA-12 — A cached response is never read by a parser version other than the one that keyed it.**
- *Statement:* the HTTP cache key is computed over the request *and* the source rung's
  `parserVersion`. Bumping the version misses on every entry the previous parser wrote.
- *Prevents:* a parser fix reaching nobody. The response bytes are unchanged, so an input-only key
  matches every stale entry and the corrected interpretation is not applied until the TTL expires —
  the same failure the renderer version was added to the render key to prevent (*the product spec*,
  §6), in the subsystem that did not get the safeguard.
- *Test:* populate the cache at `parserVersion: 1`, bump to 2, and assert the next request misses,
  refetches, and stores under a different key. Assert the version is inside the digest by confirming
  that no read path filters on it — a filtered read would leave the old rows matching.
- *Source:* *the data model*, §11.3; *the engine design*, §16.1; D-043.

**I-DATA-13 — A reference dataset import is all-or-nothing.**
- *Statement:* a bulk dataset is staged at a new generation, verified, and promoted in one
  transaction. A truncated download, a changed column layout, or a parse failure leaves the previous
  generation live and records the failure with its reason.
- *Prevents:* a half-applied import reading, at query time, exactly like a complete one whose
  provider dropped half its rows — so the engine treats truncation as a fact about the world and
  acts on it. This is pattern P1 (§20.2) reaching the system through a bulk import rather than through
  a source fetch.
- *Test:* fixtures for a truncated file, a file with an extra column, and a file with a corrupt row.
  Assert in all three that the live generation is unchanged, that reads still return the previous
  data, and that `import_state` is `Failed` with a reason. Assert a clean import promotes and drops
  the previous generation in the same transaction.
- *Source:* *the data model*, §11.4; D-042.

### 20.11 Validation and the definition layer

**I-DEF-1 — Invalid definitions are rejected at save, never at render.**
- *Statement:* field existence, operator legality, value types, enum membership, formatter pipelines,
  ordering compatibility, seed presence, reference integrity, cron parsing, and regex compilation are
  all checked when the document is saved.
- *Prevents:* the "the pack renders nothing and nobody knows why" class of bug.
- *Test:* a corpus of invalid documents, one per validation rule; assert each is rejected with a
  structured error carrying a JSON pointer to the offending node.
- *Source:* *the engine design*, §17, §9.

**I-DEF-2 — A definition referencing an unavailable field degrades visibly, never silently.**
- *Statement:* an unavailable server-discovered field causes local-evaluation fallback and a recorded
  degraded state. The predicate is never dropped.
- *Prevents:* a filter quietly ceasing to filter after a server upgrade, changing collection contents
  with no indication why.
- *Test:* remove a discovered field between passes; assert fallback, a degraded validation row, and
  identical membership to local evaluation.
- *Source:* *the engine design*, §10, §12.4.

**I-DEF-3 — Export and import round-trip exactly.**
- *Statement:* `import(export(x))` equals `x` after canonicalisation, byte for byte.
- *Prevents:* a definition that changes meaning by being exported and reimported.
- *Test:* property test over generated definitions of every kind.
- *Source:* *the engine design*, §1.4.

**I-DEF-4 — A pack upgrade never clobbers a user fork, and a fork never blocks a pack upgrade.**
- *Statement:* pack-origin documents are replaced; forked documents are untouched and reported as
  behind upstream.
- *Prevents:* the two failure modes that make a pack system unusable — losing user edits, or freezing
  content updates.
- *Test:* fork a pack definition, upgrade the pack, assert the fork is unchanged, the pack document is
  updated, and the drift is reported.
- *Source:* *the engine design*, §10.

**I-DEF-5 — Removed registry keys are never reused.**
- *Statement:* a key that has been removed or deprecated is permanently retired, including
  user-defined computed fields, which tombstone rather than delete.
- *Prevents:* an old definition silently acquiring a new meaning.
- *Test:* attempt to create a computed field reusing a tombstoned key; assert rejection.
- *Source:* *the engine design*, §18; *the data model*, §8.1.

**I-DEF-6 — Computed fields cannot compose.**
- *Statement:* a computed field's operands are registered non-computed numeric fields. One operation,
  two operands, no constants, no nesting.
- *Prevents:* an expression language arriving by increments, with no decision ever having been taken
  to build one.
- *Test:* attempt a computed field referencing another computed field, a three-operand form, and a
  constant operand; assert each is rejected at save.
- *Source:* *the data model*, §8.1.

**I-DEF-7 — Empty-child quantifier semantics are exactly as specified.**
- *Statement:* over zero children, `any` is false, `none` is true, `all` is vacuously true, and
  `countGte: 1` is false.
- *Prevents:* "all episodes watched" matching a show with no episodes, silently, in a rule that drives
  acquisition or deletion.
- *Test:* table test over the empty-child case for every quantifier.
- *Source:* *the engine design*, §5, §14.

**I-DEF-8 — A stored definition contains no unresolved parameterization.**
- *Statement:* pack variables and template expansion are resolved by the installer. Every row in
  `definitions` is a concrete document: no variable references, no conditionals, no unexpanded
  templates, whatever its origin.
- *Prevents:* two properties failing silently at once. Validation-at-save stops being a guarantee,
  because a document with an unresolved variable cannot be checked against the field registry until
  something resolves it; and history, export, and the fork-versus-upstream comparison all lose
  information, because two documents differing only in an unresolved variable diff as identical.
- *Test:* install a pack whose manifest declares variables and an `expand` over a registry
  enumeration. Assert one row per enumeration member, assert no stored body matches the substitution
  syntax, and assert every stored body passes the full §20.11 validation sequence unaided. Then
  upgrade the pack and assert the stored variable values re-materialize the unforked definitions
  while forks are untouched.
- *Source:* *the engine design*, §8.1, §1.1, §1.4; *the data model*, §10; D-044.

### 20.12 The lifecycle state machine

These four are properties of the machine itself rather than of the evidence feeding it, which is why
they sit apart from §20.3.

**I-LIFE-1 — Every legal transition is exercised and every illegal one is refused.**
- *Statement:* the set of legal (from, to, trigger) triples is closed and enumerable. Each is
  exercised; each triple outside the set is refused.
- *Prevents:* a transition nobody designed becoming reachable, in the component that deletes files
  from a user's library. The state space is small by construction, so exhaustive testing is
  available — declining it is a choice, not a constraint.
- *Test:* table test over the full product of axes and triggers. Assert every legal triple succeeds
  and every illegal one is refused with a named reason.
- *Source:* *lifecycle and placeholders*, §1.2, §8.

**I-LIFE-2 — Every reachable composite maps to exactly one derived status.**
- *Statement:* the mapping from (phase, acquisition, presence, production) to the status an overlay
  renders is total and single-valued. No composite maps to none, and none maps to two.
- *Prevents:* a poster badge that renders nothing, or renders two conflicting states, for a
  combination nobody enumerated.
- *Test:* table test over every reachable composite. Assert exactly one status each.
- *Source:* *lifecycle and placeholders*, §7.

**I-LIFE-3 — A release date moving backwards never destroys.**
- *Statement:* phase transitions are bidirectional. A delayed title moves back through phases and
  keeps its placeholder; no reverse transition is a destructive trigger.
- *Prevents:* a studio delaying a film causing its placeholder to be removed, which reads to the
  user as the product losing track of the thing they were waiting for.
- *Test:* generated date sequences that move forwards and backwards across every phase boundary.
  Assert zero destructive actions arise from a backwards move.
- *Source:* *lifecycle and placeholders*, §3, §13.

**I-LIFE-4 — A whole-title subject and a season subject never own the same placeholder path.**
- *Statement:* placeholder ownership divides by what is absent. The whole-title subject materializes
  only while the show is absent from the library; a season subject materializes only while the show
  is `Real` and its own season is absent. No pass produces two subjects holding one path, and a
  season subject writes nothing while its show is absent.
- *Prevents:* two lifecycle records racing to create and delete one file — the exact failure
  *lifecycle and placeholders*, §1.6, exists to prevent, reintroduced by the second granularity D-025
  admits. The damaging half is silent: the show subject removes a stub the season subject just wrote,
  the season subject rewrites it, and the pair churn the library on every pass while each reports
  success.
- *Test:* a table over the product of (show presence, season presence, `seasonGranularity`). Assert
  at most one subject holds a placeholder path per pass, and assert the path sets of the two
  granularities are disjoint. Include the ordering case: run the season subject before the show
  subject and after it, and assert the same outcome both ways.
- *Source:* *lifecycle and placeholders*, §2.1, §13. Decided as D-025.

### 20.13 The interface

I distinguish nine states (*the interface design*, §7.1) precisely so the interface can tell the
truth about what it knows. An interface that flattens them discards the design at the last mile, so
these are build-failing properties like any other.

**I-UX-1 — Every data-bearing component handles every state that applies to it.**
- *Statement:* a component has a story or a test for each of the nine states in *the interface
  design*, §7.1, that can reach it. Loading, empty, and error alone is incomplete.
- *Prevents:* the six engine-specific states — frozen, degraded, stale, pending, blocked,
  non-convergent — arriving at a component that renders them as one of the three generic ones.
- *Test:* a coverage assertion over the component inventory; a component missing an applicable
  state fails the build.
- *Source:* *the interface design*, §7.1.

**I-UX-2 — The client never infers a state.**
- *Statement:* frozen, degraded, stale, pending, blocked, and non-convergent each arrive as an
  explicit field on the response. The client does not derive them from response shape, timing, or
  an empty array.
- *Prevents:* re-introducing in the client exactly the flattening the engine was built to avoid.
- *Test:* architectural test asserting no client code branches on array length or elapsed time to
  produce one of the six states.
- *Source:* *the interface design*, §7.1.

**I-UX-3 — A failed fetch and an empty result render differently.**
- *Statement:* every list surface distinguishes "the query succeeded and matched nothing" from "the
  query failed".
- *Prevents:* the interface expression of P1 — absence of evidence shown as evidence of absence —
  which teaches the operator to distrust every empty state in the product.
- *Test:* for each list surface, render both cases; assert the output differs and that the failure
  case names the failure.
- *Source:* *the interface design*, §7.3.

**I-UX-4 — An unverified move is never shown as settled.**
- *Statement:* a reordered row displays as pending until read-back confirms it.
- *Prevents:* the board and the Plex home screen disagreeing, with the board looking authoritative.
- *Test:* reorder against a fake whose verification is delayed; assert the row shows pending, and
  that it settles only after read-back returns.
- *Source:* *the interface design*, §4.4, §7.6.

**I-UX-5 — A destructive action's preview equals what it does.**
- *Statement:* the counts and named objects in the preview match the operation's effect against the
  same fixture.
- *Prevents:* a typed confirmation collected against numbers that turn out to be wrong, which is
  consent obtained under a false statement.
- *Test:* for each destructive action, compare the preview against the executed effect on a fixture.
- *Source:* *the interface design*, §7.5.

**I-UX-6 — Every drag operation has a keyboard path.**
- *Statement:* any reorder or placement achievable by pointer is achievable through the layer list,
  the element inspector, or move-up / move-down / move-to-position controls.
- *Prevents:* the product's primary ordering surface being unusable without a mouse.
- *Test:* perform the same reorder by drag and by keyboard; assert an identical resulting
  definition.
- *Source:* *the interface design*, §9.2.

**I-UX-7 — No user-facing string is hard-coded.**
- *Statement:* every user-facing string resolves through the message catalogue, from the first
  commit.
- *Prevents:* retrofitting i18n across every component later, which is the expensive order.
- *Test:* lint failure on any user-facing literal.
- *Source:* *the interface design*, §9.1.

**I-UX-8 — Onboarding reaches a populated library within the target.**
- *Statement:* a scripted first run completes the wizard and the first sync within the ten-minute
  target, asserted on step count and blocking calls rather than on wall time.
- *Prevents:* the onboarding promise decaying release by release with nothing failing.
- *Test:* scripted run of the first-run journey against a fixture server.
- *Source:* *the interface design*, §4.1; *the product spec*, §10.

**I-UX-9 — Every live surface is correct without the stream.**
- *Statement:* with SSE blocked, every surface the stream feeds still renders correct data on load,
  and the disconnection is visible.
- *Prevents:* a lost connection presenting as frozen numbers that look live.
- *Test:* block SSE; assert each live surface loads correct data and shows the indicator.
- *Source:* *the interface design*, §8.

**I-UX-10 — The wizard's step is derived from state, not from the client.**
- *Statement:* the step the setup wizard resumes at is computed from the database by the table in
  §7.14. No request parameter, cookie, or client-held draft can move it, and a step whose evidence is
  absent is never reported as complete.
- *Prevents:* the client naming the step it would like to be on, which on the claim step means
  skipping the claim. Also prevents a resumed wizard that believes a step is done because the browser
  remembers doing it, on an instance where the write failed.
- *Test:* for each step, seed the database at that step's evidence level and assert the derived step.
  Then request every other step index directly and assert the server answers with the derived one.
  Assert a client that reports step 8 against an empty database is answered with step 1.
- *Source:* §7.14; D-046.

### 20.14 Security

D-029 assumes a publicly reachable instance, which turns several things that would be hardening on a
private network into correctness properties.

**I-SEC-1 — A forged forwarded header never buys a fresh rate-limit budget.**
- *Statement:* `X-Forwarded-For` is honoured only when the immediate peer is in the configured
  trusted-proxy list. A request carrying a forged header from an untrusted peer is limited against
  its real peer address.
- *Prevents:* the failure that makes every other rate limit decorative while continuing to report
  success. An attacker who can vary one header defeats per-IP limiting entirely, and nothing in the
  logs looks wrong — the limiter is working perfectly against an identifier the attacker chooses.
- *Test:* drive login failures past the threshold from one peer, varying `X-Forwarded-For` each
  request, with the peer outside the trusted list. Assert the lockout still fires. Repeat with the
  peer inside the list and assert the forwarded address is used instead.
- *Source:* *non-functional requirements*, §4.3. Required by D-029.

**I-SEC-2 — Every response carries the full security header set.**
- *Statement:* the headers specified in *non-functional requirements*, §4.4, are present on every
  response from every route, including errors, redirects, static assets, and the SSE stream.
- *Prevents:* headers applied per handler rather than by middleware, which are always present on the
  routes someone remembered and missing on the one they did not.
- *Test:* enumerate every route from the router; assert the header set on each, including the 404
  and 500 paths.
- *Source:* *non-functional requirements*, §4.4.

**I-SEC-3 — No path escapes its asset root.**
- *Statement:* every browsed path is canonicalised and symlink-resolved *before* the containment
  check, and a path resolving outside an enabled root is refused.
- *Prevents:* path traversal on an instance the operator exposed. Checking containment before
  resolution is the classic mistake: `roots/../../etc` passes a prefix test and resolves outside.
- *Test:* a corpus of traversal sequences, absolute paths, symlinks pointing outside a root, and
  encoded variants. Assert refusal on each, and assert the message names the root rather than the
  resolved path.
- *Source:* *non-functional requirements*, §4.6.

**I-SEC-4 — No placeholder is written outside a configured placeholder root.**
- *Statement:* the containment rule in I-SEC-3 applies to writes against `placeholderRoots`, with
  the same resolve-then-check ordering.
- *Prevents:* the component that writes files into a user's library writing one somewhere else. This
  is the highest-consequence write path in the product.
- *Test:* the I-SEC-3 corpus, applied to placeholder materialisation. Assert no file is created
  outside a root, and assert the subject settles to a reported error rather than to `Placeholder`.
- *Source:* *non-functional requirements*, §4.6.

**I-SEC-5 — A restore without the secret key loses credentials and nothing else.**
- *Statement:* restoring a backup that excludes `secrets.key` yields every definition, collection,
  placement record, and base poster intact. The `secrets` rows are present and marked undecryptable.
  No row is deleted because it could not be read.
- *Prevents:* the default backup path producing a silently lossy restore. Excluding the key is the
  default (*non-functional requirements*, §6.1), so this is the common case, not the edge case — and
  treating an unreadable secret as an absent one is failure pattern P1 applied to the operator's
  credentials.
- *Test:* back up, restore without the key, assert full row counts across every table, assert each
  integration reports needing re-authentication, and assert nothing was deleted.
- *Source:* *non-functional requirements*, §6.3.

**I-SEC-6 — A backup is verified before it is called a backup.**
- *Statement:* the nightly job verifies the archive it just wrote — integrity check on the database
  copy, digest sample against the asset files — and raises a doctor finding when verification fails.
- *Prevents:* discovering that backups have been failing at the moment one is needed, which is the
  moment D-023 makes the only recovery path from a bad upgrade.
- *Test:* corrupt a written archive and assert the verification fails and the finding is raised.
  Assert a valid archive passes and records its verification timestamp.
- *Source:* *non-functional requirements*, §6.4.

**I-SEC-7 — The volatile-parameter feed can change a declared value and nothing else.**
- *Statement:* a fetched parameter is applied only when the feed signature verifies, the name exists
  in the registry the binary ships, and the value satisfies the constraint that registry declares.
  A failure on any of the three leaves the last known-good value in force and raises a doctor
  finding.
- *Prevents:* an out-of-band repair channel becoming a remote-configuration channel, and then remote
  code by a slower route — the class of design D-001 already rejected. Also prevents a feed outage
  or a hostile feed from disabling a source that a stale value would have kept working.
- *Test:* four fixtures — a valid feed, an unsigned feed, a feed naming a parameter absent from the
  registry, and a feed whose value fails its constraint. Assert only the first is applied; assert the
  other three leave `last_good_value` in force, increment `reject_count`, and raise a finding.
  Assert no feed shape can introduce a parameter name the shipped registry does not declare.
- *Source:* *the data model*, §11.5; *the product spec*, §8; D-041.

**I-SEC-8 — An unconfigured instance grants nothing without console proof.**
- *Statement:* while `instance.setup_completed_at` is `NULL`, every route except health and the claim
  endpoint refuses. The claim endpoint mints a claim only for a caller presenting the live bootstrap
  token or, once an admin exists, that admin's credentials. No admin account, Plex connection, or
  secret is ever written on behalf of a caller holding neither.
- *Prevents:* the race that D-029 makes real — a reachable instance whose first visitor becomes its
  administrator and, one wizard step later, holds a Plex token that authorises deletion. Also
  prevents the quieter version, where a second browser hijacks a wizard the operator is halfway
  through.
- *Test:* against a fresh instance, drive every wizard endpoint with no claim and assert refusal on
  each. Assert an expired token, a wrong token, a malformed token, and an empty token are refused
  with one indistinguishable response. Assert a claim held by one cookie makes a second cookie's
  claim attempt fail with the retry time and no state change. Assert the token never appears in
  `logs/afisharr.log`, in any table, or in any response body. Assert a restart while setup is
  incomplete invalidates the previous token.
- *Source:* §19.6.1; §21.4.2; D-029, D-045.

### 20.15 Performance and resource bounds

D-030 sets the scale target at 200,000 items and 2,000 collections. These three are the bounds whose
violation is a crash rather than a disappointment, which is why they are invariants while the rest of
*non-functional requirements* §2 is a budget.

**I-PERF-1 — No pass holds the library in memory.**
- *Statement:* the working set of a full reconciliation pass is a function of batch size, not of
  library size. Doubling the item count does not increase peak memory.
- *Prevents:* the pass that works at 5,000 items and is killed by the OOM reaper at 200,000. The
  failure arrives as a container restart loop, on the largest library, belonging to the user least
  able to work around it.
- *Test:* run a full pass against fixtures at 50,000 and 200,000 items. Assert peak RSS differs by
  less than a fixed tolerance rather than scaling with the input.
- *Source:* *non-functional requirements*, §2.3. Required by D-030.

**I-PERF-2 — The render cache respects its cap.**
- *Statement:* the cache never exceeds its configured size. Eviction is by `last_used_at`, and an
  entry protected by `ON DELETE RESTRICT` is never evicted.
- *Prevents:* filling the operator's disk. At 200,000 items an uncapped cache is roughly 50 GB, and
  it grows fastest right after onboarding, when the operator is least prepared for it.
- *Test:* drive renders past the cap; assert the cap holds, assert eviction order, and assert a
  render still bound to a Plex upload survives.
- *Source:* *non-functional requirements*, §2.3, §3.2.

**I-PERF-3 — A full pass at the scale target stays inside the memory ceiling.**
- *Statement:* a full pass over 200,000 items and 2,000 collections completes without process RSS
  exceeding 1 GB.
- *Prevents:* the reference deployment needing more memory than the reference deployment has. This
  fails on the ceiling rather than on the clock, because a slow pass is a disappointment and an
  unbounded one takes the machine with it.
- *Test:* a memory-bounded run against scale fixtures, in the nightly lane. The assertion is the
  ceiling; the elapsed time is recorded for trend, not asserted.
- *Source:* *non-functional requirements*, §1, §3.1.

**I-PERF-4 — Cache expiries are spread, never clustered.**
- *Statement:* `expires_at` is set to `fetched_at + ttl - random(0, ttl / 4)`, so entries written in
  one burst do not expire in one burst. The offset is subtracted, so an entry never outlives the TTL
  the provider's headers permit.
- *Prevents:* a cold start turning into a recurring simultaneous refetch against every provider at
  once. The first run populates the cache in a burst; without the spread, every subsequent expiry
  arrives in the same burst forever. That is the traffic shape that gets API keys revoked and
  addresses blocked — the same outcome I-SRC-4 protects against, reached from the other direction.
- *Test:* populate 10,000 entries with one TTL in a single pass and assert the expiry distribution
  spans at least a quarter of the TTL with no bucket holding a disproportionate share. Assert
  separately that no `expires_at` exceeds `fetched_at + ttl`.
- *Source:* *the data model*, §11.3; *non-functional requirements*, §2.2.

### 20.16 Coverage

What has been swept for failure modes, and what has not. Stated because a list of invariants implies
a completeness it does not have.

| Area | Swept | Notes |
| --- | --- | --- |
| Lifecycle, placeholders, cleanup | Yes | Deep |
| Placement and ordering | Yes | Deep; the precision failure is documented in the source material itself |
| Base posters and overlay application | Yes | This pass. Produced I-ID-2, I-REV-2, I-REV-5, I-RENDER-2 |
| Acquisition and `*arr` integration | Yes | This pass. Produced I-ACQ-1 through I-ACQ-3 |
| Collection reconciliation | Partial | Produced I-SRC-6; the merge and filter paths are not deeply swept |
| Poster generation and templates | No | Roughly 2,900 lines unexamined |
| The overlay renderer itself | No | Roughly 1,600 lines unexamined; font and asset fallback behaviour unknown |
| Source adapters individually | Partial | One adapter family swept on 2026-08-09 (internal audit notes, §5.6), which produced I-SRC-8, I-DATA-12, and I-DATA-13. The remaining adapters are unswept, and the shared rails are still only as good as each adapter's own honesty |
| External-response caching | Yes | 2026-08-09. Produced I-DATA-12 and I-PERF-4 |
| Scheduling and job orchestration | No | |
| Auth, sessions, permissions | Partial | First run swept on 2026-08-09, against the onboarding design of a sibling project. Produced I-SEC-8 and I-UX-10, D-045 and D-046, and §19.6.1. Session lifetime, API-key scoping, and the Plex PIN flow are still unswept |
| The frontend | No | Out of scope for this document |

The unswept rows are not a gap in the invariants that exist; they are unknown unknowns. Two are worth
scheduling before implementation of the corresponding subsystem: **the overlay renderer**, because
font and asset fallback behaviour is exactly where "renders nothing and nobody knows why" lives, and
**the source adapters**, because the shared safety rails only work if each adapter classifies its own
responses honestly.

### 20.17 Obligation traceability

Where each obligation from the four absorbed lists landed. A row with no target is a defect in this
section. This table is the conservation record for the absorption; it is not a second statement of
any obligation.

**From the lifecycle test obligations**

| Obligation | Landed as |
| --- | --- |
| Exhaustive transition tests | I-LIFE-1 |
| Status mapping table test | I-LIFE-2 |
| Destructive-action property test | I-EVID-2 |
| Crash-injection test | I-DATA-1 |
| Reverse-transition test | I-LIFE-3 |
| Staleness test | I-EVID-7 |
| Reference-counting test | I-EVID-6 |
| Reproducibility test | I-ACQ-4 |

**From the placement test obligations**

| Obligation | Landed as |
| --- | --- |
| Minimality property test | I-CONV-1 |
| Convergence property test with adversarial input | I-CONV-6 |
| Simulated precision exhaustion | I-CONV-2, I-CONV-3 |
| Anchor immovability | I-CONV-4 |
| Sort-title round trip | I-REV-3 |
| Idempotency | I-IDEM-1 |
| Randomization stability | I-IDEM-2 |
| Foreign-object safety | I-CONV-5 |
| Drag-and-drop round trip | I-CONV-8 |

**From the schema-level test obligations**

| Obligation | Landed as |
| --- | --- |
| Round-trip projection | I-DATA-4 |
| Per-user visibility needs no DDL | I-DATA-5 |
| Destructive transitions unrecordable outside the allowlist | I-EVID-2, test (b) |
| One subject per identity | I-DATA-10 |
| Sort-title round trip including lock state and absence | I-REV-3 |
| Migration forward from every released schema | I-DATA-11 |
| Downgrade refusal | I-DATA-7 |
| Crash-consistency of intents | I-DATA-1 |
| Retention keeps destructive records | I-DATA-6 |
| Asset GC preserves protected assets | I-DATA-9 |

**From the interface acceptance criteria**

| Obligation | Landed as |
| --- | --- |
| State coverage | I-UX-1 |
| No inferred state | I-UX-2 |
| Empty is never failure | I-UX-3 |
| Preview equals output | I-RENDER-3 |
| Optimistic ordering is not shown as settled | I-UX-4 |
| Destructive actions preview accurately | I-UX-5 |
| Keyboard parity | I-UX-6 |
| No hard-coded strings | I-UX-7 |
| Ten-minute onboarding | I-UX-8 |
| Offline stream degradation | I-UX-9 |
## 21. Non-functional requirements

Scale, performance, security, platforms, backup, upgrade, privacy, licensing, and the test strategy.

This section turns four decisions into obligations a build can fail on: the scale target (D-030),
the security posture (D-029), the licence (D-028), and forward-only migration (D-023). Where it
states a number, that number is a **budget** — a target the implementation must be measured against,
not a measurement. Nothing here has been measured, because nothing has been built yet. Every budget
carries the hardware it is written against, so a later measurement can disagree with it usefully.

Nine invariants are introduced by this section: `I-SEC-1` through `I-SEC-6` and `I-PERF-1` through
`I-PERF-3`.

### 21.1 The reference deployment and scale target

A budget without hardware is an adjective. Every figure in §21.2 and §21.3 is written against this
machine, and a claim that a budget is met must name the machine it was met on.

| Property | Reference value |
| --- | --- |
| CPU | 4 cores, x86-64, circa 2020 |
| Memory | 8 GB total, of which Afisharr may use 1 GB steady-state |
| Storage | SATA SSD. Not NVMe, and explicitly not spinning rust for the database |
| Network | Gigabit LAN to the Plex server; ordinary consumer WAN to external providers |
| Deployment | Docker, single container, behind a reverse proxy that terminates TLS |

**Spinning disk is a supported deployment for the asset store and an unsupported one for the
database.** WAL with `synchronous = NORMAL` on a 5,400 rpm disk turns every pass into a queue of
fsyncs. The asset store is streamed and sequential, so it is fine there. Startup checks the database
device and warns; it does not refuse, because an operator who accepts the consequence is entitled to.

**Scale target.** From D-030. This is the point at which the product must still be pleasant, not the
point at which it falls over.

| Dimension | Target | Notes |
| --- | --- | --- |
| Library items | 200,000 | Across all managed libraries combined |
| Plex libraries | 8 | Movie and show libraries; music and photo are non-goals |
| Managed collections | 2,000 | Includes person auto-collections, which are the likely bulk |
| Placement participants | 2,500 | Managed plus adopted collections plus native hubs (D-008) |
| Lifecycle subjects | 20,000 | Titles inside `countdownWindow`, plus opted-in season subjects (D-025) |
| Definitions | 3,000 | Collections, templates, packs |
| Concurrent operators | 4 | Admin-only surface (D-007); this is one household, not a tenancy |

**Two of these are load-bearing in a way the others are not.** 200,000 items sets the memory ceiling
and forbids any pass that materialises the library. 2,500 placement participants is the number that
makes ordering hard, and it is discussed honestly in §21.2.4.

### 21.2 Performance budgets

Each budget names the hot path it constrains, so that the budget and the index that serves it refer
to the same operation (see *Data model*).

#### 21.2.1 Interactive paths — the operator is waiting

| Operation | Budget (p99) | Hot path |
| --- | --- | --- |
| Any GUI list page, first paint | 300 ms | `ix_definitions__kind`, `ix_doctor_findings__open` |
| Collection editor open | 200 ms | Definition body by id |
| Collection preview — "what would this contain now?" | 3 s | Cached source contributions; a cold fetch is exempt and shows progress |
| Single poster render, 1000×1500, 3 overlay elements | 150 ms | Render pipeline, no I/O in the hot loop |
| Template editor live preview | 300 ms | One render plus paint |
| Resolve an external id to a library item | 1 ms | `ix_library_item_ids__lookup` — the highest-volume lookup in the product |
| Home screen board load, 2,500 participants | 1 s | `ix_placement_desired__plan` |
| Login | 500 ms | Argon2id verification dominates and is deliberate |

**300 ms is the threshold at which a page stops feeling instant**, which is why it is the list-page
budget rather than a rounder number. The board is given 1 s because it is a genuinely large page and
is entered deliberately.

#### 21.2.2 Background paths — nothing is waiting, but nothing may stall

| Operation | Budget | Notes |
| --- | --- | --- |
| Cold start to serving HTTP | 5 s | Excluding migrations |
| Startup integrity checks after a migration | 60 s | `foreign_key_check` plus `integrity_check` at 200,000 items |
| First full library cache build | 30 min | Bounded by Plex's paging, not by us. Must show item-level progress |
| Incremental library sync | 2 min | The steady-state case, and the one that runs on a schedule |
| One collection evaluated, 10,000-item candidate pool | 5 s | Source resolve, filter, order |
| Full pass over 2,000 collections | 30 min | Parallel across cores; §21.2.3 |
| Placement planning for 2,500 participants | 60 s | Planning only. Applying moves is bounded by Plex and reported separately |
| Lifecycle pass over 20,000 subjects | 5 min | Evidence refresh dominates and is rate-limited by providers |
| Nightly maintenance — retention, vacuum, asset GC | 15 min | Must not block writes for more than 1 s at a time |

#### 21.2.3 What the scale target forbids

Four rules follow from 200,000 items, and each is testable rather than aspirational.

1. **No pass may hold the library in memory.** Reconciliation streams in bounded batches. The
   working set is a function of batch size, never of library size. `I-PERF-1`.
2. **Writes batch into transactions sized by work, not by item.** Two hundred thousand single-row
   transactions is an fsync per item. D-030 makes this binding on D-024's single write actor.
3. **The render cache is bounded.** Default cap 10 GB, LRU eviction by `last_used_at`, configurable.
   An unbounded cache at this scale is a disk-exhaustion bug with a delay fuse. `I-PERF-2`.
4. **Full-library passes parallelise across cores.** Single-threaded reconciliation of 200,000 items
   does not fit the 30-minute budget on 4 cores. Parallelism is over read and transform; the write
   actor stays single (D-024).

#### 21.2.4 Where these budgets are least trustworthy

Stated plainly, because a budget nobody doubts is a budget nobody validates.

**Placement is the one to watch.** 2,500 participants in an ordered sequence, with `gapBudget`
defaulting to 8, is a combination nobody has measured. The gap scheme was designed for the precision
problem, not for a sequence of this length, and the two interact: longer sequences exhaust precision
faster, and rebalancing a 2,500-item sequence is itself expensive. The 60-second planning budget is
a guess, and it is the guess most likely to be wrong.

This raises the stakes on the two spikes rather than lowering them. **Q-014 must calibrate against a
sequence of at least 2,500**, not against a short one, or it will measure the wrong thing. **Q-015
decides whether this is one sequence of 2,500 or eight sequences averaging 300**, which changes the
answer by an order of magnitude. Both are recorded in *Decisions of record*, and both must be sequenced before
placement is implemented.

**The retention cap collides with the scale target.** The schema (see *Data model*) caps
non-destructive `lifecycle_transitions` at 200,000 rows. Initial population of 20,000 subjects, each
passing through several phases, approaches that cap within the first few passes — so the window that
was meant to hold 90 days of history could hold days. The cap was sized before D-030 existed. This
is Q-005, and D-030 promotes it from "revisit once real volumes exist" to "revisit before the
lifecycle component ships".

#### 21.2.5 One instrumented outbound client

Every outbound HTTP request — source adapters, the Plex client, the `*arr` clients, artwork
downloads, the volatile-parameter feed, and dataset imports — goes through one client in
`backend/crates/sources`. Two properties follow, and neither is achievable per adapter.

**Every request is timed, for free.** The budgets in §21.2.1 and §21.2.2 are dominated by external
calls, so per-request duration, per-host totals, and retry counts are what make a missed budget
diagnosable rather than merely visible. Instrumenting one client yields that for every adapter,
including adapters written after the instrumentation. Instrumenting per adapter yields it for the
adapters somebody remembered.

**Every request has a hard timeout, structurally.** A default per-request deadline is set on the
client, not passed at each call site. Retry-with-backoff fires only on a raised error, and a stalled
connection never raises — so a socket that hangs stalls the pass indefinitely while the retry policy
waits for an exception that never arrives. A call site may shorten the deadline; it cannot omit one.

The same client owns the response cache (see *Data model*). That is what lets the parser version
participate in the key and the expiry spread apply uniformly, rather than every adapter reimplementing
both and one of them getting it wrong.

### 21.3 Resource footprint

#### 21.3.1 Memory

| Component | Steady state | Peak |
| --- | --- | --- |
| Process RSS | 400 MB | 1 GB |
| SQLite page cache | 32 MB per connection (see *Data model*) | — |
| Render pipeline | 50 MB per concurrent render | 4 concurrent renders |

`I-PERF-3` asserts the ceiling: a full pass at the scale target completes without RSS exceeding
1 GB. The test is a memory-bounded run, not a benchmark — it fails on the ceiling, not on the clock,
because a slow pass is a disappointment and an unbounded one is a crash.

#### 21.3.2 Disk — and the finding that changes the backup design

| Store | Budget at 200,000 items | Regenerable? |
| --- | --- | --- |
| `afisharr.db` | 5 GB | **No** |
| Base posters — the pristine originals | ~50 GB | **No.** See below |
| Render cache | 10 GB, capped | Yes, from templates and base posters |
| HTTP cache | 1 GB, expiry-driven | Yes |
| Placeholder video stubs | 1 GB | Yes, one static file, hard-linked |
| Application logs | 500 MB, rotated | Yes |

**The schema chapter (see *Data model*) sized the asset store against a 5,000-item library and
concluded "several gigabytes". D-030 multiplies that by forty.** At roughly 250 KB per 1000×1500
WebP, 200,000 base posters is about 50 GB. The rendered posters would be another 50 GB if they were
kept, which is precisely why the render cache is capped in §21.2.3 rather than allowed to grow with
the library.

**Base posters are the only irreplaceable bytes in the product, and the reason is worth stating.**
Once an overlay is applied, Plex holds the overlaid poster. The pristine original exists in exactly
one place: Afisharr's asset store. Losing it means byte-exact restoration is impossible forever, and
byte-exact restoration is what `I-REV-2`, `I-REV-4`, and `I-REV-5` promise. A backup that captures
the database and skips the base posters therefore restores a system that can no longer undo itself —
and it does so silently, because every digest still resolves to a row.

That single fact drives §21.6.

### 21.4 Security model

D-029 assumes a publicly reachable instance. Not "LAN, hardened just in case" — the documentation
says the instance may face the internet, and the defaults are set for that case.

#### 21.4.1 What is being protected, and from whom

| Asset | Threat | Consequence if lost |
| --- | --- | --- |
| Plex token | Credential theft | Full control of the operator's Plex server, including deletion |
| `*arr` and Overseerr API keys | Credential theft | Control of the download stack |
| TMDB, Trakt credentials | Credential theft | Quota theft, account suspension |
| Session cookies | Session hijack | Admin access to everything above |
| Library contents | Disclosure | Reveals what the household watches |
| Filesystem | Path traversal | Arbitrary read within the process's reach |

**The Plex token is the crown jewel and should be reasoned about that way.** It authorises
deletion. Every control below exists primarily to keep it.

#### 21.4.2 Authentication and sessions

| Control | Requirement |
| --- | --- |
| Password hashing | Argon2id, PHC string (see *Data model*). Parameters tuned to ~250 ms on the reference machine |
| Session storage | SHA-256 of the cookie value, never the value (already schema-enforced) |
| Cookie flags | `Secure`, `HttpOnly`, `SameSite=Lax` |
| Idle timeout | 7 days, sliding on `last_seen_at` |
| Absolute timeout | 30 days, no extension |
| Rotation | New session id on privilege change and on password change |
| Revocation | All sessions for a user revoked on password change; individual revocation from Settings |
| CSRF | Always on, no toggle (see *Decisions of record*) |
| API keys | Hashed at rest, scoped, individually revocable, last-used timestamp shown |
| First run | No default credentials. Nothing is reachable until the admin account exists (see *The interface*) |
| First-run proof | A 62-bit bootstrap token, printed to the console at startup while setup is incomplete, held in memory for 15 minutes, compared in constant time, validated rather than consumed (§19.6.1) |
| Setup claim | The wizard is leased to one browser for 10 minutes, sliding on each gated request. Stored as the `setup:claim` lease with the SHA-256 of the cookie as `owner`, never the cookie itself |
| Claim cookie | `afisharr_setup_claim`; `HttpOnly`, `Secure` over HTTPS, `SameSite=Lax`, `Path=/api/setup`, `Max-Age=600` |
| Setup recovery | Once the admin account exists, admin credentials mint a claim without the token, so an interrupted setup survives the restart that destroys the token |

#### 21.4.3 Rate limiting, and the trap that makes it decorative

Rate limits apply per client IP and per account to authentication, to the API, and to any endpoint
that reaches a third-party service on the caller's behalf.

| Endpoint class | Limit | On exceed |
| --- | --- | --- |
| Login, per account | 5 failures in 15 min | Lock 15 min, exponential to 24 h |
| Login, per IP | 20 failures in 15 min | Throttle, then block |
| Setup claim and setup recovery, per IP | 5 attempts in 15 min | 429 with `Retry-After` |
| API, authenticated | 600 requests/min | 429 with `Retry-After` |
| Endpoints that call a provider | 60 requests/min | 429; protects the operator's provider quota, not us |

**The setup-claim limiter is consulted after the already-claimed check, not before.** An instance
already claimed by another browser answers 409 without touching the limiter, so an operator
refreshing the page does not spend the attempts they will need once the claim expires. The limiter
guards the token comparison, which is the only step where guessing gains anything.

**The trap named in D-029 is discharged here.** `trustProxy` decides whether
`X-Forwarded-For` is honoured. If it is on and the instance is reachable directly rather than only
through the proxy, an attacker sets that header per request and every per-IP limit above becomes
decorative — while continuing to report that it is working.

The setting is therefore **not a boolean**. It is a list of trusted proxy addresses or CIDR ranges,
and a forwarded header is honoured only when the immediate peer is in that list. An empty list means
forwarded headers are ignored and the peer address is used. `I-SEC-1` tests it, and it tests the
attack rather than the setting: a request carrying a forged `X-Forwarded-For` from an untrusted peer
must be rate-limited against its real address.

#### 21.4.4 Transport and response headers

TLS is terminated by the reverse proxy; Afisharr does not implement TLS itself, and says so, because
a product that half-implements TLS invites being exposed with it half-configured. When Afisharr sees
a plaintext request whose forwarded protocol claims HTTPS, it trusts the proxy list from §21.4.3 and
nothing else.

| Header | Value |
| --- | --- |
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains`, emitted only over HTTPS |
| `Content-Security-Policy` | `default-src 'self'`, no inline script, `frame-ancestors 'none'` |
| `X-Content-Type-Options` | `nosniff` |
| `Referrer-Policy` | `no-referrer` |
| `Permissions-Policy` | Deny camera, microphone, geolocation |

The SPA is served from the binary with no external origins (see *Product overview*), so a strict CSP costs
nothing. `I-SEC-2` asserts every header on every response, because a header applied by a handler
rather than by middleware is a header that is missing on the one route nobody remembered.

#### 21.4.5 Secret key management

The secrets schema (see *Data model*) isolates secrets into their own encrypted table and explicitly
defers key management here. **D-032** settles it.

- **Cipher:** XChaCha20-Poly1305, per-secret random nonce, recorded in the `nonce` and `algorithm`
  columns the schema already carries.
- **Key:** 32 bytes from the OS CSPRNG, generated on first start, stored at `secrets.key` beside the
  database with mode `0600`. Overridable by `AFISHARR_SECRET_KEY` for operators who mount it from a
  secret manager.
- **Rejected:** an operator passphrase at startup, because it breaks unattended container restart,
  which is the primary deployment. Rejected: the environment variable as the *default*, because env
  vars leak into process listings, crash dumps, and container inspection. It remains available as an
  override for operators who have somewhere better to put it.

**What this protects against, stated honestly:** a stolen database file. It does not protect against
an attacker who can read the filesystem, because such an attacker reads the key too. This is the
standard limit of encryption at rest for an unattended service, and the documentation says it in
these words rather than implying more.

**The consequence that matters is for backup.** A backup containing `secrets.key` is
credential-bearing and must be stored accordingly. A backup without it restores everything except
the ability to decrypt credentials. §21.6.3 makes that a deliberate choice rather than an accident.

#### 21.4.6 The filesystem boundary

The asset-root definitions (see *Data model*) govern what the browser walks. On a reachable instance
the browser that walks them is a path-traversal boundary and gets the tests a boundary gets.

Every path is canonicalised and then checked for containment within an enabled root **after**
resolution, not before. Symbolic links are resolved before the check. Traversal sequences, absolute
paths, and links pointing outside a root are all refused with the same message, which names the root
rather than the resolved path. `I-SEC-3`.

Placeholder writes obey the same rule against `placeholderRoots`. This is the component that writes
files into a user's library, so the boundary matters more there than anywhere else. `I-SEC-4`.

#### 21.4.7 Disclosure

`SECURITY.md` at the repository root names a contact, a 90-day coordinated-disclosure window, and
the supported versions. A vulnerability report is triaged within 5 working days. The file is launch
work under D-028, not post-launch work.

#### 21.4.8 What is deliberately not built

- **Multi-tenancy.** The surface is admin-only (D-007). Per-principal visibility is *stored* from day
  one, but the authorisation model is one trust level.
- **Audit logging of operator actions as a security control.** The audit record exists for
  explaining what the engine did (see *Lifecycle*), not for forensics against the operator.
- **TLS termination.** §21.4.4.

### 21.5 Platform matrix

Docker is the supported deployment, recorded as D-037. Native binaries are published and supported
on a best-effort basis, because the filesystem and permission stories differ per platform and the
placeholder writer touches both.

| Target | Support | Notes |
| --- | --- | --- |
| `linux/amd64` Docker | **Primary** | The tested target. Every budget in §21.2 is measured here |
| `linux/arm64` Docker | **Supported** | Raspberry Pi 5 class and Apple Silicon. Render budgets scale with the core count |
| `linux/amd64` native | Best effort | Published binary, no per-release manual testing |
| `linux/arm64` native | Best effort | As above |
| `darwin/arm64` native | Best effort | Development target, not a deployment target |
| `windows/amd64` native | Best effort | Path handling differs enough that the placeholder writer needs its own tests |
| `linux/armv7` | **Unsupported** | 32-bit address space against a 1 GB memory ceiling and a 50 GB asset store |

**Minimum Plex version** is stated per release and tested against, because rating-key behaviour and
hub semantics have changed across versions and the placement subsystem depends on both.

**SQLite** is bundled, never the system copy. The schema depends on `STRICT` tables and expression
indexes, and a system SQLite that predates them fails at a confusing moment rather than at startup.

### 21.6 Backup and restore

The recovery path from a bad upgrade is *restore* (D-023). That makes backup a correctness feature,
and it is specified with the same seriousness as the write path.

#### 21.6.1 What must be captured

Recorded as D-033. The backup unit is not "the database". It is three things with different
properties, and conflating them is what produces a backup that restores into a system that cannot
undo itself (§21.3.2).

| Component | In backup | Why |
| --- | --- | --- |
| `afisharr.db` | **Always** | Every record. Not regenerable |
| Base-poster assets | **Always** | The only copy of every pristine original. Not regenerable. ~50 GB at scale |
| `secrets.key` | **Opt-in, default off** | Restores credential access. Makes the backup credential-bearing (§21.4.5) |
| Render cache | Never | Regenerable from templates and base posters |
| HTTP cache | Never | Regenerable, and stale by the time it is restored |
| Placeholder stubs | Never | Regenerable; one static file |
| Logs | Never | Not state |

Excluding the render cache is what makes the backup tractable: it removes roughly half the bytes and
all of the churn.

#### 21.6.2 How the database is captured

**SQLite's online backup API, never a file copy.** A file copy of a WAL database mid-write is not a
valid database, and the failure is silent — the copy exists, has the right size, and opens. This is
already binding on the pre-migration backup (see *Data model*) and it is binding here.

Base-poster assets are content-addressed and immutable, so an asset backup is an incremental sync of
new digests. A digest never changes meaning, which is what makes the incremental safe.

#### 21.6.3 Restore, including the case that will actually happen

Restore is offered as a first-class operation, not a documented file-shuffling procedure.

1. Refuse to restore into a running instance. Stop first.
2. Verify the archive before touching anything: database integrity, schema version, and a sample of
   asset digests against their files.
3. **Refuse a backup whose schema version is newer than the binary**, for the same reason startup
   refuses one (see *Data model*).
4. Restore the database, then the assets.
5. Reconcile: every asset row is checked against the filesystem, and rows with no file are marked
   `missing_since` rather than deleted.
6. Report what could not be restored, by name and by count.

**The case that will actually happen is a restore without `secrets.key`,** because the default is to
exclude it. That path must be graceful and it must be tested: every definition, collection,
placement record, and base poster restores; the `secrets` rows are present but undecryptable; the
UI marks each integration as needing re-authentication and walks the operator through it; nothing is
deleted on the assumption that an unreadable secret is an absent one. `I-SEC-5`.

**Step 5 exists because the alternative is worse.** An asset row whose file is missing is
recoverable — a base poster can be recaptured from Plex if Plex still holds the original. A deleted
row is not recoverable at all. Absence of the file is not evidence that the asset is unwanted, which
is failure pattern P1 (see *Invariants*).

#### 21.6.4 Schedule and verification

Nightly by default, retaining seven daily and four weekly. **A backup that has never been restored
is a hypothesis.** The nightly job therefore verifies the archive it just wrote — integrity check on
the database copy, digest sample on the assets — and surfaces a doctor-page finding when
verification fails. `I-DATA-7` and `I-DATA-8` already cover the pre-migration backup; `I-SEC-6`
covers the scheduled one.

### 21.7 Upgrade policy

Forward-only, with a pre-migration backup and downgrade refusal (D-023). The migration mechanics are
covered in *Data model*; this section states the policy around them.

| Rule | Detail |
| --- | --- |
| Versioning | Semantic. A major bump means a migration that cannot be reversed even in principle |
| Downgrade | **Refused at startup**, naming the version found and the newest the binary knows |
| Pre-migration backup | Automatic, non-skippable, last three retained |
| Migration failure | The instance does not start. The message names the failed migration and the backup path |
| Skipping versions | Supported. Migrations are ordered and applied in sequence |
| Deprecation | A field or setting is warned about for one minor release before removal |
| Release cadence | No commitment at 1.0. Security fixes are not held for a release train |

**The recovery path from a bad upgrade is restore, so it must be one command and it must be
documented before the first release**, not after the first person needs it.

### 21.8 Privacy posture

Afisharr collects nothing, recorded as D-038. There is no telemetry, no analytics, no crash
reporting, and no update ping. This is a decision, not an omission, and it is worth the cost it
carries: we will have no data about how the product is used and no aggregate crash signal.

| Data | Where it goes |
| --- | --- |
| Library contents, watch state, operator credentials | The local database only. Never leaves the instance |
| Requests to TMDB, Trakt, IMDb, and the rest | Directly from the instance to the provider. Afisharr proxies nothing through any server we run |
| Update checks | None. Version comparison is the operator's business |
| Crash reports | Written locally. Never transmitted |

**Support bundles are the one place data could leak, so they are specified rather than improvised.**
A bundle contains logs, settings, and schema version. It **excludes the `secrets` table entirely**
(see *Data model*), redacts tokens from log lines by pattern, and lists exactly what it contains
before it is written. The operator sees the manifest before the file exists.

**What third parties learn regardless:** every provider sees the instance's IP address and what it
asked for. TMDB learns which titles are looked up. This is inherent in querying them and is stated
plainly rather than being left for someone to discover.

### 21.9 Licence compliance

The licence is AGPL-3.0-or-later (D-028). Three obligations become build and launch work.

1. **Dependency licences are checked by machine.** `cargo deny check` with an explicit allow-list,
   wired into `prek.toml` and CI. A Rust dependency tree runs to hundreds of crates and a transitive
   addition introduces a licence silently. Allowed: MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0,
   Unicode-3.0. Refused by default: everything else, including BSL, SSPL, Elastic, and CC-BY-NC.
2. **The source link is a runtime obligation, not paperwork.** *The interface* specifies it: it resolves to the
   running version rather than a branch, and a fork can retarget it.
3. **Attribution is generated, not maintained by hand.** A third-party licence file is produced at
   build time from the dependency tree and shipped in the image and the About panel. The BSD-3-Clause
   protocol reference retains its notice and is credited in the documentation.

Contributions are certified by DCO (D-031). `CONTRIBUTING.md` carries the DCO 1.1 text verbatim.

### 21.10 Test strategy

#### 21.10.1 Which invariants gate a merge, and which gate a release

Recorded as D-035. Every invariant is build-failing; the question is only which build. The split is
by runtime and by determinism, decided deliberately rather than by whichever tests happen to be slow
when someone gets impatient.

| Lane | Runs | Contains | Budget |
| --- | --- | --- | --- |
| **Merge** | Every pull request | Every table-driven and unit-level invariant. All of `I-LIFE-*`, `I-DEF-*`, `I-SEC-1` to `I-SEC-4`, and every invariant whose test is a pure function over enumerated inputs | 10 min |
| **Nightly** | Every night on `main` | Crash injection across every intent kind, all property tests, the teardown integration test (`I-REV-4`), `I-PERF-1` and `I-PERF-3` at scale | 2 h |
| **Release** | Every tagged release | Everything above, plus `I-PERF-2`, the restore path (`I-SEC-5`, `I-SEC-6`), and the full matrix against every supported Plex version | 6 h |

**Two rules keep the split from decaying.** A nightly failure blocks the next merge to `main` until
it is fixed or explicitly waived with a named reason — otherwise the nightly lane becomes a wall of
red nobody reads. And an invariant may move from merge to nightly only with a recorded measurement
showing it exceeded the merge budget, never because it is *felt* to be slow.

**Why teardown is nightly rather than per-merge**, despite D-022 making it the acceptance test for
reversibility: it needs a fully populated fixture library and a full apply-then-reverse cycle. It is
the single most important test in the product and it does not fit in ten minutes. Nightly with a
merge-blocking failure is the honest compromise.

#### 21.10.2 The adversarial Plex fake

Recorded as D-036. Several invariants are untestable against a stub, because a stub does what it is
told and the failures worth testing are ones where Plex does not. The fake is a real piece of work
and is scheduled as one, not folded into whichever task first needs it.

**The fidelity contract.** The fake must be able to produce, on demand and deterministically:

| Behaviour | Invariants that need it |
| --- | --- |
| A move that reports success and does not happen, past a precision budget | `I-CONV-*` |
| Artwork URLs in unrecognised formats | `I-ID-2`, `I-RENDER-2` |
| Rating-key churn — the same item under a new key | `I-ID-1`, `I-ID-3`, `I-SRC-6` |
| Partial scan states — an item indexed but not yet complete | `I-EVID-*` |
| Sort titles with independent value, presence, and lock state | `I-REV-3` |
| Timeouts and 5xx mid-pass, at a chosen operation | `I-EVID-1`, `I-ACQ-1` to `I-ACQ-3` |
| A changed server machine identifier | `I-ID-5` |

**Determinism is the requirement that makes it useful.** Every misbehaviour is triggered by an
explicit scenario, seeded, and reproducible from the seed alone. A fake that misbehaves randomly
produces flaky tests, which get muted, which is worse than not having it.

**It is not a Plex emulator.** It implements exactly the surface Afisharr calls, and it is allowed to
be wrong about everything else. A contract test against a real server, run at release, is what keeps
the fake honest — the fake makes failures reproducible, and the contract test makes the fake
truthful.

### 21.11 Obligations inherited from the design work

Every item raised in the design work is discharged below. This table is the checklist that work
asks for.

| Obligation | Discharged in |
| --- | --- |
| Backup covers database and assets as a unit | §21.6.1, and §21.3.2 explains why base posters are the irreplaceable half |
| Backup uses SQLite's online backup API, not a file copy | §21.6.2 |
| Upgrade is forward-only with a non-skippable pre-migration backup | §21.7 |
| The test-suite split | §21.10.1, as D-035 |
| The adversarial Plex fake | §21.10.2, as D-036 |
| Secret key management | §21.4.5, as D-032 |
| The filesystem browser's root jail | §21.4.6, `I-SEC-3` and `I-SEC-4` |
| Performance budgets expressed against the hot paths identified in *Data model* | §21.2.1 and §21.2.2 of this document |
| Licensing accounts for the BSD-3-Clause protocol reference | §21.9 |

**Two things this document found rather than inherited**, both recorded in §21.2.4 and §21.3.2: the
retention cap (see *Data model*) is smaller than D-030's volumes require, and the asset-store sizing
in §21.2.1 predates D-030 by a factor of forty. Neither is a contradiction to resolve by choosing —
both are amendments the schema owes, raised here because this is the section that noticed.

### 21.12 What is still open

| Item | Why it stays open |
| --- | --- |
| Retention windows (Q-005) | Promoted by §21.2.4 from "revisit later" to "revisit before the lifecycle component ships" |
| The real precision budget (Q-014) | Empirical. §21.2.4 adds a requirement: calibrate against a sequence of at least 2,500 |
| One home-screen sequence or several (Q-015) | Empirical, and it changes the placement budget in §21.2.2 of this document by an order of magnitude |

Nothing in this section is blocked on them. The milestone rebuild is: it cannot sequence placement
until Q-014 and Q-015 report.
## 22. Decisions of record

### 22.1 Decision codes

Every capability-scope row in this document set carries one of four tier codes, and every
architectural and design decision below is dated so a later reader can tell an argued decision from
an accumulated one. The codes:

| Code | Meaning |
| --- | --- |
| **T0** | In the first shippable release. |
| **T1** | Committed, after first release. |
| **T2** | Content/pack tier or long-tail; committed only as a pack or a later add. |
| **CUT** | Not building. Recorded with a reason so it stays cut. |

Identifiers are stable. A `D-nnn`, `Q-nnn`, or `CR-n` survives any renumbering of the surrounding
material, which is what lets one part of this document cite a decision made in another without
naming a section number that might later move. All twenty original scope questions against the
capability audit closed on 2026-08-08.

### 22.2 Architectural decisions D-001 to D-010

These are structural: each one closes off a family of designs rather than choosing a single feature.
No individual date is recorded for this group in the source ledger — they were absorbed as
pre-existing structural commitments rather than argued and closed on a specific day. Cite them by
identifier.

**D-001 — No browser automation for external sources.** Structured endpoints first, TLS
impersonation second. Preserves the single-binary story.

**D-002 — No third-party configuration import.** Foreign schemas are moving targets, and an importer
rots silently into producing bad imports, which is worse than having none. First-party packs and the
setup wizard serve onboarding instead. If demand appears after 1.0, an importer may exist only as a
clearly labelled, best-effort community tool — never a core compatibility promise.

**D-003 — No music libraries, no preroll management, no watchlist sync, no in-app content browsing.**
Each is a different product or a different problem domain. The collection editor's preview is not
browsing and stays in scope.

**D-004 — No migration path from any predecessor tool.** Afisharr starts clean.

**D-005 — Pure-Rust renderer.** tiny-skia, resvg, cosmic-text/parley, behind a `Renderer` trait so an
alternative backend remains possible if a fidelity wall is hit. Default packs are authored to the
committed stack's strengths.

**D-006 — Playlists: engine support from day one, UI at Tier 1.5.** The pipeline produces ordered,
per-user-owned item lists regardless; only the surface waits.

**D-007 — Admin-only permission surface at launch, principal-set visibility storage from day one.**
The storage half is not an intention — it is made testable and enforced by an invariant test.

**D-008 — Afisharr owns the full home screen.** Its own collections, adopted collections, and native
hubs, in one ordering space. This is what promoted placement to the highest-risk subsystem.

**D-009 — Multi-library definition targeting is structural.** There is no collection-linking feature,
because the problem linking would solve does not arise.

**D-010 — The field registry has a static core and a server-discovered layer.** Plex-native filter
compilation is decided by lookup against the discovered layer, not by a maintained allowlist.

### 22.3 Design decisions D-011 onward

Absorbed from the open-questions sections of the design work, all closed on 2026-08-08 unless
otherwise dated. The reasoning is retained deliberately: a decision without its argument is one
somebody reopens by accident.

**D-011 — Retire policy defaults to `keep`, paired with a visible list.** *2026-08-08.* When a
`JustReleased` window expires and no real media arrived, the placeholder stays. A title silently
vanishing from a collection is the worse failure — the user who was waiting for it gets no
explanation. Bare `keep` only trades visible accumulation for invisible accumulation, so it is paired
with a **stale-placeholder view** listing every placeholder past its retirement window, clearable in
bulk, plus a per-definition `retirePolicy: remove` for operators who prefer automatic cleanup.
Surface owned by §7.7. Raised in *Lifecycle*.

**D-012 — `Grabbing` is out at launch, as an opt-in toggle later.** *2026-08-08.* It is a badge, not
a correctness feature, and the lifecycle design already specifies under-reporting as honest. Queue
polling on every pass is a real cost for a more precise label. Raised in *Lifecycle*.

**D-013 — Ambiguous-match resolution appears on both surfaces, with the doctor page authoritative.**
*2026-08-08.* The doctor page holds the resolvable list; the collection editor shows a badge that
deep-links to it. The resolution is stored once and applies everywhere, so this is two doors onto one
room rather than two copies of the state. Raised in *Lifecycle*.

**D-014 — Adopted-collection consent is per library.** *2026-08-08.* One control per library, with a
per-collection override for exceptions, and no global control at launch. All three scopes remain
storable, so widening later needs no migration. Enforced by I-REV-6. Raised in *Placement and
ordering* and *Data model*.

**D-015 — A rebalance runs inline, never deferred to a quiet-hours window.** *2026-08-08.* A
rebalance is scheduled precisely because it is needed now. Deferring it leaves the ordering wrong
until the window opens, and the pressure that triggered it keeps building meanwhile. Raised in
*Placement and ordering*.

**D-016 — The static registry core ships as code.** *2026-08-08.* Rust constants are the source of
truth, with a generated JSON artifact served to the GUI and a CI drift check, matching the OpenAPI
pipeline. **There is no registry table in the database and none is to be added** — the core appears
there only as a version snapshot. The registry is a contract, and every other contract in this stack
is compile-time checked; making this the one hand-editable artifact would put the least checking on
the piece with the widest blast radius. Raised in *The definition layer* and *Data model*.

**D-017 — A stale server-discovered field warns and falls back; it never blocks a save.** *2026-08-08.*
The definition is flagged and evaluates locally. Blocking would mean a Plex upgrade breaking saved
work the user never touched. Enforced by I-DEF-2. Raised in *The definition layer*.

**D-018 — User-defined computed fields are in, in a restricted form.** *2026-08-08.* One arithmetic
operation over two registered numeric fields, in a closed `user.*` namespace. No constants, no third
operand, no nesting, and no computed field may reference another. That last rule is the load-bearing
one: permitting nesting would reconstruct an expression language one field at a time, with no
decision ever having been taken to build one. Enforced by I-DEF-6. Recorded as CR-1. Raised in
*The definition layer* and *Data model*.

**D-019 — User-scoped fields are marked as such now.** *2026-08-08.* `item.viewCount`,
`item.lastViewedAt`, `item.userRating`, and `item.isWatched` carry a `userScoped` attribute recording
that they resolve against a specific account. At Tier 0 that account is the admin. Marking them now
is what stops Tier 1 per-user targeting from silently changing what existing definitions mean.
Raised in *The definition layer*.

**D-020 — Season and episode field depth stays minimal at launch.** *2026-08-08.* Expanding the
`media.*` namespace across episode scope roughly triples the registry; it waits until real rules
justify the cost. Raised in *The definition layer*.

**D-021 — Asset bytes live on disk, content-addressed, outside the database.** *2026-08-08.* The
cost is stated rather than hidden: a database restored without its asset directory has dangling
digests, so backup must treat the pair as a unit. Raised in *Data model*.

**D-022 — Teardown is built at launch.** *2026-08-08.* A first-class operation reversing every change
Afisharr has made to a library, resumable after a crash, reporting everything it could not restore.
It is the acceptance test for reversibility rather than a feature beside it: I-REV-1, I-REV-2,
I-REV-3, I-REV-5, and I-REV-6 are each individually testable, but only a full teardown against a
populated library exercises them together and in the order they actually occur. The integration test
in I-REV-4 is therefore the highest-value single test in the suite, and its fixtures are worth
building early rather than last. Recorded as CR-2. Raised in *Invariants*.

**D-023 — Migrations are forward-only, with a pre-migration backup and downgrade refusal.** *No
distinct date recorded; part of the 2026-08-08 closing batch.* The recovery path from a bad upgrade
is *restore*, which is what makes the backup non-skippable rather than advisory. Enforced by I-DATA-7
and I-DATA-8.

**D-024 — Concurrency is WAL, one write actor, leases per logical pass, and no transaction across
I/O.** *No distinct date recorded; part of the 2026-08-08 closing batch.* A single writer removes a
class of contention rather than managing it; leases prevent two passes rather than two writes.
Enforced by I-DATA-2 and I-DATA-3.

**D-025 — Both lifecycle granularities ship at Tier 0, whole-title by default.** *2026-08-08.* A
subject tracks either a whole title or one season, carried by `seasonNumber`. The whole-title subject
always exists; season subjects are added beside it, opt-in per show through `seasonGranularity`, and
only for seasons inside `countdownWindow`. The whole-title subject therefore always computes a status
for the show's own poster, and a show with nine aired seasons and one upcoming season gets one season
subject rather than ten.

*Why both rather than one:* season tracking has Tier 0 value that does not depend on season overlays,
which stay Tier 1.5. A placeholder for an upcoming season of a show already in the library, and
season-level monitoring against Sonarr, both consume it at launch. Rendering is one consumer of this
state, not its purpose. *Why whole-title by default:* the cost of season subjects is per-show, so it
should be paid per show, by the operator who wants it.

*What it costs:* a second granularity reintroduces the contention the lifecycle design exists to
prevent, so placeholder ownership divides explicitly by what is absent, and a season subject
materializes nothing while its show is absent. I-LIFE-4 is the test, and it is the load-bearing one.
The evaluator runs one loop over subjects, not two, because granularity is a field rather than a
type.

*No capability row changes tier.* Season granularity is a refinement inside "Coming Soon item
tracking," which was already T0. No migration: `lifecycle_subjects.season_number` was already
nullable and already inside the unique identity index. Design in §17.2.1; schema note in
§19.12.1. Raised in *Lifecycle*.

**D-026 — The setup wizard reports existing collections and adopts none of them.** *2026-08-08.* It
lists what it found per library, explains what adoption is, states that Afisharr leaves those
collections alone, and links to the page where adoption happens. There is no bulk adoption control in
the wizard.

*Why:* the operator with sixty hand-made collections is the one for whom one click is most tempting
and most alarming. Bulk adoption five minutes after install is the fastest way to make someone feel
they lost control of their own library, and a first impression is the worst moment to spend that
trust — even though teardown (D-022) makes adoption genuinely reversible. Reporting still earns the
step: the operator learns Afisharr sees their collections, learns it will not touch them, and learns
where the control lives. Raised in *The interface*. Surface owned by §5.1.

**D-027 — The product is named Afisharr.** *2026-08-08.* From афиша (*afisha*), "poster" or
"playbill." The word names the product's headline job, and the Soviet poster-design tradition gives
the visual identity a starting point. It sits in the `*arr` family at eight characters, alongside
Prowlarr and Readarr.

The name is now the binary (`afisharr`), the configuration directory, the database file
(`afisharr.db`), the crate (`backend/crates/afisharr`), the pack namespace root (`afisharr.core/…`), and every
document filename. `afisharr` was free on crates.io, npm, and as a GitHub organisation when checked on
2026-08-08. The repository directory itself is unchanged and is the owner's to rename.

*Why it was worth closing now:* the placeholder was load-bearing in six places, and every document
written after this point would have added more. A dedicated citation-checking tool made the rename
verifiable rather than hopeful. Raised as an open question in earlier drafts.

**D-028 — The licence is AGPL-3.0-or-later.** *Chosen 2026-08-08, "until further notice" — so a
change is a dated change request, not a silent edit.* `-or-later` rather than `-only` follows the
FSF's own recommendation and keeps a future version available; switching to `-only` is a one-line
change to the licence header while the project has no outside contributors.

Five consequences, each of which costs more to retrofit than to adopt:

1. **The network clause is the reason to choose AGPL over GPL, and it binds this product
   particularly.** Afisharr serves a SPA over HTTP. Section 13 obliges anyone who modifies it and
   lets others use it over a network to offer those users the modified source. A household instance
   satisfies this by handing over the source; the clause exists for someone hosting a modified fork
   as a service.
2. **Every dependency must be AGPL-compatible, and this needs a machine check rather than
   vigilance.** MIT, Apache-2.0, BSD-2/3, ISC, Zlib, and MPL-2.0 are all fine. BSL, SSPL, Elastic,
   CC-BY-NC, and any proprietary blob are not. A Rust dependency tree runs to hundreds of crates and
   a transitive addition can introduce an incompatible licence silently, so the check belongs in CI.
   *Non-functional requirements* owns specifying it.
3. **The BSD-3-Clause protocol reference stays compatible.** BSD-3 is permissive and
   GPL-compatible. The obligation is unchanged: retain the notice and the three clauses in the
   distribution, and credit in the documentation.
4. **Packs are data, not derivative works — this project's stated position.** A collection
   definition, an overlay template, or a poster template is a document Afisharr reads. It is not
   linked against the binary, so a community pack may carry whatever licence its author chooses.
   First-party packs ship under the project licence. This is the first question a contributor asks,
   and the position should be confirmed by whoever reviews the licensing before it is published as
   guidance.
5. **A public repository makes several things launch work rather than later work.**
   `CONTRIBUTING.md`, issue templates, and a `SECURITY.md` naming a disclosure process. The
   contributor-agreement choice had a deadline — it stops being freely choosable the moment an
   outside contribution lands — and closed the same day as D-031.

The network clause also lands as a user-interface requirement rather than a paperwork one: the
running instance must offer its own source. §6.4 specifies the source link, why it must resolve
to the running version rather than to a branch, and why a fork must be able to retarget it.

Raised as an open question; full treatment belongs in *Non-functional requirements*.

**D-029 — The security model assumes an internet-exposed instance.** *Decided 2026-08-08.* Not "LAN
behind a reverse proxy, hardened just in case" — the default assumption is that the instance is
publicly reachable, and the documentation says so.

What this makes launch work rather than later work:

1. **Rate limiting and brute-force lockout** on authentication, on the API, and on any endpoint that
   reaches a third-party service on the caller's behalf.
2. **Session hardening.** Short-lived rotating sessions, `Secure`, `HttpOnly`, `SameSite`, and both
   an idle and an absolute timeout. CSRF stays always-on with no toggle.
3. **Security response headers.** HSTS, CSP with a `frame-ancestors` policy,
   `X-Content-Type-Options`, and `Referrer-Policy`.
4. **A published disclosure process** in `SECURITY.md`, with a stated response window.
5. **The filesystem browser's root jail stops being a nicety.** On a reachable instance it is a
   path-traversal boundary, and it needs the tests a boundary gets.

*The trap worth naming now:* the trust-proxy toggle and rate limiting interact, and the failure is
silent. If `trustProxy` is on and the instance is reachable directly rather than only through the
proxy, an attacker sets `X-Forwarded-For` freely and every per-IP limit above becomes decorative. The
setting must therefore constrain *which* proxy is trusted rather than whether forwarded headers are
honoured at all. *Non-functional requirements* owns the specification and the invariant.

**D-030 — Scale target: 200,000 items, 2,000 collections, across several Plex libraries.** *Decided
2026-08-08.* The performance budgets are written against this figure. This is an architectural
constraint, not a tuning parameter. Four things follow:

1. **No pass may load the library into memory.** Reconciliation streams, in bounded batches, with
   the working set independent of library size.
2. **The render cache needs a size cap and an eviction policy.** At this scale an unbounded cache is
   a disk-exhaustion bug with a delay fuse.
3. **The indexes on the hot paths must be measured at scale, not asserted.** A plan that is fine at
   5,000 rows and quadratic at 200,000 looks identical in review.
4. **D-024 survives, with a condition.** One write actor is still right, but writes must batch into
   transactions sized by work rather than by item. Two hundred thousand single-row transactions is an
   fsync per item and minutes of wall-clock; the same rows in batched transactions is seconds. The
   decision is unchanged; the implementation note is now binding.

*Stated plainly, because it is the part most likely to hurt:* 2,000 collections is where placement
gets hard, and placement is already the highest-risk subsystem. The gap-budget scheme (`gapBudget`
defaults to 8) was never sized against a sequence this long, which raises the stakes on both spikes
rather than lowering them — Q-014 must calibrate against a sequence of this order, and Q-015 decides
whether this is one sequence of 2,000 or several shorter ones. Sequence *these two spikes first*.

**D-031 — Contributions are certified by DCO, not assigned by CLA.** *Closed 2026-08-08; raised by
D-028.* Every commit carries a `Signed-off-by` line (`git commit -s`). The GitHub DCO check on pull
requests is authoritative; a local pre-commit configuration carries a local mirror so the failure
arrives before the push rather than after.

*The accepted consequence:* without a CLA, AGPL-3.0-or-later becomes effectively permanent once the
first outside contribution merges. Relicensing after that needs every past contributor's agreement.
D-028's "until further notice" is therefore a window that closes by itself, and this decision is what
closes it. That is the trade DCO buys: a contribution process nobody has to sign a document to enter.

`CONTRIBUTING.md` must carry the DCO 1.1 text verbatim and explain `git commit -s`. It is not written
yet, because its other half — branch strategy and review process — is implementation-plan work rather
than a decision.

**D-032 — The secret encryption key is a file beside the database, not a passphrase.** *Closed
2026-08-08.* XChaCha20-Poly1305 with a per-secret nonce; a 32-byte key from the OS CSPRNG at
`secrets.key`, mode `0600`, overridable by `AFISHARR_SECRET_KEY`. An operator passphrase at startup
was rejected because it breaks unattended container restart, which is the primary deployment. The
environment variable was rejected as the *default* because env vars leak into process listings, crash
dumps, and container inspection; it remains an override for operators with a secret manager.

*The limit, stated rather than implied:* this protects a stolen database file. It does not protect
against an attacker who can read the filesystem, because that attacker reads the key too. Spec in
*Non-functional requirements* §4.5.

**D-033 — The backup unit is the database plus the base posters, and the key is opt-in.** *No
distinct date recorded; part of the 2026-08-08 closing batch.* The render cache, the HTTP cache, and
placeholder stubs are excluded because they are regenerable — which is what makes the backup
tractable, since the render cache is roughly half the bytes and all of the churn. `secrets.key` is
excluded by default, making the common restore a credential-less one.

*Why base posters are not optional:* once an overlay is applied, Plex holds the overlaid poster and
the pristine original exists only in Afisharr's asset store. A backup that skips them restores a
system that can no longer undo itself, silently, because every digest still resolves to a row. That
would quietly void I-REV-2, I-REV-4, and I-REV-5. Enforced by I-SEC-5 and I-SEC-6; procedure in
*Non-functional requirements* §6.

**D-034 — The original milestone plan is superseded.** Recorded and reproduced in full at §23.3
below, alongside the rest of the superseded-artifacts record.

**D-035 — The test suite splits into merge, nightly, and release lanes.** *Closed 2026-08-08.* Merge
runs every table-driven and unit-level invariant inside 10 minutes. Nightly runs crash injection, the
property tests, teardown, and the scale runs. Release adds the restore path and the Plex version
matrix.

Two rules stop the split decaying: a nightly failure blocks the next merge to `main` unless waived
with a named reason, and an invariant moves from merge to nightly only with a recorded measurement,
never because it feels slow. Teardown is nightly despite D-022 making it the acceptance test for
reversibility, because it needs a populated fixture library and a full apply-then-reverse cycle.
Lanes in *Non-functional requirements* §10.1. Raised in *Non-functional requirements*.

**D-036 — The Plex fake is adversarial and deterministic, and is scheduled as its own work.**
*Closed 2026-08-08.* It reproduces silent no-op moves, unrecognised artwork URL formats, rating-key
churn, partial scan states, independent sort-title value/presence/lock, mid-pass failures, and
machine-identifier change. Every misbehaviour is triggered by an explicit seeded scenario, because a
fake that misbehaves randomly produces flaky tests, which get muted, which is worse than not having
it.

It is not a Plex emulator: it implements only the surface Afisharr calls, and a release-lane contract
test against a real server is what keeps it truthful. Fidelity contract in *Non-functional
requirements* §10.2. Raised in *Non-functional requirements*.

**D-037 — Docker on `linux/amd64` and `linux/arm64` is the supported deployment.** *No distinct date
recorded.* Native binaries are published best-effort. `linux/armv7` is unsupported: a 32-bit address
space against a 1 GB memory ceiling and a 50 GB asset store is not a configuration that can be made to
work. SQLite is bundled rather than taken from the system, because the schema needs `STRICT` tables
and expression indexes, and a system copy that predates them fails confusingly rather than at
startup. Matrix in *Non-functional requirements* §5.

**D-038 — Afisharr collects nothing.** *No distinct date recorded.* No telemetry, no analytics, no
crash reporting, no update ping. The accepted cost is real: no usage data and no aggregate crash
signal, ever. Support bundles are the one place data could leak, so they exclude the `secrets` table
entirely, redact tokens by pattern, and show the operator a manifest before the file exists. What
providers learn regardless — the instance's IP and what it asked for — is stated plainly rather than
left to be discovered. Posture in *Non-functional requirements* §8.

**D-039 — The delivery plan is foundations-first, with the two spikes as a parallel track.** *No
distinct date recorded.* The main line runs the implementation plan's milestones in dependency order;
Q-015 and Q-014 run alongside from an early milestone and must land before the placement-heavy
milestone begins. Plan detailed in the implementation plan.

*Why foundations first when placement is the riskiest subsystem:* the foundations here are unusually
low-risk. The schema is fully specified and its DDL was already executed; the definition engine and
registries are specified to the field. Building them is execution, not discovery. The discovery is
concentrated in placement, and it is drained by two spikes rather than by building placement early.

*Why the spikes are parallel rather than first:* they need a real Plex server and a Plex client and
nothing else — no schema, no engine, no collections. Running them alongside the main line costs
almost nothing. What they must not do is finish after the placement-heavy milestone starts, because
that milestone designed against an assumed answer is that milestone built twice.

*The plan carries no dates and no capacity figures*, deliberately. Both would be fabrications against
work that has not started, and D-034 warned specifically against a plan compressed to fit a shape
that no longer applies. Exit criteria are invariants instead, all 97 assigned exactly once.

**D-040 — Source capability flags belong to the rung that answered, not to the source.** *Raised by
CR-3, 2026-08-09.* A source declares an ordered ladder of endpoints — a structured API first, a
structured payload embedded in a page second, markup last — and each rung carries its own
`affirmativeEmpty`, `ordered`, and `deterministic`. The engine applies the flags of the rung that
actually produced the result.

*Why this is not a detail:* the whole safety argument for the fallback ladder rests on
`affirmativeEmpty` being true only where a zero-item response is genuinely trustworthy. A source
whose primary rung affirms emptiness and whose fallback rung cannot must not carry one flag, because
the flag would be right exactly when the fallback was not needed. Declaring capabilities per source
would make the first fallback silently unsafe, which is the moment the safeguard exists for. Shape in
§13.6.1; tested by I-SRC-8.

**D-041 — Volatile third-party parameters travel out of band, and carry values only.** *Raised by
CR-4, 2026-08-09.* Query hashes, endpoint paths, and challenge signatures are fetched from a signed
feed, verified, and constrained by a registry the binary ships. The feed can change a declared
parameter's value. It cannot introduce a parameter, change a type, or alter behaviour.

*Why the constraint is the decision:* an unconstrained remote configuration feed is remote code by a
slower route, and D-001 already rejected the class of designs where an external party decides what we
execute. Values-only with a shipped registry keeps the repair path — one file, no release — while
leaving the blast radius of a compromised feed no larger than the stale value it replaced. Schema in
§19.11.5; tested by I-SEC-7.

**D-042 — Wholesale reference datasets are imported, staged in SQLite, and swapped atomically.**
*Raised by CR-5, 2026-08-09.* Never held in memory, never merged row by row into a live table, never
partially applied.

*Why atomically:* a partial import is indistinguishable, at read time, from a complete one whose
provider dropped half its rows. The engine would treat a truncated download as a fact about the world
and act on it — the same failure class recorded against a core invariant, arriving through a
different door. Schema in §19.11.4; tested by I-DATA-13.

**D-043 — Every cache key includes the version of the code that interprets what it stores.** *Raised
by CR-6, 2026-08-09.* The render key already carries the renderer version. The HTTP cache key carries
the per-source parser version for the same reason.

*The general rule this states:* a cache keyed only on inputs is correct only while the function from
inputs to outputs is fixed. Where that function ships as code that changes, its version is part of
the input. Any future cache added to this system inherits the obligation. Schema in *Data model*
§11.3; tested by I-DATA-12.

**D-044 — Parameterization is an install-time operation, never a stored-document feature.** *Raised
by CR-7, 2026-08-09.* Pack manifests declare variables; the installer resolves them and writes
concrete definitions; the resolved values are stored so a pack upgrade can re-materialize.

*Why the boundary sits there:* definitions must carry no logic and must diff meaningfully. Both
properties survive if substitution happens before storage and neither survives if it happens after.
This also keeps one rule intact that nothing else protects: what the GUI shows is what runs, with no
resolution step between them. Manifest in §12.8; schema in §19.10; tested by I-DEF-8.

**D-045 — First run is claimed with a bootstrap token printed to the console.** *Raised by the
first-run sweep, 2026-08-09.* An unconfigured instance prints a `xxxx-xxxx-xxxx` token to stdout at
startup and refuses every route but health until a caller returns it. Returning it leases the wizard
to that browser for ten minutes at a time.

*Why this is a correctness decision rather than hardening:* D-029 already commits to an
internet-reachable instance. Under that commitment, "the first visitor creates the admin account" is
not a convenience — it is an unauthenticated grant of the Plex token, which §21.4.1 names as the
asset that authorises deletion. The console is the one channel an attacker who merely found the port
cannot read, and reading it costs the real operator one glance at the terminal they just used.

*Why validated and not consumed:* a consumed token strands an operator who loses the cookie, on the
console where the proof already lives. It stays live for its fifteen minutes so the same paste works
twice.

*Why the claim TTL is shorter than the token's:* a stranded claim must expire while the token that
created it is still usable, or the recovery path needs a container restart. Ten against fifteen.

*Rejected:* an environment variable holding a fixed admin password, because it leaks into process
listings and container inspection, and because a value that never rotates is a permanent credential
for a one-time act. Rejected: no gate at all with a warning in the README, because the failure is
silent and total. Mechanism in §19.6.1; tested by I-SEC-8.

**D-046 — The wizard's resume point is derived from state; recovery is by admin credentials.**
*Raised by the first-run sweep, 2026-08-09.* The server computes which wizard step is next from what
the database contains, and the client cannot name a step. Once the admin account exists, those
credentials mint a claim without the token.

*Why derived:* a client-supplied step is a request to skip the steps before it, and the first step
is the claim. The same rule that makes the wizard tamper-resistant also makes it honest after a
crash — a step whose write failed is not complete because the browser remembers completing it.

*Why recovery exists:* the token dies with the process, and a container that restarts mid-wizard is
ordinary. Without a second path, the operator's own account — created at step 2 and sitting in the
database — would be unable to open the wizard that created it. Resume table in §7.14; tested by
I-UX-10.

**D-047 — Modular structure is a build gate, not a refactor to schedule.** *Raised while recording
the structural requirement, 2026-08-09.* The source tree divides into subfolders by domain, every
file states one thing, god files are prohibited outright, every file carries a soft and a hard size
limit, and every module exposes a narrow public surface it declares in one place. All five bind from
the first commit of Phase 0 and are checked on every change.

*Why this is a decision rather than a preference:* this project is fifteen phases of accretion
against a schema of 68 tables, sixteen source adapters, and two ordering surfaces. Every one of those
is a plausible reason to add "just one more thing" to a module that already exists. Structure is the
only quality property that no single commit destroys — a file crosses from reviewable to unreviewable
across twenty correct changes, none of which is the culprit. A rule applied per change is therefore
the only form of this rule that works.

*Why size limits are soft first, hard second:* a single hard threshold is either loose enough to
permit the drift or tight enough to be routinely overridden, and an override that becomes routine
stops being read. Two thresholds separate two conversations: at the soft limit the author justifies
once in the PR, and at the hard limit the author justifies to a second person and leaves the reason
in the file.

*Why the hard limit has an exception at all:* the same argument. A ceiling with no exception is
overridden anyway — in a PR comment, off the record, leaving nothing in the file for the next reader.
It also collides with the rest of this decision: a 750-line module with one responsibility often has
exactly one available split, into `types` and `impls`, which is the layer-shaped division §24.6.1
prohibits. A rule that forces what another rule forbids is decided by whichever reviewer is paying
attention. Two signatures and a header comment cost more than a silent override and less than a bad
split, and unlike either, they leave a record that a later reviewer can disagree with.

*Why the limits are explicitly the weaker half of the rule:* a script can measure lines and cannot
measure whether four unrelated helpers share a file. §24.6.2 and §24.6.3 bind at any size, and a
green `wc -l` is not evidence that they hold.

*Rejected:* a periodic refactoring phase, because it schedules the cleanup after every phase that
depends on the structure is already written against the mess. Rejected: a lint rule as the sole
control, because the failure this prevents — unrelated responsibilities in one module — is a
semantic property no linter available for either surface can see. Requirement in §24.6; gates and
checklist lines in the implementation plan's §A.1–§A.4.

**D-048 — The two stack rule files are normative, and they are read before code is written.**
*Raised while recording the coding guidelines, 2026-08-09.* `.augment/rules/frontend-dev-pro.md`
covers the frontend stack and `.augment/rules/backend-rust-dev-pro.md` covers the Rust backend
stack. Both are normative, alongside this document and the implementation plan. Every author, human
or agent, reads the file for the surface they are about to touch before writing code for it.

*Why the files are normative rather than reference material:* they are the only place the current
idiom for each stack is written down against the pinned versions. §24 is long, but it is a project
layer — it assumes the stack idiom and states what Afisharr does on top of it. Demote the rule files
to "background" and every construct §24 does not happen to mention has no stated right answer, which
in practice means whichever answer the author already had.

*Why read-first rather than check-at-review:* the failure these files prevent is silent. Both stacks
accept the previous generation's idiom and the adjacent ecosystem's habits without complaint —
`export let` compiles, `unwrap()` runs, a hand-rolled `fetch` returns data, and every gate in §A.1
stays green. So review is the first place the mistake can surface, and by then it is a rewrite rather
than a choice. Reading first costs one pass over a document; reading last costs the diff.

*Why both surfaces are named explicitly rather than "the relevant rules":* an author who has to work
out which file applies sometimes decides neither does. Two files, one per surface, each named where
the reader starts (§0.1, §24.1, and the implementation plan's *How to read this*), removes that step.

*Why the project layer wins on conflict:* the rule files are written for the stack in general and
cannot know that this frontend has no server runtime (§24.4), that the API contract is generated
(§24.5), or that files carry size limits (§24.6). Where §24 contradicts a rule file it is because
this project's architecture requires it, and §24 says so. Where §24 is silent there is no conflict,
and the rule file binds unchanged.

*Rejected:* copying the rule files into §24, because two copies of a version-anchored document
diverge at the first stack upgrade, and the copy inside the PRD is the one that would go stale
unread. Rejected: leaving them as an unnamed convention that agents happen to pick up, because a
convention nothing states is a convention no reviewer can enforce. Requirement in §24.1; authority
in §0.2; gate and checklist lines in the implementation plan's §A.1–§A.4.

**D-049 — The implementation plan carries progress checkboxes, and a checked box records a check
that passed.** *Raised while recording the plan's tracking convention, 2026-08-09.* Every subtask and
every **Done when** clause in the implementation plan carries a `- [ ]` marker, ticked in place as
work lands. The marker records that the clause was checked and held. It is never the thing that makes
a task done, and a ticked box beside a failing gate is a documentation bug in which the gate wins.

*Why this is not the fabrication D-039 refuses:* D-039 keeps dates and capacity figures out of both
documents because each is a prediction about work that has not started, and a prediction stated as
fact is a fabrication. A checkbox is the opposite kind of claim. It says a named command exited zero
or a named invariant's test passed — a past event, re-runnable by anyone who doubts it. The two rules
do not collide, and saying so here stops the next reader reopening D-039 by accident on the grounds
that the plan now appears to track something.

*Why the boxes live in the plan rather than in a tracker:* this document and the implementation plan
are the complete normative set (§0.1). A second artefact holding the same fact is a second thing to
keep true, and the one that goes stale is always the one nobody has to read in order to do the work.

*Why the subtask level and not the task level alone:* a task here runs to seven subtasks of
independent work, several of them days apart. One box per task reports nothing until the whole task
lands, which is exactly when a reader no longer needs the report.

*Why a phase carries no box:* a phase is finished when its exit invariants pass, which is a property
of a build rather than of this document set. A phase box could only be a summary of the task boxes
beneath it, kept in step by hand — and a summary is believed over the thing it summarises the first
time the two disagree.

*Rejected:* checkboxes on the numbered subtask markers themselves (`1. [ ]`), because a task-list
marker inside an ordered list is an undocumented extension of an extension: it renders on github.com
and inconsistently elsewhere, and this plan is read in editors as well as on the web. The subtasks
keep their numbers as text on an unordered box instead. Rejected: no tracking at all, with the git
history as the record, because "what is built" is not a question a log answers without reading every
commit against a plan of 122 tasks. Convention and the two-kinds-of-checkbox rule in the
implementation plan's *How to read this*; the appendix's own boxes are a reusable template and say so
where they live.

**D-050 — The interface ships one palette, `tangerine`, and defaults to the system's mode with a
light fallback.** *Raised while specifying the visual layer, 2026-08-13.* The palette is the tweakcn
`tangerine` registry item, fetched with
`bunx shadcn@latest add https://tweakcn.com/r/themes/tangerine.json` and transcribed into
`frontend/src/app.css` (`:root` and `.dark`) and `uno.config.ts`. The default mode follows
`prefers-color-scheme`; an explicit operator choice overrides and persists; where the preference
cannot be read, the interface renders light. Requirement in §10.4, placement in §24.3.5.

*Why a named palette rather than shadcn's defaults:* the defaults are a neutral greyscale with a
near-black primary, which is what every shadcn project looks like before anybody decides anything. A
media-library tool that runs on a television-adjacent screen and is read at night is not served by
looking like an unconfigured admin panel. Choosing once, writing it down, and binding every component
to the semantic tokens is cheaper than fifteen pages each reaching for a color.

*Why the fallback is light and not dark:* `prefers-color-scheme` resolves to `light` in current
browsers when the operator has expressed no preference, so the fallback only fires where the query
cannot run at all. In that case the interface knows nothing about the room it is in. A dark surface
shown to somebody who did not ask for it reads as broken in daylight; a light one reads as plain at
night. The failure that looks like a bug is the one to avoid.

*Why the fallback is stated at all:* because the library gets it backwards by default.
`mode-watcher` tests `(prefers-color-scheme: light)` and maps every non-match to dark, including the
no-`matchMedia` case. Left alone, the undetectable case lands in dark — a default nobody chose,
arriving through a dependency's internals. Stating the requirement makes it testable.

*Why the fonts are self-hosted:* the palette names Inter, JetBrains Mono, and Source Serif 4, and the
idiomatic way to load them on this stack is `presetWebFonts` with Google as the provider. That sends
the operator's IP to Google on every page load of a product that collects nothing (D-038). The faces
ship in the binary instead.

*Rejected:* an operator-selectable palette in Tier 0, because a theme picker is a settings surface, a
persistence concern, and a contrast-audit obligation multiplied by however many palettes ship — for a
single-operator tool whose owner can edit two CSS blocks. Rejected: taking the CLI's output as
authoritative, because `shadcn` is the React CLI writing Tailwind-v4 `@theme` shape into a repository
that has neither Tailwind nor that schema; the JSON's values are the contract and the CLI is a
convenience. Rejected: dark-by-default, which suits the night use this product is built for but
misreports the system preference on every daytime first run.

**D-051 — Interface work loads the `frontend-design` skill before it writes markup.** *Raised
alongside D-050, 2026-08-13.* An agent building or reshaping any rendered surface — a page, a
visual component, a layout, the theme — loads its `frontend-design` skill first. Copy fixes, typing
fixes, and changes with no rendered consequence are outside it. Rule in §24.3.5.1; checklist line in
the implementation plan's §A.3; reviewer line in §A.4.

*Why this needs saying when §24.3 already runs to thirteen subsections:* every one of them constrains
correctness, and a page can satisfy all of them and still be shapeless. Nothing in the rule file, the
gates, or the invariants fails when an interface is merely dull, so nothing catches it until a human
looks — and by then it is fifteen pages deep and the fix is a redesign.

*Why the read-first framing, matching D-048:* the failure mode is identical. An agent that starts
writing reaches for its defaults immediately, and the defaults are the centred card on the neutral
page. Design advice read afterwards is design advice applied as a rewrite, which is exactly the cost
D-048 exists to avoid on the code side.

*Why it is not in the normative set:* the skill is a working aid for the author, not a source of
product requirements, and it lives outside the repository where this document cannot pin its version.
§10.4 fixes the palette and §24.3.5 fixes the tokens; the skill operates inside those, never against
them. Where the two disagree, this document wins, and that ordering is what keeps a general design
aid from quietly reopening a decided question.

*Rejected:* a subjective "the interface should look good" line with no trigger and no owner, because
it is unenforceable and every reviewer reads it differently. Rejected: a design-system document of our
own in §24, because writing one is a project, keeping it true is a second project, and neither is
this product.

### 22.4 Change requests against the frozen scope

The scope ledger froze on 2026-08-08. Every entry below is a recorded, dated reopening — never a
silent edit to the tables that preceded it. Each carries the reason and the cost, so a future reader
can tell an argued decision from an accumulated one.

**CR-1 — User-defined computed fields — T0 (in) — 2026-08-08.** An operator may define a named field
as one arithmetic operation over two registered numeric fields — a "rating gap" of critic score minus
audience score, for example — usable anywhere a numeric registry field is legal.

**Constraints that make this safe:** one operation, two operands, no constants, no nesting, and no
reference to another computed field. Keys live in a closed `user.*` namespace and are tombstoned
rather than deleted, so a retired key never returns meaning something else.

**Why the constraints matter more than the feature:** the unrestricted version of this is a string
expression language, which the engine design forbids on security and validation grounds. Permitting
computed fields to reference each other would rebuild that language one field at a time, with no
decision ever having been taken to build it.

**Cost:** one table, one editor form, six validation rules. Small. Recorded as D-018; schema in
§19.8.1.

**CR-2 — Teardown — T0 (in) — 2026-08-08.** A first-class operation reversing every change Afisharr
has made to a Plex library: base posters restored byte-exactly, sort-title prefixes stripped and lock
states restored, applied labels removed, managed collections and placeholders deleted, native hub
placement restored. Resumable after a crash or cancel; reports everything it could not restore rather
than skipping silently.

**Why:** it is the cheapest possible answer to "what if I try this and don't like it." The deeper
reason is that it is the only thing that exercises the reversibility invariants together and in the
order they actually occur — without it, I-REV-1 through I-REV-6 are individually tested and
collectively unproven.

**Cost:** substantial. Crosses placement, rendering, lifecycle, and collections, and needs an
integration test with a populated fixture library plus a crash-resume variant. The milestone rebuild
should build those fixtures early rather than last, because the teardown test is the highest-value
single test in the suite.

Recorded as D-022. Product commitment recorded in the specification's §2.

**CR-3 — IMDb is an API-tier source — T0 (already in, tier corrected) — 2026-08-09.** IMDb charts and
custom lists move from the scraped tier to the API tier, with `affirmativeEmpty` decided per rung of
the endpoint ladder rather than per source.

**Why this is a correction and not new scope:** the capability scope already recorded IMDb charts as
Tier 0 "via documented JSON/GraphQL endpoints," but the specification's source list omitted IMDb from
the API-first list and the engine design listed `imdb.chart` and `imdb.list` under the scraped tier.
Three documents, two answers. Under the stated authority order, the decisions ledger wins, so the
other two are amended.

**What the prior-art pass established:** IMDb serves charts, lists, watchlists, and advanced search
from a JSON endpoint that returns cursor-paginated results and typed error codes. A typed "not found"
or "forbidden" code is exactly the affirmation the engine design requires, so a zero-item response on
that rung is trustworthy.

**The cost is the rung distinction, not the endpoint.** The endpoint authenticates its query by a
hash that the provider rotates on its own schedule, so a working source can stop working without
anything changing on our side. That is what CR-4 exists to absorb, and what makes the fallback rung
mandatory rather than decorative.

**Cost:** small on its own. Two registry rows change tier, one capability field becomes per-rung.
Recorded as D-040; registry shape in §13.6.1; tested by I-SRC-8.

**CR-4 — A volatile-parameter channel — T0 (in) — 2026-08-09.** A small signed data feed, fetched at
runtime and separate from both the binary and the pack system, carrying values that a third party can
change without notice: query hashes, endpoint paths, and challenge-page signatures.

**Why:** without it, a provider rotating a query hash breaks a Tier 0 source for every installed
copy, and the only repair is a release. D-023 makes upgrade forward-only with a mandatory
pre-migration backup, so shipping a release is the most expensive repair available and the least
suitable for the most frequent breakage.

**What keeps it from becoming a remote-code channel:** the feed carries values only, never
behaviour. Each parameter is declared in the registry with a name, a type, and a syntactic
constraint, and a fetched value that fails its constraint is rejected and the last-known-good value
is kept. The feed is signed, verified before use, and cannot introduce a parameter the shipped
registry does not already declare — so the worst a compromised feed achieves is breaking a source
that a stale value would have broken anyway.

**Cost:** one table, one fetch job, one signature check, one doctor row. Recorded as D-041; schema in
§19.11.5; tested by I-SEC-7.

**CR-5 — Bulk reference datasets are a first-class storage class — T0 (in) — 2026-08-09.** Some
providers publish their whole dataset as a periodic file — ratings and genres for every title,
refreshed daily, free and unauthenticated. Importing one file is cheaper and more complete than
asking per title, and at the scale target it is the only viable shape.

**Why it needs its own storage class:** the existing homes are both wrong. The HTTP cache is keyed
per request and purged by TTL, and a 20 MB body is not a cache entry. The identifier-mapping table is
bulk reference data of exactly the right shape but is scoped to identifier mapping, and widening it
would mix two datasets with different refresh cadences into one table.

**The property that matters:** an import is all-or-nothing. A truncated download, a changed column
layout, or a partial parse must leave the previous dataset in place and report, never half-replace
it. This is the same obligation already placed on discovered-field refresh, applied to a second kind
of wholesale replacement.

**Staging, not loading.** The dataset is streamed into SQLite and queried there. Holding it in memory
would breach the footprint budget for a table that a join answers.

**Cost:** two tables, one import job, one integrity check. Recorded as D-042; schema in *Data model*
§11.4; tested by I-DATA-13.

**CR-6 — A cache key carries the version of the code that interprets the bytes — T0 (in) —
2026-08-09.** `http_cache.cache_key` gains a parser-version component, so a cached response is only
ever read by the parser version that keyed it.

**Why:** this is the argument already made and accepted for the render key. The specification puts
the renderer version in the render key because a rasteriser change alters the output for identical
definition-layer inputs, and without it a rendering improvement matches every existing cache entry
and therefore reaches nobody. A response parser has the same property: fixing a misparsed field
changes the parsed result for identical bytes, and every cached body still parses the old way until
its TTL expires. One subsystem got the safeguard and the other did not.

**Why it is not solved by clearing the cache on upgrade:** the failure is not limited to upgrades.
Parser versions are per source, and a source whose parser is unchanged should keep its cache across a
release. A blanket clear trades a correctness bug for a thundering herd at every provider at once.

**Cost:** one column, one constant per source adapter. Recorded as D-043; schema in *Data model*
§11.3; tested by I-DATA-12.

**CR-7 — Packs parameterize; stored definitions stay pure — T0 (in) — 2026-08-09.** A pack manifest
may declare variables. The installer substitutes them and writes concrete definition documents. What
lands in `definitions` contains no substitution syntax, no conditionals, and no references to
variables.

**Why:** a media-info overlay pack has roughly thirty near-identical variants, and the launch pack set
has several such families. Without a parameter layer each variant is a separate hand-maintained
document, which is both an authoring cost and a correctness risk — the variants drift, and a fix
applied to one is silently absent from twenty-nine.

**Why the substitution belongs to the installer and not to the engine:** definitions must be pure
data with no logic, and they must be diffable and round-trippable. A stored document containing
`<<variable>>` breaks both — it cannot be validated against the field registry until something
resolves it, and two documents that differ only in an unresolved variable diff as identical. Putting
the substitution in the installer keeps every stored document validatable at rest and keeps the
property that the GUI edits exactly what runs.

**What this costs at upgrade:** the resolved variable values must be stored, or a pack upgrade cannot
re-materialize the definitions the user did not fork. That is the one piece of state this adds.

**Cost:** one manifest field, one table, one installer stage. Recorded as D-044; manifest shape in
§12.8; schema in §19.10; tested by I-DEF-8.

---

## 23. Open questions

### 23.1 Open questions Q-nnn

Every question still genuinely open, with the decision it blocks. A question that is closed is a
decision in §22.2 or §22.3, not an entry here.

| Id | Question | Blocks | Owner |
| --- | --- | --- | --- |
| Q-002 | **How much of the collection editor is progressive disclosure?** Six configurable sections, most with sensible defaults. Showing everything intimidates on the first collection; hiding things is tedious by the tenth | The editor layout. Retrofitting disclosure into a form costs more than designing it in | §7.3 |
| Q-003 | **Does the dashboard show engine-facing status or library-facing results?** It currently specifies findings, job outcomes, and source health. Showing what the household sees is closer to browsing, which is a non-goal, but the boundary is genuinely unclear | The most-used page in the product | §7.1 |
| Q-005 | **Are the retention windows right?** The numbers in §19.17.1 are defensible defaults, not measured ones | Nothing yet; revisit once real volumes exist | *Data model* |
| Q-012 | **One more prior-art extraction pass before implementation?** The overlay renderer, poster generation, the individual source adapters, and scheduling are unswept. Every pass so far found design-changing failures | Nothing directly; findings are most useful when fresh in the implementer's mind, so the timing is the question | the design work |
| Q-013 | **Does the home screen board show one merged surface or one board per library?** Blocked on Q-015. One sequence means one list; several means a set of lists with a merge preview, which is a materially different and harder page | The largest single unknown in the frontend plan | §7.6 |

### 23.2 Questions only a real server can answer

Four questions cannot be settled by discussion. The first two belong in the implementation plan as
explicit spikes rather than folded into implementation tasks, where they would be answered by
assumption. The last two are answered by captures the release lane already takes.

**Q-014 — What is the real precision budget before exhaustion?** `gapBudget` defaults to 8, which is
a guess. Too high and moves start failing silently; too low and rebalances run constantly, burning
the budget they exist to protect. The true figure depends on Plex's numeric representation, which is
undocumented and may vary by version. Calibrate against a real server.
*Design: Placement and ordering §4.4.*

**Q-015 — Is the home screen one global sequence, or per-library sequences merged at render?** **This
is the most consequential unknown left in the design.** It determines whether ordering is one
planning problem or several — which changes the planner, the gap accounting, and the lease scope —
and it also blocks Q-013. Schedule it first.
*Design: Placement and ordering §2.*

**Q-016 — What does a real server answer to `PUT /library/sections/{id}/all`?** A size, an empty
body, or `204`. Every edit this build makes turns on it. The client reads an empty answer as "the
server did not say" and reports the edit as incomplete; the reference client reads a blank body on a
write as success (`plexapi/server.py:759`, `plexapi/utils.py:836-839`). If a real server answers
empty, this build reports every landed edit — a collection edit, a label edit, a sort-title write —
as a failure. Not a spike: the release lane's contract test already writes and captures, so it
answers this by name (implementation plan, Task 2.1.7). Until it does, the current reading stands,
because reversing it swaps one unevidenced claim for another. The asking half is built: the write
cycle sends one edit uninterpreted, captures the status and the body, and fails the lane with the
shape it found and the change that shape implies (implementation plan, Task 2.1.9 subtask 6).
*Design: §15.6, and the reversibility invariant `I-REV-3`.*

**Q-017 — How does a real server name and mark a row in the ordering space?** Two facts, one
question, because a wrong answer to either has the same effect and neither is visible when it is
wrong. First, does every manage row carry `deletable`? It defaults to removable
(`plexapi/library.py:3035`), and `HubKind` is read straight off it — so a server that omits it on its
own rows has each of them classified as a promoted collection, and the placement algorithm tries to
reposition an anchor that cannot move (§15.1). Second, is a promoted collection's row identifier
`custom.collection.{sectionKey}.{ratingKey}`? That is what the reference client synthesises for an
unpromoted collection and then reloads the promoted row by (`plexapi/collection.py:212`), and it is
what this build matches a row to a collection by. A server that composes it differently finds no row
for any collection, and finds it silently.

Not a spike, and not left to a default either: the release lane fails by name on a row that carries
no `deletable` and on a promoted row whose identifier this build cannot match, so the first real run
answers both (implementation plan, Task 2.1.9). What guards the second answer in the meantime is the
prefix — a row that does not carry it is not matched at all, so one of Plex's own rows is never
answered as a collection's row whatever its last segment reads as.
*Design: §15.1, and the ordering space in §15.5.*

### 23.3 Superseded artifacts

**D-034 — The original milestone plan is superseded and was deleted, not archived.** The nine
milestones spanning twenty weeks predate the frozen capability scope and do not describe the product
that was decided. Retaining a plan nobody may plan against is a trap for the next reader, so the file
was removed on 2026-08-08 and this entry replaces it.

**The rebuild is done.** The implementation plan (D-039) replaces it with fifteen milestones, a
parallel spike track, and no dates. Its own §3 discharges each requirement below, row by row.

What invalidated the original plan, carried forward as the requirements for the rebuild:

1. Tier 0 grew by roughly a factor of two. Full home-screen ownership, four capabilities restored
   from Tier 1, and a visual poster editor are each substantial bodies of work absent from the old
   milestones.
2. Placement and ordering is now the highest-risk subsystem and had no milestone of its own. It
   needs Q-014 and Q-015 answered before it can be sequenced.
3. The lifecycle system requires persisted state, an append-only audit log, and crash-safe intent
   handling — none of which the old milestone plan accounted for.
4. The field registry gained a server-discovered layer, changing what the early milestones build
   (D-010).
5. Theme music and local assets appeared in the old Tier 0 list but in no milestone; i18n appeared
   nowhere and belongs in the first milestone.
6. The invariants sweep replaced the external references the old plan relied on. There are now 89
   invariants, and D-035 sets which of them gate a merge, a night, and a release.

Two further constraints on the rebuild, both recorded because the temptation runs the other way: the
teardown fixtures in I-REV-4 are worth building early rather than last (D-022), and the plan must not
be compressed to fit the original twenty weeks. The user was told plainly that scope doubled and
accepted it; an optimistic plan would be the least useful thing to produce.
## 24. Coding guidelines (normative)

### 24.1 Status of these guidelines

These guidelines are normative for all code merged into the Afisharr repository. Every rule below is a requirement the code must satisfy, not a suggestion. "Must"/"must not" statements are binding; "should"/"prefer" statements set the default we deviate from only with a documented reason in the change that deviates.

The guidelines cover two independent surfaces that meet only at the generated API contract:

- **The Rust backend** (§24.2): a single binary, stable Rust 1.97.1, edition 2024, Axum, SQLite via SQLx, utoipa-generated OpenAPI, SSE. Stack rule file: `.augment/rules/backend-rust-dev-pro.md`.
- **The frontend** (§24.3): SvelteKit 2 / Svelte 5 / UnoCSS `presetWind4` / shadcn-svelte, built with Bun, compiled to a fully prerendered static site and embedded in the Rust binary. Stack rule file: `.augment/rules/frontend-dev-pro.md`.

**Read the rule file for your surface before you write any code for it.** The two files in
`.augment/rules/` are normative, on the same footing as this section, and the obligation is to read
first — not to write, then check. It binds every author, human or agent, on every change.

Each file states the one current idiomatic pattern for each construct on its stack, anchored to the
pinned versions, with the wrong-but-plausible alternative shown next to it. That pairing is the
point. Neither stack fails loudly when you write the previous generation's idiom: Svelte 4's
`export let` compiles, `Arc<Mutex<…>>` around a read-mostly map runs, and a green test suite reports
nothing about either. A rule file read afterwards catches these in review, once the code exists and
the cost of changing it is a rewrite. Read first and they never enter the diff (D-048).

The rule files carry the stack; §24 carries the project. §24 selects from them, tightens them where
this architecture needs it, and adds what no general stack guide can know — the static-SPA carve-out
(§24.4), the generated-client contract (§24.5), the structural limits (§24.6). Where a rule file and
§24 disagree, §24 wins. Where §24 is silent, the rule file binds on its own. Neither replaces the
other, and reading §24 alone is not sufficient.

Because the frontend is a static export with no JavaScript server runtime in production, a subset of standard SvelteKit guidance is structurally inapplicable here. §24.3 states the full frontend standard as a coherent reference; §24.4 is the authoritative carve-out that says exactly which parts of §24.3 do not apply to Afisharr and why, and what we do instead. Where the two sections conflict, §24.4 wins.

§24.5 states the standards that are genuinely cross-cutting: they constrain both surfaces at once, or they constrain the seam between them. §24.6 states the structural requirement — how the source tree divides, and how large one file may get — which binds every file on both surfaces.

**Structure is a requirement, not a cleanup task.** §24.6 has the same force as every rule below it, and it is checked in the same pass, not in a later refactor. The reason it is named here, ahead of the surface-specific rules: every other rule in §24 is local to a few lines, and a reviewer catches a bad `unwrap()` by reading the diff. Structure is the one property a diff hides. A file grows past its limit one small, correct commit at a time, and no single commit is the one that broke it. So the limit is measured on every change, and a change that pushes a file past it splits the file in the same change (D-047).

We write idiomatic code for each ecosystem on its own terms. We do not import habits from an adjacent ecosystem: not Go/Java-style `Arc<Mutex<…>>`-and-clone reflexes, not Python/TypeScript-style `unwrap()`-as-control-flow or stringly-typed errors, not C++-style inheritance trait-object trees, not Svelte-4-style `export let`/`$:`/`on:click`/slots, not React-style `forwardRef`/`asChild`/`useState`, not Node-style `dotenv`/`ts-node`/`jest`/`bcrypt`/hand-rolled connection pools. Every rule below exists to make the idiomatic, current pattern the only pattern that appears in the codebase.

### 24.2 Rust backend standards

#### 24.2.1 Ownership and borrowing

The single highest-value rule: **accept the least-owned type that works, return the most-owned type you must.**

| Want to accept… | Use | Not |
|---|---|---|
| Read-only text | `&str` | `&String`, `String` |
| Read-only slice | `&[T]` | `&Vec<T>` |
| A path | `impl AsRef<Path>` | `&str`, `PathBuf` |
| Maybe-owned text | `Cow<'_, str>` | always-clone `String` |
| Generic bytes | `impl AsRef<[u8]>` | `Vec<u8>` |

```rust
use std::borrow::Cow;
use std::path::Path;

// Accepts &str, String, Box<str>, ... with zero forced allocation.
fn shout(s: &str) -> String {
    s.to_uppercase()
}

// Cow: only allocate if we actually change something.
fn normalize(input: &str) -> Cow<'_, str> {
    if input.contains(' ') {
        Cow::Owned(input.replace(' ', "_"))
    } else {
        Cow::Borrowed(input)   // no allocation on the common path
    }
}

// impl AsRef<Path>: caller passes "foo.txt", String, PathBuf, Path — all work.
fn read_config(path: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(path.as_ref())
}
```

We reach for `Rc`/`Arc` only when ownership is genuinely shared and lifetimes cannot express it (graphs, shared caches, spawned tasks), and for interior mutability only when we must mutate through a shared reference.

| Need | Single-thread | Multi-thread |
|---|---|---|
| Shared ownership | `Rc<T>` | `Arc<T>` |
| Mutate one value | `Cell<T>` (Copy) / `RefCell<T>` | `Mutex<T>` |
| Many readers, rare writes | `RefCell<T>` | `RwLock<T>` |
| Init once, read forever | `OnceCell` | `OnceLock<T>` |
| Lazy global | `LazyCell` | `LazyLock<T>` |

```rust
use std::sync::LazyLock;
use std::collections::HashMap;

// Lazy global config — no lazy_static!, no once_cell.
static SETTINGS: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    HashMap::from([("retries", 3), ("timeout_ms", 500)])
});

fn retries() -> i32 { SETTINGS["retries"] }
```

`Arc<Mutex<HashMap<K, V>>>` is a code smell we do not write if the map is read-mostly (use `RwLock`, or a sharded/concurrent map such as `dashmap`), or if the lock is only held to hand data to a task (pass an owned clone or a channel instead).

We must not clone to dodge the borrow checker:

```rust
// WRONG: needless allocation to satisfy lifetimes
fn greet(name: String) -> String { format!("hi {name}") }
let n = String::from("ada");
greet(n.clone());  // clone just to keep `n`

// RIGHT: borrow
fn greet(name: &str) -> String { format!("hi {name}") }
greet(&n);         // n still usable
```

We must not reach for `Arc<Mutex<…>>` by reflex on read-mostly shared state:

```rust
// WRONG: serializes all readers behind a Mutex
let cache: Arc<Mutex<HashMap<K, V>>> = ...;

// RIGHT: many concurrent readers with RwLock (or dashmap for high contention)
let cache: Arc<RwLock<HashMap<K, V>>> = ...;
```

We must not take `String`/`&Vec<T>` parameters where a borrow/slice works:

```rust
// WRONG
fn total(v: &Vec<i32>) -> i32 { v.iter().sum() }

// RIGHT: accepts arrays, slices, Vec — everything
fn total(v: &[i32]) -> i32 { v.iter().sum() }
```

#### 24.2.2 Typing and API design

We use **newtypes** for domain invariants, **builders** for many-optional-field construction, **`From`/`TryFrom`** for conversions, generics + `impl Trait` by default, and `dyn Trait` only when we need heterogeneous collections or want to cut monomorphization/compile time.

```rust
// Newtype: makes illegal states unrepresentable and adds no runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl TryFrom<i64> for UserId {
    type Error = &'static str;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        u64::try_from(v).map(UserId).map_err(|_| "user id must be non-negative")
    }
}
```

**`async fn` in traits (native, RPITIT) vs `async-trait`.** Native `async fn` in traits is used directly for application traits. We reach for the `async-trait` crate only when the trait must be **`dyn`-compatible**, because a native async trait method returns an anonymous `impl Future` that is not object-safe.

```rust
// Static dispatch: native async fn in trait — no crate needed.
trait Fetcher {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, std::io::Error>;
}

struct Http;
impl Fetcher for Http {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, std::io::Error> {
        Ok(b"data".to_vec())
    }
}

// Need `Box<dyn Fetcher>`? Then use async_trait, OR return a boxed future:
trait DynFetcher {
    fn fetch<'a>(&'a self, url: &'a str)
        -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<u8>> + Send + 'a>>;
}
```

**Sealed traits** prevent downstream implementations of traits we own but do not want implemented outside the crate:

```rust
mod sealed { pub trait Sealed {} }

pub trait Command: sealed::Sealed {
    fn run(&self);
}

pub struct Ping;
impl sealed::Sealed for Ping {}
impl Command for Ping { fn run(&self) { println!("pong"); } }
// Downstream crates can call Command but cannot implement it.
```

**`let` chains and async closures** are used to flatten nested `if let` + boolean conditions and to write retry/adapter helpers that capture environment and return a future:

```rust
// let chains: flatten nested if-let + boolean conditions with &&
fn describe(v: &Option<Result<i32, String>>) -> &'static str {
    if let Some(Ok(n)) = v && *n > 0 && *n < 100 {
        "small positive"
    } else {
        "other"
    }
}

// async closures: capture environment and return a future (AsyncFn traits)
async fn retry<F>(mut op: F) -> Result<(), String>
where
    F: AsyncFnMut() -> Result<(), String>,
{
    for _ in 0..3 {
        if op().await.is_ok() { return Ok(()); }
    }
    Err("exhausted retries".into())
}
```

We must not build inheritance-style trait-object hierarchies for behavior that could be monomorphized:

```rust
// WRONG (C++ habit): Box<dyn Base> hierarchy for behavior you could monomorphize
fn run(items: Vec<Box<dyn Animal>>) { /* virtual dispatch everywhere */ }

// RIGHT: generics + impl Trait for static dispatch; dyn only for true heterogeneity
fn run<A: Animal>(a: &A) { a.speak(); }
```

**Iterators and pattern matching.** We prefer iterator chains and combinators — they compile to the same code as hand-written loops — and switch to a `for` loop with `?` the moment we have fallible steps or side effects; forcing `Result` through `collect::<Result<Vec<_>, _>>()` is fine, but nested combinators with early exit must not become unreadable.

```rust
use std::collections::HashMap;

// Idiomatic: build a frequency map in one pass, no manual indexing.
fn word_counts(text: &str) -> HashMap<&str, usize> {
    text.split_whitespace().fold(HashMap::new(), |mut acc, w| {
        *acc.entry(w).or_insert(0) += 1;
        acc
    })
}

// let-else: bind-or-diverge, keeps the happy path unindented.
fn parse_port(s: &str) -> Option<u16> {
    let Ok(p) = s.parse::<u16>() else { return None };
    (p >= 1024).then_some(p)
}

// matches! and slice patterns
fn classify(xs: &[i32]) -> &'static str {
    match xs {
        [] => "empty",
        [_] => "one",
        [first, .., last] if first == last => "palindromic ends",
        _ => "many",
    }
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}
```

**Edition 2024 changes that alter daily code and must be followed:**

RPIT in return position now captures all in-scope generic and lifetime parameters automatically. Use `use<..>` only to restrict capture explicitly:

```rust
// 'a is captured automatically — this compiles and is correct
fn first_word<'a>(s: &'a str) -> impl Iterator<Item = &'a str> {
    s.split_whitespace()
}

// Restrict capture explicitly when you must NOT borrow:
fn make_counter<T>(_seed: T) -> impl Iterator<Item = u32> + use<> {
    0..10   // captures nothing; independent of T's lifetime
}
```

Temporaries in an `if let $pat = $expr` scrutinee drop before the `else` branch, not at the end of the statement — we rely on this to avoid lock-guard deadlocks:

```rust
// the lock temporary is dropped before entering `else` — no deadlock
if let Some(v) = shared.lock().unwrap().get(&key).copied() {
    use_value(v);
} else {
    // lock is already released here
    shared.lock().unwrap().insert(key, default());
}
```

Inside an `unsafe fn`, unsafe operations require their own `unsafe {}` block. Unsafe attributes must be wrapped (`#[unsafe(no_mangle)]`, `#[unsafe(export_name = "…")]`). `extern` blocks must be written `unsafe extern "C" { … }`. `static mut` references are hard-denied — use `&raw const`/`&raw mut` or an atomic/`OnceLock`. Any identifier named `gen` must be written `r#gen` (a reserved keyword).

#### 24.2.3 Error handling

**Libraries define typed errors with `thiserror`. Binaries use `anyhow`** for a boxed, context-rich error. We must not `unwrap()`/`expect()` in library code paths; `expect("reason")` is acceptable only for genuine invariants that cannot fail, and the message must state *why* it can't fail.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]                       // allow adding variants without a breaking change
pub enum StoreError {
    #[error("key not found: {0}")]
    NotFound(String),

    #[error("connection failed")]
    Connect(#[from] std::io::Error),    // auto From<io::Error>, keeps source chain

    #[error("invalid record for {key}: {reason}")]
    Invalid { key: String, reason: String },
}

pub fn load(key: &str) -> Result<Vec<u8>, StoreError> {
    let bytes = std::fs::read(key)?;    // io::Error auto-converts via #[from]
    if bytes.is_empty() {
        return Err(StoreError::Invalid { key: key.into(), reason: "empty".into() });
    }
    Ok(bytes)
}
```

Application/binary code uses `anyhow` with `.context()`:

```rust
use anyhow::{Context, Result, bail};

fn run(path: &str) -> Result<()> {
    let cfg = std::fs::read_to_string(path)
        .with_context(|| format!("reading config at {path}"))?;
    if cfg.is_empty() {
        bail!("config {path} is empty");
    }
    Ok(())
}

fn main() -> Result<()> {
    run("app.toml")   // returning Result from main prints the full source chain
}
```

`Box<dyn std::error::Error + Send + Sync>` is acceptable only for the simplest binaries or trait objects where we specifically do not want the `anyhow` dependency; `anyhow::Error` is otherwise strictly better ergonomically (backtraces, context, downcasting).

We must not `unwrap()` in library code:

```rust
// WRONG: panics on the caller's behalf
pub fn parse(s: &str) -> Config { serde_json::from_str(s).unwrap() }

// RIGHT: return a typed error
pub fn parse(s: &str) -> Result<Config, serde_json::Error> { serde_json::from_str(s) }
```

#### 24.2.4 Async discipline

**`tokio` is the default runtime.** We use `#[tokio::main]` for binaries and `#[tokio::test]` for async tests, and only `tokio::sync` primitives inside async code — never a blocking `std::sync::Mutex` held across `.await`. CPU-bound or blocking work must be offloaded with `spawn_blocking`.

```rust
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Structured concurrency: JoinSet owns tasks and cleans them up on drop.
    let mut set = JoinSet::new();
    for id in 0..8u32 {
        set.spawn(async move { work(id).await });
    }
    let mut total = 0u32;
    while let Some(res) = set.join_next().await {
        total += res??;   // first `?` = JoinError, second `?` = task's Result
    }
    println!("total = {total}");
    Ok(())
}

async fn work(id: u32) -> anyhow::Result<u32> { Ok(id * 2) }
```

**Cancellation and `select!` safety.** `tokio::select!` drops the losing futures, so every branch must be cancellation-safe (dropping it mid-flight must not lose committed data). Reading with `AsyncReadExt::read` is cancel-safe; a multi-step "read then write" is not — such state must be hoisted out of the branch. We use `CancellationToken` for cooperative shutdown.

```rust
use tokio_util::sync::CancellationToken;
use tokio::time::{sleep, Duration};

async fn worker(cancel: CancellationToken) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                // clean shutdown path
                break;
            }
            _ = sleep(Duration::from_millis(100)) => {
                // do a unit of cancel-safe work
            }
        }
    }
}
```

```rust
// Blocking or CPU-bound work must NOT run on the async runtime threads.
async fn hash_file(path: String) -> std::io::Result<u64> {
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path)?;          // blocking IO — fine here
        Ok(bytes.iter().map(|&b| b as u64).sum())
    })
    .await
    .expect("spawn_blocking panicked")
}
```

We must not block inside async:

```rust
// WRONG: blocks a runtime worker thread, starves other tasks
async fn load() -> Vec<u8> { std::fs::read("big.bin").unwrap() }

// RIGHT: async IO, or spawn_blocking for unavoidable blocking work
async fn load() -> std::io::Result<Vec<u8>> { tokio::fs::read("big.bin").await }
```

We must not hold a `std::sync::Mutex` guard across `.await`:

```rust
// WRONG: guard is not Send-safe to hold across await points → deadlocks/!Send future
let g = std_mutex.lock().unwrap();
do_async(&*g).await;

// RIGHT: use tokio::sync::Mutex, or drop the guard before awaiting
let data = { let g = std_mutex.lock().unwrap(); g.clone() };
do_async(&data).await;
```

CLIs, batch tools, and CPU-bound programs should stay off async entirely and use plain threads + `rayon` instead — async is not pulled in for a program that makes a handful of blocking calls.

#### 24.2.5 Concurrency (non-async)

**Scoped threads** let us borrow local data across threads without `Arc`, for fork-join over borrowed slices. **`rayon`** is used for data parallelism. We reach for channels (`std::sync::mpsc`, or `crossbeam` for MPMC/select) to pass ownership between threads instead of sharing mutable state.

```rust
use std::thread;

// Scoped threads: borrow `data` directly, no Arc, no clone. Joined at scope end.
fn parallel_sum(data: &[u64]) -> u64 {
    let mid = data.len() / 2;
    let (a, b) = data.split_at(mid);
    thread::scope(|s| {
        let h = s.spawn(|| a.iter().sum::<u64>());
        let right: u64 = b.iter().sum();
        h.join().unwrap() + right
    })
}
```

```rust
use rayon::prelude::*;

// Data parallelism: swap .iter() for .par_iter(). Guaranteed data-race free.
fn sum_squares(v: &[f64]) -> f64 {
    v.par_iter().map(|x| x * x).sum()
}
```

`std`'s `Mutex`/`RwLock` are the default; we reach for `parking_lot` only after profiling shows a hot lock that needs its smaller/faster primitives or features like fair unlocking. For atomics, we default to `Ordering::Relaxed` for counters and `Acquire`/`Release` for lock-free handoff, and use `SeqCst` only when a single total order is genuinely required.

#### 24.2.6 Web services: axum, tower, and API design

**`axum` is the web framework.** Path parameters use **`/{id}`** syntax. We serve with `axum::serve` + a `tokio::net::TcpListener`. Native async traits mean `#[async_trait]` is not needed for extractors.

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState { greeting: String }

#[derive(Serialize)]
struct User { id: u64, name: String }

#[derive(Deserialize)]
struct CreateUser { name: String }

// One error type for the whole API, rendered via IntoResponse.
enum AppError {
    NotFound,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

async fn get_user(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<User>, AppError> {
    if id == 0 { return Err(AppError::NotFound); }
    Ok(Json(User { id, name: format!("{}#{id}", state.greeting) }))
}

async fn create_user(Json(body): Json<CreateUser>) -> (StatusCode, Json<User>) {
    (StatusCode::CREATED, Json(User { id: 1337, name: body.name }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = Arc::new(AppState { greeting: "user".into() });
    let app = Router::new()
        .route("/users/{id}", get(get_user))   // path syntax
        .route("/users", post(create_user))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

`State<T>` must be `Clone` (commonly `Arc<AppState>`); `Json<T>` works both as an extractor (request body → `Deserialize`) and as a response (`Serialize` → JSON). Every route uses **one error type for the whole API**, rendered through `IntoResponse`, so error shape is consistent across every endpoint. Middleware (timeouts, compression, CORS, tracing) comes from `tower`/`tower-http`, layered with `.layer(...)`.

Every handler and DTO that is part of the public API surface must carry the utoipa annotations required to keep the generated OpenAPI schema — and therefore the generated TypeScript client — accurate; a handler is not complete until its schema is correct.

#### 24.2.7 Database access

**`sqlx`** is the database layer: async, compile-time-checked raw SQL, no DSL.

```rust
use sqlx::postgres::PgPoolOptions;

#[derive(sqlx::FromRow)]
struct User { id: i64, name: String }

async fn find(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<Option<User>> {
    // query_as! verifies columns/types against the live DB at compile time.
    sqlx::query_as!(User, "SELECT id, name FROM users WHERE id = $1", id)
        .fetch_optional(pool)
        .await
}

async fn make_pool(url: &str) -> sqlx::Result<sqlx::PgPool> {
    PgPoolOptions::new().max_connections(10).connect(url).await
}
```

On Afisharr the pool and row types are the SQLite equivalents (`SqlitePool`, `SqlitePoolOptions`) rather than the Postgres ones shown above, but the required pattern is identical: every query goes through `query!`/`query_as!` so it is checked against the schema at compile time, never through hand-built SQL strings. We use `sqlx migrate` for versioned migrations, pool the connection once at startup, and share the pool (it is `Clone` and cheap — internally `Arc`) through Axum `State`.

#### 24.2.8 Serialization with serde

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    listen_port: u16,
    #[serde(default)]                       // missing → Default
    verbose: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(flatten)]                        // merge nested fields inline
    extra: Extra,
}

#[derive(Serialize, Deserialize)]
struct Extra { region: String }

// Tagged enum: {"type":"create","id":1}  — great for message protocols.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Create { id: u64 },
    Delete { id: u64 },
}

// Zero-copy: borrow &str straight out of the input buffer (no allocation).
#[derive(Deserialize)]
struct Ref<'a> {
    #[serde(borrow)]
    name: &'a str,
}
```

`serde_json` is the default JSON library; we reach for `simd-json` only when JSON parsing is a proven hot path. We prefer `#[serde(deny_unknown_fields)]` on config/DTO types to catch typos.

#### 24.2.9 Security practices

Default to `#![forbid(unsafe_code)]` at the crate root. When `unsafe` is genuinely required (FFI, a proven-necessary optimization, or a safe abstraction over raw memory), **every `unsafe` block must carry a `// SAFETY:` comment** stating the invariants that make it sound. Under edition 2024, `unsafe fn` bodies still need inner `unsafe {}` blocks.

```rust
use std::mem::MaybeUninit;
use std::ptr::NonNull;

/// Fills a buffer via a C function that writes exactly `len` bytes.
///
/// # Safety
/// `ptr` must be valid for writes of `len` bytes.
unsafe fn fill(ptr: NonNull<u8>, len: usize) {
    // SAFETY: caller guarantees ptr is valid for `len` writes; we write within bounds.
    unsafe {
        std::ptr::write_bytes(ptr.as_ptr(), 0, len);
    }
}

fn zeroed_page() -> [u8; 4096] {
    let mut buf = MaybeUninit::<[u8; 4096]>::uninit();
    // SAFETY: we initialize all 4096 bytes before assuming init.
    unsafe {
        std::ptr::write_bytes(buf.as_mut_ptr() as *mut u8, 0, 4096);
        buf.assume_init()
    }
}
```

FFI code uses `#[repr(C)]` on types crossing the boundary, `unsafe extern "C" { … }` blocks, and `#[unsafe(no_mangle)]` on exported symbols; bindings are generated with `bindgen` (C → Rust) or `cbindgen` (Rust → C headers). Any crate containing `unsafe` must be run under `cargo +nightly miri test` — Miri catches UB that normal tests miss.

Security is also a dependency-supply-chain concern: advisories, licenses, and duplicate/banned dependencies are enforced by `cargo deny` (see §24.2.13), and any dependency that is discontinued or superseded must not be introduced — see the prohibited list in §24.2.13.

#### 24.2.10 Performance practices

- **Pre-size collections:** `Vec::with_capacity(n)` / `HashMap::with_capacity(n)` when the size is known, to avoid reallocation churn.
- **Pass slices, not owned containers:** `&[T]` over `&Vec<T>`, `&str` over `&String`.
- **`Box<[T]>` over `Vec<T>`** when the collection never resizes.
- **`SmallVec`** for collections that are usually tiny, to keep them on the stack.
- **`#[inline]` discipline:** do not sprinkle it; use it only on small cross-crate hot functions.
- **Faster hashing:** for internal, non-adversarial maps, swap the default SipHash hasher for `ahash`/`FxHashMap`/`hashbrown` with a fast hasher.
- **`dyn` to cut compile time:** using `&dyn Trait` at a few call sites in heavily-monomorphized code is a legitimate tradeoff to shrink binary size and compile time.

```rust
use std::collections::HashMap;

fn dedup_sorted(mut v: Vec<u32>) -> Box<[u32]> {
    v.sort_unstable();          // sort_unstable is faster and allocation-free
    v.dedup();
    v.into_boxed_slice()        // no spare capacity retained
}

// Pre-sized, fast-hasher map for internal counting (not attacker-controlled keys).
fn counts(words: &[&str]) -> HashMap<&str, u32> {
    let mut m = HashMap::with_capacity(words.len());
    for &w in words { *m.entry(w).or_insert(0) += 1; }
    m
}
```

The release profile (LTO, one codegen unit, `panic = "abort"` — see §24.2.13) is where most real speedups for the shipping binary come from. We profile with `cargo flamegraph` before optimizing; we do not guess.

#### 24.2.11 Logging and observability

**`tracing`** is structured logging with spans, and replaces `log` for anything involving async or spans (the two interoperate via `tracing-log` where required).

```rust
use tracing::{info, instrument, warn};

#[instrument(skip(db))]                 // auto-creates a span with the args
async fn handle_order(db: &Db, order_id: u64) -> anyhow::Result<()> {
    info!(order_id, "processing order");
    if order_id == 0 {
        warn!("suspicious order id");
    }
    Ok(())
}
```

We initialize the subscriber once at startup with `tracing_subscriber::fmt().with_env_filter(...)` for human logs, or `.json()` for production/aggregation.

#### 24.2.12 Testing

Unit tests live in-module under `#[cfg(test)]`; integration tests go in `tests/`; doc examples in `///` blocks compile and run under `cargo test`. **`cargo nextest`** is the day-to-day runner (faster, better output) — it does not run doctests, so `cargo test --doc` is a separate required step.

```rust
pub fn add(a: i32, b: i32) -> i32 { a + b }

/// Adds two numbers.
///
/// ```
/// assert_eq!(mycrate::add(2, 3), 5);   // doc test — compiled and run
/// ```
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn panics_on_bad_input() {
        panic!("overflow");
    }

    #[tokio::test]
    async fn async_case() {
        assert_eq!(tokio::spawn(async { 21 * 2 }).await.unwrap(), 42);
    }
}
```

| Tool | Use for |
|---|---|
| **cargo-nextest** | Fast parallel test runner |
| **insta** | Snapshot testing (assert against reviewed snapshots) |
| **proptest** | Property-based / generative testing |
| **mockall** | Mocking traits |
| **criterion** | Statistical benchmarks + HTML reports |
| **divan** | Ergonomic benchmarks, easy CI |

```rust
// proptest: assert a property over many generated inputs.
proptest::proptest! {
    #[test]
    fn round_trips(s in ".*") {
        let encoded = s.as_bytes().to_vec();
        proptest::prop_assert_eq!(String::from_utf8(encoded).unwrap(), s);
    }
}
```

```rust
// criterion benchmark in benches/bench.rs (with harness = false in Cargo.toml)
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    c.bench_function("fib20", |b| b.iter(|| fib(black_box(20))));
}
fn fib(n: u64) -> u64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
criterion_group!(benches, bench);
criterion_main!(benches);
```

We prefer trait-based fakes over heavy mocking where possible: define a trait for the dependency, implement a real and a test version. We use `assert_matches!` for asserting on enum variants. `#[bench]`/`test::Bencher` must never be used — it is a hard error on stable; `criterion` or `divan` are the only benchmark harnesses.

#### 24.2.13 Dependency policy

We prefer a **workspace** from day one, even for a single binary, because it makes adding crates, sharing dependency versions, and centralizing lints trivial.

```
myapp/
├── Cargo.toml                 # workspace root
├── Cargo.lock                 # COMMIT for binaries; for libs, do not commit
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── crates/
│   ├── app/                   # binary crate
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── core/                  # library crate (domain logic)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── api/                   # library crate (axum handlers)
│       ├── Cargo.toml
│       └── src/lib.rs
```

Workspace root `Cargo.toml` uses **workspace inheritance** — dependency and lint versions are defined once:

```toml
[workspace]
members = ["crates/*"]
resolver = "3"                 # edition 2024 default resolver

[workspace.package]
edition = "2024"
rust-version = "1.97"          # MSRV, checked by the resolver
license = "MIT OR Apache-2.0"

[workspace.dependencies]
tokio = { version = "1.52", features = ["rt-multi-thread", "macros"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0"
anyhow = "1.0"
axum = "0.8.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
```

Member crate `Cargo.toml` inherits everything:

```toml
[package]
name = "core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true               # opt into all workspace lints
```

A crate that sets `[lints] workspace = true` **cannot also override lints in the same `[lints]` table** — that is a hard error. Per-crate exceptions must be done with crate-level attributes such as `#![allow(clippy::missing_errors_doc)]` in `lib.rs`.

**Feature flags must be additive only** — enabling one must never remove APIs or break another consumer. Mutually-exclusive modes must never be modeled as features.

```toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]          # `dep:` hides the implicit feature
postgres = ["dep:sqlx", "sqlx/postgres"]

[dependencies]
serde = { version = "1.0.229", optional = true }
sqlx = { version = "0.9", optional = true }
```

Release profile:

```toml
[profile.release]
opt-level = 3
lto = "thin"                   # "fat" for max, slower link; "thin" is the sweet spot
codegen-units = 1              # better optimization; slower compile
panic = "abort"                # smaller binaries, no unwinding — only if you never catch_unwind
strip = "symbols"

[profile.dev]
opt-level = 0

# Optimize heavy dependencies even in dev builds (e.g. crypto, parsing):
[profile.dev.package."*"]
opt-level = 2
```

`Cargo.lock` policy: **commit it for binaries/applications** (reproducible builds); **do not commit it for libraries** (let downstream resolve). Dependencies are managed with `cargo add`/`cargo update`; audited with `cargo audit` or `cargo deny check advisories`. `cargo build --timings` and `sccache` are used to diagnose/speed up compile times. `cargo udeps` finds unused deps that `cargo machete` may miss.

**Discontinued or superseded crates must never be introduced:**

- `async-std` — discontinued; use `tokio` (default) or `smol`.
- `lazy_static` / `once_cell` — replaced by `std::sync::LazyLock`/`LazyCell` and `OnceLock`; no crate needed.
- `structopt` — folded into `clap` v4 derive; use `#[derive(Parser)]`.
- `failure` / `error-chain` — dead; use `thiserror` + `anyhow`.
- `#[bench]` / `test::Bencher` — de-stabilized; use `criterion` or `divan`.
- Old `rand` API (`rand::thread_rng().gen_range(...)`) — use the current API:

```rust
// WRONG: deprecated names + reserved keyword collision
let x = rand::thread_rng().gen_range(0..10);

// RIGHT: current rand API: rng(), random_range()
let x = rand::rng().random_range(0..10);
```

#### 24.2.14 Formatting and lint configuration

`rust-toolchain.toml` pins the toolchain for reproducible builds:

```toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy", "rust-src"]
profile = "default"
```

`rustfmt.toml`:

```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Default"
imports_granularity = "Crate"     # nightly-gated formatting options
group_imports = "StdExternalCrate"
```

`clippy.toml`:

```toml
msrv = "1.97"
avoid-breaking-exported-api = false
```

`deny.toml` (license + advisory + source policy):

```toml
[advisories]
yanked = "deny"
[bans]
multiple-versions = "warn"
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-3.0"]
```

The workspace-level clippy lints in §24.2.13 (`all`, `pedantic` at `warn`, `unwrap_used` at `warn`) apply to every crate via `[lints] workspace = true`; `unsafe_code = "forbid"` and `missing_docs = "warn"` are workspace-wide Rust lints.

#### 24.2.15 Module structure, naming, and documentation comments

We use the **no-`mod.rs`** layout: a module `foo` with children lives in `foo.rs` plus a `foo/` directory (never `foo/mod.rs`). Internals stay `pub(crate)`; we expose a curated public surface via re-exports (the facade pattern). Domain invariants are named types (newtypes, §24.2.2), not raw primitives passed around by convention.

```rust
// src/lib.rs
#![warn(missing_docs)]
//! # mycrate
//! A one-line crate summary that appears on docs.rs.

mod store;      // -> src/store.rs
mod api;        // -> src/api.rs (+ src/api/ for children)

// Facade: re-export the public API so users write `mycrate::Store`.
pub use store::Store;

/// A key-value store.
pub struct Store { /* … */ }
```

```rust
// Feature-gated item, documented as gated on docs.rs:
#[cfg(feature = "postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "postgres")))]
pub fn connect_pg() { /* … */ }
```

`#[doc(hidden)]` marks public-but-not-really items (macro helpers). **Every public item must have a doc comment**; `#![warn(missing_docs)]` is enabled at the crate root so the compiler enforces it. Docs are built locally with `cargo doc --no-deps --open`.

`const fn` is used wherever arithmetic, slicing, and control flow allow it in `const` context, and const generics parameterize types by values where that avoids heap allocation:

```rust
// Const generic: a fixed-size ring buffer with no heap allocation.
struct Ring<const N: usize> { buf: [u8; N], head: usize }

impl<const N: usize> Ring<N> {
    const fn new() -> Self { Self { buf: [0; N], head: 0 } }
}

const LOOKUP: [u32; 4] = { let mut a = [0; 4]; a[1] = 1; a };  // const block
```

`no_std` (with `core` + `alloc`) is used only when targeting bare metal or WASM without an allocator; it is not the default posture for this project.

### 24.3 Frontend standards

#### 24.3.1 Component structure

shadcn-svelte components are copied into the repository (`$lib/components/ui`) and edited directly — they are not an external dependency to be upgraded in place. Every such component is runes-native and wraps **Bits UI** primitives. Composition uses the `child` snippet, never `React.forwardRef` or `asChild`.

The required authoring idiom: destructure `ref = $bindable(null)`, rename `class`, spread `...restProps`, forward `bind:ref`, add a `data-slot` attribute, and render `children` with `{@render}`. Styling variants use **`tailwind-variants`** (`tv`), not `class-variance-authority`.

```svelte
<!-- $lib/components/ui/button/button.svelte -->
<script lang="ts" module>
  import { tv, type VariantProps } from "tailwind-variants";
  export const buttonVariants = tv({
    base: "inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors disabled:opacity-50 disabled:pointer-events-none",
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        outline: "border border-input bg-background hover:bg-accent",
        ghost: "hover:bg-accent hover:text-accent-foreground",
      },
      size: { default: "h-9 px-4 py-2", sm: "h-8 px-3", icon: "size-9" },
    },
    defaultVariants: { variant: "default", size: "default" },
  });
  export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
  export type ButtonSize = VariantProps<typeof buttonVariants>["size"];
</script>

<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { cn, type WithElementRef } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    variant = "default",
    size = "default",
    children,
    ...restProps
  }: WithElementRef<HTMLButtonAttributes> & { variant?: ButtonVariant; size?: ButtonSize } = $props();
</script>

<button
  bind:this={ref}
  data-slot="button"
  class={cn(buttonVariants({ variant, size }), className)}
  {...restProps}
>
  {@render children?.()}
</button>
```

Wrapping a Bits UI primitive follows the same shape (note `WithoutChild`, `bind:ref`, `data-slot`):

```svelte
<!-- $lib/components/ui/accordion/accordion-content.svelte -->
<script lang="ts">
  import { Accordion as AccordionPrimitive } from "bits-ui";
  import { cn, type WithoutChild } from "$lib/utils.js";
  let { ref = $bindable(null), class: className, children, ...restProps }:
    WithoutChild<AccordionPrimitive.ContentProps> = $props();
</script>

<AccordionPrimitive.Content
  bind:ref
  data-slot="accordion-content"
  class="overflow-hidden text-sm data-[state=open]:animate-accordion-down data-[state=closed]:animate-accordion-up"
  {...restProps}
>
  <div class={cn("pb-4 pt-0", className)}>{@render children?.()}</div>
</AccordionPrimitive.Content>
```

To render a Bits UI trigger as our own element/component, we use the `child` snippet:

```svelte
<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { buttonVariants } from "$lib/components/ui/button/index.js";
</script>

<Dialog.Root>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <a href="/settings" class={buttonVariants({ variant: "outline" })} {...props}>Open</a>
    {/snippet}
  </Dialog.Trigger>
  <Dialog.Content>...</Dialog.Content>
</Dialog.Root>
```

Compound components are imported as namespaces; icons come from **`@lucide/svelte`** (the scoped package — the unscoped `lucide-svelte` must not be mixed in, since that ships two icon libraries).

```svelte
<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import Settings from "@lucide/svelte/icons/settings";
</script>

<Card.Root>
  <Card.Header><Card.Title>Title</Card.Title></Card.Header>
  <Card.Content>...</Card.Content>
  <Card.Footer><Button><Settings class="mr-2 size-4" />Save</Button></Card.Footer>
</Card.Root>
```

Markup that repeats over data must use snippet props and `{@render}`, never slots:

```svelte
<!-- List.svelte -->
<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  let { items, row, empty }: {
    items: T[];
    row: Snippet<[T]>;         // generics on snippets
    empty?: Snippet;
  } = $props();
</script>

{#if items.length}
  <ul>{#each items as item (item)}<li>{@render row(item)}</li>{/each}</ul>
{:else}
  {@render empty?.()}
{/if}
```

```svelte
<!-- usage -->
<List items={users}>
  {#snippet row(user)}<span>{user.name}</span>{/snippet}
  {#snippet empty()}<p>No users</p>{/snippet}
</List>
```

`{#each}` blocks must always be keyed with `(item.id)` when items can reorder or be removed — unkeyed each blocks reuse DOM by index and cause subtle state bugs.

#### 24.3.2 Svelte 5 runes usage

Runes are compile-time primitives (prefixed `$`), not function calls to import. They work in `.svelte` files and in `.svelte.ts`/`.svelte.js` modules. We must not write Svelte-4 idioms (`export let`, `$:`, `on:click`, slots, `createEventDispatcher`, stores as default state).

```svelte
<script lang="ts">
  let count = $state(0);
  let doubled = $derived(count * 2);                 // pure, recomputed automatically
  let heavy = $derived.by(() => {                    // multi-line derivation
    let total = 0;
    for (let i = 0; i < count; i++) total += i;
    return total;
  });

  // $effect runs AFTER the DOM updates; use for side effects ONLY (not for deriving state)
  $effect(() => {
    document.title = `Count: ${count}`;
    return () => {/* cleanup on destroy / before re-run */};
  });
</script>

<button onclick={() => count++}>{count} → {doubled}</button>
```

**`$derived` is not `$effect`.** A value computed from other state must use `$derived`/`$derived.by`. Using an `$effect` to write to another `$state` (effect-based syncing) creates extra renders and loops and is the most common runes anti-pattern to avoid. A `$derived` value must never be reassigned manually; it is owned by the compiler.

`$state({...})` and `$state([...])` return a deeply reactive Proxy — mutating nested properties or calling `array.push()` triggers updates. `$state.raw(...)` is used when we want a value that only updates on reassignment (large immutable data, external instances). `$state.snapshot(x)` gets a plain, non-proxied clone before passing to `structuredClone`, `JSON.stringify`, or a non-Svelte library.

```ts
let list = $state<{ id: number; done: boolean }[]>([]);
list.push({ id: 1, done: false });   // reactive
list[0].done = true;                  // reactive (deep proxy)

let config = $state.raw({ theme: "dark" });
config = { ...config, theme: "light" }; // only reassignment triggers updates

const plain = $state.snapshot(list);   // detached clone for serialization
```

`$effect.pre` runs before DOM updates; `$effect.root` creates a manually-disposed effect scope outside the component lifecycle; `untrack(fn)` reads state without creating a dependency. `tick()` awaits the DOM flush; `flushSync()` forces it synchronously (mainly in tests).

`$props()` replaces `export let`. We destructure with defaults and rename `class`. `$bindable()` marks a prop as two-way. `$props.id()` generates a hydration-stable unique id.

```svelte
<script lang="ts">
  interface Props {
    title: string;
    count?: number;
    value?: string;          // bindable
    class?: string;
    children?: import("svelte").Snippet;
  }
  let {
    title,
    count = 0,
    value = $bindable(""),
    class: className,
    children,
  }: Props = $props();

  const uid = $props.id();   // stable across SSR/hydration
</script>

<label for={uid}>{title}</label>
<input id={uid} bind:value />
{@render children?.()}
```

Context uses `setContext`/`getContext` and pairs naturally with runes — a `.svelte.ts` state object is put into context to share reactive state down a subtree.

**Attachments (`{@attach}`) replace actions.** They are fully reactive (re-run when read state changes), inline-able, spreadable, and usable on components. Legacy library actions are converted with `fromAction`.

```svelte
<script lang="ts">
  import type { Attachment } from "svelte/attachments";
  import { fromAction } from "svelte/attachments";
  import { tooltip as tooltipAction } from "some-legacy-lib";

  function tooltip(content: string): Attachment {
    return (node) => {
      const t = createTooltip(node, content);   // runs on mount + when content changes
      return () => t.destroy();                  // cleanup
    };
  }
  let text = $state("Hi");
</script>

<button {@attach tooltip(text)}>hover</button>
<div {@attach fromAction(tooltipAction, () => text)}>legacy</div>
```

The `class` attribute accepts objects/arrays, merged with `clsx` under the hood — this is preferred over the legacy `class:` directive. Class props are typed as `ClassValue`.

```svelte
<script lang="ts">
  import type { ClassValue } from "svelte/elements";
  let { class: className }: { class?: ClassValue } = $props();
  let active = $state(false);
</script>

<div class={["card", { active }, className]}>...</div>
```

Components are mounted programmatically with `mount`/`unmount`/`hydrate` from `svelte` — `new Component()` must never be used.

```ts
import { mount, unmount } from "svelte";
import App from "./App.svelte";
const app = mount(App, { target: document.getElementById("app")!, props: { name: "world" } });
// later: unmount(app);
```

Reactivity is debugged with `$inspect(value)` and `$inspect.trace()` inside a function to log why it re-ran; both are stripped in production.

Svelte's experimental `await`-in-components (`experimental.async`) must not be enabled in production code paths. It remains experimental, is coupled to SvelteKit remote functions (§24.3.7), and is not subject to semantic versioning. If used at all it is for prototyping only, clearly marked, with exact versions pinned.

#### 24.3.3 State management

Reactive state outside components — replacing the store pattern for most cases — is a factory or class exporting state via getters, defined in `.svelte.ts`:

```ts
// counter.svelte.ts
export function createCounter(initial = 0) {
  let count = $state(initial);
  const doubled = $derived(count * 2);
  return {
    get count() { return count; },
    get doubled() { return doubled; },
    increment() { count++; },
    reset() { count = initial; },
  };
}
```

Reactive collections must use the drop-in classes from `svelte/reactivity`: `SvelteMap`, `SvelteSet`, `SvelteDate`, `SvelteURL`, and `MediaQuery`. Plain `Map`/`Set`/`Date` are not reactive and must not be used for reactive data.

```ts
import { SvelteMap, MediaQuery } from "svelte/reactivity";
const cache = new SvelteMap<string, number>();
const prefersDark = new MediaQuery("(prefers-color-scheme: dark)");
// prefersDark.current is reactive
```

Page and navigation state is read from **`$app/state`**, not `$app/stores` — the `$page` store form is deprecated and scheduled for removal.

```svelte
<script lang="ts">
  import { page, navigating } from "$app/state";   // NOT $app/stores
</script>
<nav class:active={page.url.pathname === "/"}>...</nav>
{#if navigating.to}<div class="loading-bar" />{/if}
```

Navigation helpers live in `$app/navigation`: `goto`, `invalidate`, `invalidateAll`, `preloadData`, `pushState`/`replaceState` (shallow routing). `depends("app:data")` in `load` is paired with `invalidate("app:data")` to re-run a specific load. Shallow routing with `pushState` + `page.state` powers modals-as-history-entries.

#### 24.3.4 Accessibility

Every input paired with a label must use a hydration-stable id from `$props.id()` rather than a hand-rolled string, so the `for`/`id` association survives SSR and hydration without collisions:

```svelte
<script lang="ts">
  const uid = $props.id();
</script>
<label for={uid}>Title</label>
<input id={uid} />
```

We build interactive widgets (accordions, dialogs, menus) by wrapping Bits UI primitives (§24.3.1) rather than hand-rolling them, because the primitive owns the accessible keyboard/focus/ARIA behavior; a component that reimplements that behavior directly on plain elements is not acceptable.

#### 24.3.5 Styling conventions

**`presetWind4`** is the only UnoCSS preset used — it is Tailwind-v4-compatible, emits oklch colors, and includes its own reset. `presetUno`/`presetWind3` are legacy/superseded names and must not be used. There is no `tailwind.config.js` and no `@tailwind` directive on this stack.

```ts
// uno.config.ts
import { defineConfig, presetWind4, presetIcons, presetWebFonts, transformerVariantGroup } from "unocss";

export default defineConfig({
  presets: [
    presetWind4({
      preflights: { reset: true },   // built-in Tailwind-v4-aligned reset
    }),
    presetIcons({ scale: 1.2, extraProperties: { display: "inline-block", "vertical-align": "middle" } }),
    presetWebFonts({ themeKey: "font", provider: "google", fonts: { sans: "Inter:400,500,600,700" } }),
  ],
  transformers: [transformerVariantGroup()],
  shortcuts: {
    "btn": "px-4 py-2 rounded bg-primary text-white hover:bg-primary/90 disabled:opacity-50",
  },
  theme: {
    colors: { brand: { DEFAULT: "#4f46e5", muted: "#6366f1" } },
  },
  safelist: ["i-lucide-loader-2"],   // classes generated dynamically
});
```

Rules anchored to `presetWind4` that must be followed: `transformerDirectives` (`@apply`, `@screen`) is used with caution — `@screen` breaks because breakpoints moved out of config — so `shortcuts` are preferred over `@apply`. `presetWind4` uses the oklch color model and must not be combined with `presetLegacyCompat` or `presetRemToPx`. `presetWebFonts` uses `themeKey: 'font'` (the old `fontFamily` theme key is unsupported).

We use **global mode** (`unocss/vite`) — not `@unocss/svelte-scoped/vite`, which is reserved for component libraries — because it is required for shadcn-svelte compatibility:

```ts
// vite.config.ts
import { sveltekit } from "@sveltejs/kit/vite";
import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [UnoCSS(), sveltekit()],   // UnoCSS BEFORE sveltekit
});
```

```ts
// src/routes/+layout.svelte  (or app entry)
import "virtual:uno.css";
```

Making shadcn-svelte's Tailwind-authored components work on UnoCSS requires: theme tokens defined as CSS variables in the global stylesheet (`:root` and `.dark`) exactly as shadcn's `init` generates them; `cn()` kept as shipped (it uses `tailwind-merge` + `clsx`, which works because `presetWind4` emits Tailwind-compatible class names); animation keyframes provided via `tw-animate-css` (the successor to the discontinued `tailwindcss-animate`) or a small CSS shim in `uno.config.ts`, since there is no `@plugin` mechanism; dark mode driven by the **class strategy** (`.dark` on `<html>`) via `mode-watcher`, which `presetWind4`'s `dark:` variant keys off; and no `@theme`/`@plugin` directives, since those are Tailwind-v4 CSS-file features — tokens go in `:root`/`.dark` CSS and the rest is configured in `uno.config.ts`.

```css
:root {
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --radius: 0.625rem;
}
.dark {
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);
  --primary: oklch(0.985 0 0);
  --primary-foreground: oklch(0.205 0 0);
}
```

`mode-watcher` sets the theme class **before paint**, avoiding the light→dark flash that setting the class in `onMount` would cause. `<ModeWatcher />` goes in the root layout.

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import "../app.css";
  import { ModeWatcher } from "mode-watcher";
  let { children } = $props();
</script>

<ModeWatcher />
{@render children?.()}
```

```svelte
<!-- theme toggle -->
<script lang="ts">
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import { toggleMode } from "mode-watcher";
  import { Button } from "$lib/components/ui/button/index.js";
</script>

<Button variant="outline" size="icon" onclick={toggleMode}>
  <Sun class="size-5 dark:hidden" />
  <Moon class="hidden size-5 dark:block" />
</Button>
```

**The token values are `tangerine`'s, not shadcn's defaults.** §10.4 states the product requirement;
this is where the values land. The registry item is fetched with
`bunx shadcn@latest add https://tweakcn.com/r/themes/tangerine.json` and its four parts are
distributed as follows:

| Registry field | Where it lands here | Why not where the CLI would put it |
| --- | --- | --- |
| `cssVars.light` (53 tokens) | `:root` in `frontend/src/app.css` | Same place shadcn-svelte's own tokens live |
| `cssVars.dark` (52 tokens) | `.dark` in `frontend/src/app.css` | Class strategy, driven by `mode-watcher` |
| `cssVars.theme` (`font-*`, `radius`, `tracking-*`) | `uno.config.ts` `theme`, plus `--tracking-normal` and the `tracking-*` `calc()` chain in `:root` | There is no `@theme` directive on UnoCSS; it is a Tailwind-v4 CSS-file feature |
| `css["@layer base"]` (`body { letter-spacing }`) | A plain rule in `app.css` | `@layer base` here would be a Tailwind layer we do not have |

**Treat the command as a fetch, not as a build step.** `shadcn` is the React CLI reading a
`components.json` whose schema is not the one this repository carries (`shadcn-svelte.com/schema.json`),
and its Tailwind-v4 output assumes a `@theme inline` block that UnoCSS never processes. What is
authoritative is the JSON's token values, transcribed into the two files above; what is disposable is
whichever CLI put them there. A run that rewrites `app.css` into Tailwind-v4 shape has not applied the
theme, it has broken the stylesheet — revert it and transcribe.

Two consequences carry beyond the token list:

- **Fonts are self-hosted.** Inter, JetBrains Mono, and Source Serif 4 ship as files inside the SPA.
  `presetWebFonts` with `provider: "google"` — the pattern the rule file shows — is not used here,
  because it makes every page load an outbound request to Google carrying the operator's IP (D-038,
  §21.8, D-050).
- **Semantic tokens only.** Components reference `bg-background`, `text-muted-foreground`,
  `border-border`, `bg-primary`, and the rest. A literal color in a component — a hex, an `oklch(…)`,
  or a `bg-orange-500` — is a component that has left the theme and will be wrong in one of the two
  modes. The exceptions are the poster and overlay renderer's own colors, which are content rather
  than interface, and live in the backend.

**The default mode is `system`, and the fallback is light.** `<ModeWatcher />`'s `defaultMode` already
defaults to `"system"` and `track` to `true`, so following the operating system needs no prop. The
fallback does need code: `mode-watcher` resolves the system preference by testing
`(prefers-color-scheme: light)` and mapping every non-match — including a browser with no
`window.matchMedia` at all — to `"dark"`. §10.4 requires light there, so the root layout sets the mode
explicitly when the query cannot be run. It is a few lines, and it is the difference between a
documented default and an accident.

#### 24.3.5.1 Visual design is an obligation, not a taste

**Before an agent writes or reshapes any interface surface, it loads its `frontend-design` skill.**
This applies to a new page, a new component with a visual surface, a layout change, and any work on
the theme itself. It does not apply to a copy fix, a typing fix, or a change with no rendered
consequence.

The reason is narrow and specific. Everything else in §24.3 constrains correctness — runes over
stores, `$derived` over `$effect`, Bits UI over hand-rolled ARIA — and a page can satisfy every line
of it while looking like nothing at all. Left to defaults, an agent reaches for the same centred card
on a neutral background on every page, and the result is not a design that was chosen but a shape
that was defaulted into fifteen times. §4.3 and §8 already say this product's interface has to carry
state, density, and consequence honestly; a templated shell cannot.

What the obligation is *not*: a licence to invent per-page visual languages. The skill informs
typography, hierarchy, density, and restraint within the theme in §10.4 — it does not authorise a
second palette, a page-specific font, or a literal color. Where the skill's advice and this section
collide, this section wins (D-051).

#### 24.3.6 Forms

The reference form stack is **sveltekit-superforms** + **formsnap** + a Standard Schema validator (Zod or Valibot), giving typed, progressively-enhanced forms wired to SvelteKit actions:

```ts
// +page.server.ts
import { superValidate } from "sveltekit-superforms";
import { zod } from "sveltekit-superforms/adapters";
import { z } from "zod";
import { fail } from "@sveltejs/kit";

const schema = z.object({ email: z.string().email(), name: z.string().min(2) });

export const load = async () => ({ form: await superValidate(zod(schema)) });
export const actions = {
  default: async ({ request }) => {
    const form = await superValidate(request, zod(schema));
    if (!form.valid) return fail(400, { form });
    // persist...
    return { form };
  },
};
```

This stack's `load`/`actions` half depends on a SvelteKit server runtime. **§24.4 states that this half does not apply on Afisharr and describes the client-only replacement.** The schema definition (Zod/Valibot object) and the resulting typed `form` shape remain the right way to describe a form's fields regardless of how the submission is transported.

#### 24.3.7 Data fetching and the server boundary

SvelteKit's file conventions in `src/routes/` define what runs where:

| File | Runs | Purpose |
|---|---|---|
| `+page.svelte` | client + SSR | Page component |
| `+page.ts` | client + server | Universal `load` (runs both places) |
| `+page.server.ts` | server only | Server `load` + form `actions`; DB/secrets safe here |
| `+layout.svelte` / `+layout.server.ts` | — | Shared UI + data for a subtree |
| `+server.ts` | server only | API endpoints (`GET`/`POST`/…) returning `Response` |
| `+error.svelte` | client + SSR | Error UI boundary |
| `(group)` | — | Route group (organize without affecting URL) |
| `[param]`, `[...rest]`, `[[optional]]` | — | Dynamic / rest / optional params |

`load` must return serializable data. Universal `load` (`+page.ts`) can return non-serializable values and runs on both sides; server `load` (`+page.server.ts`) runs only on the server and its return is devalue-serialized to the client. Everything is typed with the generated `./$types`.

```ts
// src/routes/blog/[slug]/+page.server.ts
import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import { db } from "$lib/server/db";

export const load: PageServerLoad = async ({ params, locals, setHeaders }) => {
  const post = db.query("SELECT * FROM post WHERE slug = ?").get(params.slug);
  if (!post) error(404, "Not found");            // no `throw` needed
  setHeaders({ "cache-control": "max-age=60" });
  return { post, user: locals.user };
};
```

```svelte
<!-- +page.svelte -->
<script lang="ts">
  import type { PageProps } from "./$types";
  let { data }: PageProps = $props();
</script>
<h1>{data.post.title}</h1>
```

`error()` and `redirect()` are called directly, never thrown. Code that catches genuine errors guards with `isHttpError`/`isRedirect` from `@sveltejs/kit`.

Slow data is streamed by returning a **promise** from a server `load` (top-level keys resolve first, nested promises stream in):

```ts
export const load: PageServerLoad = async () => ({
  fast: await getCriticalData(),
  slow: getSlowData(),          // a promise — streams to the client
});
```

```svelte
{#await data.slow}<Spinner />{:then value}{value}{/await}
```

`src/hooks.server.ts` `handle` runs on every request and populates `event.locals` for auth; `handleFetch` rewrites server-side `fetch`; `handleError` shapes error reporting; the `transport` hook (`hooks.ts`) serializes/deserializes custom types across the server/client boundary.

```ts
// src/hooks.server.ts
import type { Handle } from "@sveltejs/kit";
import { redirect } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
  const sessionId = event.cookies.get("session");
  event.locals.user = sessionId ? await getUser(sessionId) : null;

  if (event.url.pathname.startsWith("/dashboard") && !event.locals.user) {
    redirect(303, "/login");
  }
  return resolve(event);
};
```

```ts
// src/app.d.ts — type your locals
declare global {
  namespace App {
    interface Locals { user: { id: string; email: string } | null; }
  }
}
export {};
```

Environment variables come from four modules:

| Module | Values | Where |
|---|---|---|
| `$env/static/private` | build-time secrets | server only |
| `$env/static/public` | build-time, `PUBLIC_`-prefixed | client + server |
| `$env/dynamic/private` | runtime secrets | server only |
| `$env/dynamic/public` | runtime, `PUBLIC_`-prefixed | client + server |

```ts
import { DATABASE_URL } from "$env/static/private";   // never reaches the client
import { PUBLIC_API_BASE } from "$env/static/public";
```

Server-only code lives under `$lib/server/` — SvelteKit hard-errors if a `$lib/server` module is imported into client code, which is the safety net for DB clients and secrets.

Remote functions (`query`, `form`, `command`, `prerender` from `$app/server`, defined in `.remote.ts` files) let type-safe server functions be called from anywhere. They are **experimental** (require `kit.experimental.remoteFunctions` and `compilerOptions.experimental.async`), every function becomes a public HTTP endpoint (inputs must be validated with a Standard Schema library), and the API has continued to take breaking changes across minor versions. If adopted for an internal feature, the exact SvelteKit version must be pinned; `+server.ts` remains the choice for public/webhook APIs.

```ts
// data.remote.ts  (experimental)
import { query } from "$app/server";
import * as v from "valibot";
import { db } from "$lib/server/db";

export const getPost = query(v.string(), async (slug) => {
  return db.query("SELECT * FROM post WHERE slug = ?").get(slug);
});
```

```js
// svelte.config.js
import adapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    alias: { $lib: "src/lib" },
    // CSP example
    csp: { directives: { "script-src": ["self"] } },
  },
};
```

Adapter options:

| Adapter | Status | Use when |
|---|---|---|
| `@sveltejs/adapter-auto` | official | Zero-config deploys to supported platforms |
| `@sveltejs/adapter-node` | official | Self-hosted; run the output with `bun ./build/index.js` |
| `@sveltejs/adapter-static` | official | Fully prerendered SPA/SSG |
| `svelte-adapter-bun` | community | Standalone `Bun.serve()` server, native WS, precompression |

Server-side database access under a server runtime uses Bun's native drivers — `bun:sqlite` for SQLite:

```ts
// src/lib/server/db.ts
import { Database } from "bun:sqlite";
export const db = new Database("app.sqlite", { strict: true });
db.run("PRAGMA journal_mode = WAL;");
```

```ts
// usage in a load
const posts = db.query("SELECT * FROM post ORDER BY created_at DESC LIMIT ?").all(20);
const post  = db.query("SELECT * FROM post WHERE id = $id").get({ id });
```

or `Bun.sql` tagged templates (auto-parameterized, injection-safe) for Postgres:

```ts
import { sql } from "bun";
const users = await sql`SELECT * FROM users WHERE active = ${true} LIMIT ${10}`;
```

**§24.4 states which parts of this section apply on Afisharr and which do not.**

#### 24.3.8 Error boundaries

`<svelte:boundary>` catches errors in its subtree with `failed` and `pending` snippets:

```svelte
<svelte:boundary>
  <RiskyComponent />
  {#snippet failed(error, reset)}
    <p>Something broke: {error.message}</p>
    <button onclick={reset}>Retry</button>
  {/snippet}
</svelte:boundary>
```

#### 24.3.9 Performance

`$derived`/`$derived.by` must be used instead of effect-based syncing (§24.3.2) — beyond correctness, effect-based syncing causes extra render passes that a pure derivation avoids. Slow data must be streamed from `load` as a promise rather than blocking the whole page on the slowest query (§24.3.7). Reactive collections (`SvelteMap`/`SvelteSet`, §24.3.3) are used instead of a plain collection plus a manually triggered re-render. `{#each}` blocks over data that can reorder must be keyed (§24.3.1) — an unkeyed list forces the runtime to reconcile by index instead of by identity. `$state.raw` (§24.3.2) is used for large or external data that is only ever replaced wholesale, to avoid the cost of deep-proxying data that never needs fine-grained reactivity.

#### 24.3.10 Testing

Split tests by what they exercise:

| Test target | Tool |
|---|---|
| Pure `.ts` utils, `.svelte.ts` state classes | `bun test` (native, fast) |
| `.svelte` component rendering | **Vitest + `vitest-browser-svelte`** (real browser via Playwright) |
| End-to-end | Playwright (`@playwright/test`) |

`bun test` cannot compile `.svelte` component files and SvelteKit's `$app/*` modules fail under it — component tests must go through Vitest browser mode, never `bun test`.

```ts
// test-setup.ts
import { GlobalRegistrator } from "@happy-dom/global-registrator";
GlobalRegistrator.register();
```

```ts
// counter.svelte.test.ts  — testing a .svelte.ts state module with bun test
import { test, expect } from "bun:test";
import { createCounter } from "./counter.svelte.ts";

test("counter increments", () => {
  const c = createCounter(0);
  c.increment();
  expect(c.count).toBe(1);
});
```

`bun:test` also provides `mock()`, `spyOn()`, `mock.module()`, and snapshots (`toMatchSnapshot`, `toMatchInlineSnapshot`); snapshots are updated with `bun test -u`.

```ts
// vitest.config.ts
import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    setupFiles: ["vitest-browser-svelte"],
    browser: { enabled: true, provider: "playwright", instances: [{ browser: "chromium" }] },
  },
});
```

```ts
// Button.svelte.test.ts
import { render } from "vitest-browser-svelte";
import { expect, test } from "vitest";
import Button from "./Button.svelte";

test("button click increments", async () => {
  const screen = render(Button, { initialCount: 1 });
  await screen.getByRole("button").click();
  await expect.element(screen.getByText("Count is 2")).toBeVisible();
});
```

Type-checking is wired to `svelte-kit sync && svelte-check --tsconfig ./tsconfig.json`, invoked through `bun run check`.

#### 24.3.11 Typing

Props are typed with an explicit `Props`/inline type on every component, never left implicit. Class props are typed as `ClassValue` (§24.3.2). Snippet props are typed with `Snippet`/`Snippet<[T]>`, including generics on the component itself (`<script lang="ts" generics="T">`, §24.3.1). Structural helper types ship alongside `cn()`:

```ts
// src/lib/utils.ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// biome-ignore lint/suspicious/noExplicitAny: structural probe, the shape is irrelevant
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// biome-ignore lint/suspicious/noExplicitAny: structural probe, the shape is irrelevant
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
```

`tsconfig.json` runs in `strict` mode and extends the SvelteKit-generated config:

```jsonc
// tsconfig.json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

Type-checking (`svelte-check`) is a required, separate gate from linting/formatting (§24.3.10) — Biome does not type-check.

#### 24.3.12 Tooling: Bun, Biome, TypeScript

Bun is the runtime, package manager, test runner, and bundler for the frontend. We do not add `dotenv` (Bun reads `.env` automatically), `ts-node`/`tsx` (Bun runs `.ts` directly), `jest`/`ts-jest` (use `bun:test`), `bcrypt` (use `Bun.password`), or `nodemon` (use `bun --hot`).

Bun's text lockfile `bun.lock` (JSONC) is committed; any legacy binary `bun.lockb` must be removed. `bunx` (not `npx`) executes package binaries.

```bash
bun install                 # install; writes/updates bun.lock
bun add bits-ui @lucide/svelte
bun add -d vitest @sveltejs/adapter-node
bun install --frozen-lockfile   # CI: fail if lockfile would change
bunx shadcn-svelte@latest add button
```

```toml
# bunfig.toml
[install]
# The npm registry; use exact versions in production apps
registry = "https://registry.npmjs.org/"
exact = true

[install.scopes]
# private packages, if any
"@mycompany" = { url = "https://npm.mycompany.com/", token = "$NPM_TOKEN" }

[test]
# Register happy-dom (or other globals) before tests run
preload = ["./test-setup.ts"]
coverage = true
coverageReporter = ["text", "lcov"]
# Single number applies to lines/functions/statements; object sets each
coverageThreshold = { lines = 0.85, functions = 0.90, statements = 0.80 }
```

Monorepos use Bun **workspaces** (`package.json` `workspaces`) and **catalogs** to pin shared dependency versions in one place, and `--filter` to run scripts across packages.

Running SvelteKit's Vite dev server under the Bun runtime (rather than Node, which is what a bare `bun run dev` gives) requires the `--bun` flag:

```bash
bun --bun run dev     # Vite dev server runs on the Bun runtime
bun run dev           # Node runtime (Bun only launches the script)
```

Bun-native APIs replace their Node/third-party equivalents wherever applicable:

```ts
// Password hashing — Bun.password.hash() uses the Argon2id algorithm by
// default (NOT bcrypt; no dependency needed). Output is PHC format: $argon2id$v=19$...
const hash = await Bun.password.hash(plaintext);
const ok   = await Bun.password.verify(plaintext, hash);  // algorithm auto-detected from hash

// Files
const file = Bun.file("./data.json");
const json = await file.json();
await Bun.write("./out.txt", "hello");

// Env — no dotenv; .env / .env.local read automatically
const port = Bun.env.PORT ?? "3000";

// Shell — cross-platform, auto-escaped interpolation
import { $ } from "bun";
const branch = await $`git rev-parse --abbrev-ref HEAD`.text();
```

**Biome replaces both ESLint and Prettier** — one binary, one config file, one pass over the tree. It is scaffolded and added explicitly; the `eslint`/`prettier` add-ons are never taken.

```sh
bun add -D --exact @biomejs/biome
bunx biome init
```

Three commands cover the whole workflow:

| Command | What it does |
|---|---|
| `biome ci .` | Checks format, lint, and assist actions. Writes nothing. Use in pre-commit and CI. |
| `biome check --write .` | Same checks, applies every safe fix. Use while developing. |
| `biome format --write .` | Formats only. |

A rule suppression must be inline with `// biome-ignore lint/<group>/<rule>: <reason>` — the reason is mandatory; a bare ignore is rejected.

Biome parses `.svelte` files natively. Two configuration caveats govern this: without `html.experimentalFullSupportEnabled`, Biome touches only the `<script>` and `<style>` blocks and leaves template markup alone — turning it on formats the markup too, but that support is experimental. Biome does not type-check, so `svelte-check` stays in the toolchain and in the pre-commit gate regardless.

```jsonc
// biome.json
{
  "$schema": "https://biomejs.dev/schemas/2.5.7/schema.json",
  "vcs": { "enabled": true, "clientKind": "git", "useIgnoreFile": true },
  "files": { "includes": ["**", "!build", "!.svelte-kit", "!dist"] },
  "formatter": { "enabled": true, "indentStyle": "tab" },
  "linter": { "enabled": true, "rules": { "preset": "recommended" } },
  "assist": { "actions": { "source": { "organizeImports": "on" } } },
  "javascript": {
    "formatter": { "quoteStyle": "single", "trailingCommas": "all" }
  },
  // Formats the Svelte template markup, not just <script> and <style>.
  // Experimental — drop this line if the markup output surprises you.
  "html": { "experimentalFullSupportEnabled": true }
}
```

Biome's version is pinned exactly (`--exact`), because it ships formatter changes in minor releases and a floating range would reformat the whole tree on an unrelated `bun install`.

`package.json` scripts standardize the workflow:

```jsonc
// package.json (scripts)
{
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "test:unit": "bun test",
    "test:browser": "vitest run",
    "lint": "biome ci .",
    "format": "biome format --write ."
  }
}
```

#### 24.3.13 Frontend anti-patterns

| Wrong (must not write) | Right (write instead) | Why |
|---|---|---|
| `export let count;` | `let { count } = $props();` | Svelte 4 props are gone in runes mode |
| `$: doubled = count * 2;` | `let doubled = $derived(count * 2);` | `$:` is Svelte 4; not reactive in runes |
| `on:click={fn}` | `onclick={fn}` | Event handlers are plain props now |
| `$effect(() => { doubled = count * 2; })` | `let doubled = $derived(...)` | Effect-based syncing loops; derive instead |
| Mutating a `$derived` value | Derive from source; mutate the source | Deriveds are compiler-owned |
| `createEventDispatcher()` | Callback props (`onsave={...}`) | Removed in Svelte 5 |
| `<slot />` | Snippet props + `{@render children?.()}` | Slots replaced by snippets |
| `new Component({ target })` | `mount(Component, { target })` | Class instantiation removed |
| Plain `Map`/`Set`/`Date` for reactive data | `SvelteMap`/`SvelteSet`/`SvelteDate` | Native ones aren't reactive |
| `import { page } from "$app/stores"` (`$page`) | `import { page } from "$app/state"` | Stores deprecated |
| `throw error(404)` / `throw redirect(303, …)` | `error(404)` / `redirect(303, …)` | No `throw` needed |
| Secrets in `+page.ts` or `$lib` | `$env/static/private` in `+page.server.ts` / `$lib/server` | Universal/client code ships to browser |
| `import "dotenv/config"` | `Bun.env` / `$env/*` | Bun reads `.env` automatically |
| `bcrypt` package | `Bun.password.hash` (argon2id) | Native, no dependency |
| `presetUno` / `presetWind3` | `presetWind4` | Legacy/superseded preset names |
| `tailwind.config.js` + `@tailwind` | `uno.config.ts` | Tailwind config doesn't apply to UnoCSS |
| `use:action` for new element behavior | `{@attach ...}` | Actions superseded by attachments |
| `asChild` prop (React) | `child` snippet | Bits UI uses the `child` snippet |
| `React.forwardRef` | `ref = $bindable(null)` + `bind:this` | React idiom; not Svelte |
| `class-variance-authority` | `tailwind-variants` (`tv`) | Current shadcn-svelte variant tool |
| `lucide-svelte` mixed with scoped | `@lucide/svelte` | Match the scoped package to avoid dupes |
| `jest` / `ts-jest` | `bun test` + `vitest-browser-svelte` | Bun native + browser-mode components |
| `bun test` on `.svelte` components | Vitest browser mode | `bun test` can't compile `.svelte` |
| enabling `experimental.async` in prod | Keep it opt-in for prototypes | Not semver-protected; may break |

### 24.4 The static-SPA exception

Afisharr's frontend builds with `@sveltejs/adapter-static`, fully prerendered, and the output is embedded into the Rust binary. **There is no JavaScript server runtime in production.** Every request that would, on a standard SvelteKit deployment, be handled by server-side code executing per-request is instead either baked in at build time or must go over the network to the Rust API. This section is authoritative over §24.3 wherever the two disagree.

The following categories of §24.3 rules **do not apply** and must not be used:

1. **Server `load` functions that depend on per-request data** (`+page.server.ts`/`+layout.server.ts` reading `params`, `locals`, `cookies`, `request`, or calling `setHeaders`). Prerendering runs these once at build time; there is no live request at runtime for them to read. Any data a page needs must instead be fetched client-side against the Rust API using the generated typed client, from a universal `load` (`+page.ts`) or directly from component code with `$effect`/`$state`.
2. **Form actions** (the `actions` export in `+page.server.ts`, and `use:enhance` posting to `?/action`). `adapter-static` does not support actions — a build with them present fails. Mutations are submitted as client-side `fetch` calls through the generated API client, with the response handled in the component.
3. **Server hooks** (`src/hooks.server.ts`: `handle`, `handleFetch`, `handleError`, and the `transport` hook). None of these execute in a static build; there is no server request pipeline at runtime. Auth state, redirects, and error shaping happen client-side against responses from the Rust API.
4. **Server-side form validation**, meaning the `load`/`actions` half of the superforms stack in §24.3.6 that runs `superValidate` against an incoming `request` on the server. Schemas (Zod/Valibot) are still the right way to describe a form's shape, but validation runs client-side before the typed client call, and the Rust API is the final authority on validity — it must independently validate every request body regardless of what the client already checked.
5. **Direct database access from the frontend** (`bun:sqlite`, `Bun.sql`, anything under `$lib/server/db`). There is no frontend server process to hold a database connection. SQLite is reachable only from inside the Rust binary, behind the generated API client.

The following are direct corollaries of the same restriction and also do not apply in production:

- **`$env/dynamic/private`, `$env/dynamic/public`, `$env/static/private`** — there is no server runtime to hold a runtime secret, and no per-request dynamic environment to read. Only `$env/static/public` is meaningful, and its values are baked into the static bundle at build time — they are not secrets and must not be treated as such.
- **Remote functions** (`query`, `form`, `command`, `prerender` from `$app/server`, `.remote.ts` files) — they require a server runtime and are experimental regardless; neither condition holds here.
- **Adapters other than `adapter-static`** (`adapter-node`, `adapter-auto`, `svelte-adapter-bun`) and the "run the build output under Bun" guidance that goes with them — the project always builds with `adapter-static`, and the production binary that serves the assets is the Rust binary, not a Bun process.
- **Streaming a promise from a server `load`** — there is no server round trip at request time to stream from; any streaming/loading-state UX is implemented client-side around the `fetch` call to the Rust API (e.g. a pending state while `await`ing the typed client call).

What replaces all of the above, uniformly: **every dynamic read and write goes through a client-side `fetch` call against the Rust API, using the client utoipa generates from the OpenAPI schema.** Universal `load` functions (`+page.ts`, no `.server.ts`) remain available and prerender-safe as long as they only call the generated client rather than touching a database or reading request-only inputs; component-level `$effect`/`$state`-driven fetching is equally acceptable. SSE from the backend is consumed with the standard client-side `EventSource`/`fetch`-stream APIs, not through any SvelteKit server primitive.

### 24.5 Cross-cutting standards

**The generated OpenAPI client is the sole contract between the two surfaces.** The Rust backend's utoipa annotations are the source of truth; the TypeScript client generated from them is the only way the frontend is allowed to call the API. Hand-written `fetch` calls with ad hoc URL strings and untyped response shapes must not be introduced — if the client doesn't have the shape needed, the backend's utoipa-annotated handler is fixed first and the client regenerated, not worked around.

**Neither surface trusts the other for validation.** The Rust API validates every request body and parameter independently of whatever the frontend already checked (§24.4, point 4) — client-side validation is a UX convenience, never a security boundary.

**We do not import habits from an adjacent ecosystem on either surface** (§24.1). This is enforced the same way on both: anti-pattern tables (§24.2's inline wrong/right pairs and §24.3.13) are treated as a checklist during review, not background reading.

**One formatter, one linter, per surface.** Rust code is formatted by `rustfmt` and linted by `clippy`, configured exactly as in §24.2.14; frontend code is formatted and linted by Biome, configured exactly as in §24.3.12. No other formatter or linter is introduced for either surface, and no file is exempted from the applicable tool without a documented reason.

**Type-checking is a separate, mandatory gate from lint/format on both surfaces.** `cargo check`/`clippy` on the Rust side and `svelte-check` on the frontend side (§24.3.11) must both pass; a green Biome or rustfmt run is not sufficient on its own.

**Every public item is documented.** Rust public items carry doc comments enforced by `#![warn(missing_docs)]` (§24.2.15); frontend components carry explicit `Props` types (§24.3.11) that serve the same self-documenting role since there is no separate doc-comment convention for `.svelte` files.

**Dependencies are pinned and audited on both surfaces.** Rust dependencies go through `cargo deny`/`cargo machete`/`cargo audit` (§24.2.13); frontend dependencies are locked via the committed `bun.lock` and installed with `--frozen-lockfile` in CI (§24.3.12). Tool versions that ship formatting/linting changes in minor releases (Biome) are pinned exactly.

**Testing is stratified by what is being verified, on both surfaces, and no single tool is asked to cover a target it is not built for:** unit/property/integration/doctest/benchmark tools on the Rust side (§24.2.12) map to unit/component/browser/e2e tools on the frontend side (§24.3.10) — `bun test` no more compiles a `.svelte` component than a Rust unit test replaces a `criterion` benchmark.

### 24.6 Modular structure (normative, both surfaces)

This section is the structural requirement named in §24.1. It applies to every file in the
repository, on both surfaces, and it is checked on every change rather than in a periodic cleanup.
Recorded as D-047.

Five rules. They are one idea seen from five angles: a reader who opens one file should get one
subject, and a reader who opens one folder should get one domain.

#### 24.6.1 The source tree divides by feature or domain

Every crate's `src/` divides into subfolders named after a domain — a thing the product has, or a
job the product does. A flat `src/` holding twenty sibling files is not acceptable at any size, and
neither is a folder named after a layer.

| Divide by | Not by |
|---|---|
| `placement/`, `lifecycle/`, `render/`, `sources/trakt/` | `utils/`, `helpers/`, `common/`, `misc/`, `shared/` |
| `definition/validation/` | `types/`, `models/`, `structs/`, `traits/`, `impls/` |
| `collections/reconcile/` | `services/`, `managers/`, `handlers/` as a single catch-all |

A layer name describes the shape of what is inside, so anything of that shape qualifies, so
everything of that shape arrives. `utils/` is a god folder with the same failure mode as a god file:
nothing is ever the wrong thing to put in it. The prohibited names above are prohibited as
*catch-alls*, not as words — `backend/crates/api/src/routes/` is a domain (the HTTP surface), while a
`backend/crates/core/src/services/` holding six unrelated subsystems is not. Likewise
`backend/crates/sources/src/trakt/types.rs`, holding the Trakt DTOs and nothing else, is a
single-purpose file inside a domain; the rule bans `types/` as a way to *divide a crate*, not the
word.

**Where genuinely shared code goes.** This rule bans a name, not code reuse. Two domains that need
the same function still share it — under a name that predicts what it does:

| Instead of | Write |
|---|---|
| `utils.rs` holding `slugify`, `retry`, `parse_duration` | `text/slug.rs`, `net/retry.rs`, `time/duration.rs` |
| `helpers.ts` holding `formatDate`, `debounce` | `format/date.ts`, `interaction/debounce.ts` |

The shared layer is `backend/crates/core/src/<named>/` on the backend and
`frontend/src/lib/shared/<named>/` on the frontend. Each `<named>` is a domain in its own right —
`text`, `time`, `format` — and is subject to every rule in §24.6 like any other. The test is one
question: **does the folder name predict what is inside it?** `text/slug.rs` predicts. `utils.rs`
does not, which is why everything ends up there.

A helper used by exactly one domain does not go in the shared layer at all. It lives beside the code
it serves, and it moves outward the first time a second domain needs it.

Frontend structure follows the same rule against SvelteKit's own layout: routes stay in
`src/routes/` as the framework requires, and everything they use lives in `src/lib/features/<domain>/`
holding that domain's components, its `.svelte.ts` state, and its calls to the generated API client
together. `src/lib/components/ui/` remains the shared primitive layer (§24.3.1) and holds nothing
domain-specific.

Where a domain genuinely spans crates, the crate boundary is the division and the folder names match
across it: `backend/crates/core/src/placement/` and `backend/crates/api/src/routes/placement/` are
the same domain seen from two surfaces, and they carry the same name for that reason.

#### 24.6.2 One file states one thing

Every file has a single responsibility. The check is a sentence: name what the file is for, in one
sentence, without the word "and". If the honest sentence needs an "and", the file is two files and
the split is already obvious — the "and" is the seam.

One file holds one of: one aggregate and its invariants, one pipeline stage, one source adapter, one
route group, one state machine, one component. It does not hold two of them because they are both
small, and it does not hold two of them because they are both about the same feature — that is what
the folder is for.

#### 24.6.3 No god files

A god file is any module that accumulates unrelated responsibilities. It is prohibited outright, at
any line count, and it is the failure this section exists to prevent. In particular:

- No `utils.rs`, `helpers.ts`, `common.rs`, `misc.rs`, or `shared.ts` — a name that means "things"
  admits everything. A helper that is used twice belongs beside the thing it helps; a helper used by
  four domains is its own named module in the shared layer, describing what it does (§24.6.1).
- No `types.rs`/`models.ts` holding every type in a crate. A type lives with the code that owns its
  invariants.
- No `mod.rs`/`index.ts` carrying implementation. Those files declare and re-export (§24.6.5); logic
  in them is logic with no name and no home.
- No single `AppState`-adjacent module that grows a method per feature. Each feature owns its own
  state and exposes an interface to the wiring layer.
- No file that both defines a domain concept and performs I/O for it. Domain logic and its transport
  are separate files, which is also what makes the domain logic testable without the transport.

Size does not make a file a god file, and smallness does not exempt one. A 90-line module holding
four unrelated helpers is already the thing this rule prohibits; it is just early.

#### 24.6.4 The file-size limits, soft and hard

Every file carries a soft limit and a hard limit, measured in physical lines including blanks and
comments:

| File | Soft | Hard |
|---|---|---|
| Rust `.rs`, non-test | 400 | 700 |
| Rust test file (`tests/*.rs`, `#[cfg(test)]`-only file) | 600 | — |
| `.svelte` component | 250 | 400 |
| `.ts`, `.svelte.ts` | 300 | 500 |

**At the soft limit**, split the file, or state in the change description why this one should not be
split. One sentence is enough, and "we ran out of time" is not that sentence. This is the deviation
mechanism §24.1 already grants to "should" rules, made explicit.

**At the hard limit**, split the file, or take the exception in front of a second person. The
exception costs two signatures and leaves a record in the file:

1. The author writes a header comment on the file naming the category and why the split is worse —
   `// STRUCTURE: over 700 lines. One state machine; a split separates the guards from the`
   `// transitions they guard (§24.6.4).`
2. A reviewer who is not the author agrees, in the change that introduces the comment.

The comment stays in the file, so the next reader finds the decision already made instead of
reopening it, and the next reviewer can disagree with a decision that is written down.

Cases where the exception is the right answer, and the split is the worse code: a state machine whose
transitions, guards, and allowlist read as one table (§17.8); an exhaustive `match` over a large
enum, where splitting it hides the exhaustiveness the compiler is proving; one editor component whose
sub-parts would share a dozen `$bindable` values and turn into prop-threading (§7.5). What these have
in common is that the file is already one thing under §24.6.2 — the exception exists for files that
are big, never for files that are two files.

An absolute ceiling was considered and rejected, for the reason D-047 gives for two thresholds at
all: a rule with no exception gets overridden anyway, in a PR comment, off the record, with nothing
left in the file. Two signatures and a header comment is the same override made expensive and
visible.

Exempt from both limits, because their length carries no complexity and splitting them would hide
the fact that they are one table:

- Generated code: the OpenAPI TypeScript client, `.svelte-kit/`, `target/`, SQLx offline data.
- The four registries (§13.2–§13.6) and other pure constant tables — a file of data literals with no
  branching. A registry file that grows a `match` arm with logic in it stops being exempt.
- SQL migration files (§19.3), which are append-only by construction and must not be rewritten.

The limits are deliberately generous. They are a backstop that catches drift, not the primary
control — a file can violate §24.6.2 and §24.6.3 at 120 lines, and those rules bind first. Passing
`wc -l` is not evidence of a well-structured module.

#### 24.6.5 Module boundaries are explicit and narrow

Every module exposes the smallest public surface that its callers actually need, and that surface is
written down in one place rather than emerging from whatever happened to be marked public.

**Rust.** A parent declares children with private `mod x;` and re-exports the intended surface with
`pub use x::{Thing, other_thing};`. `pub mod` is for a surface a caller is meant to navigate into,
not the default. Inside a crate, `pub(crate)` is the default for anything the crate's own code
shares; `pub` means "part of this crate's contract with the workspace" and nothing less. A crate's
`lib.rs` is its entire public surface and reads as a list of what it exports — a reader answers
"what can I call?" from that one file. A caller reaching three module levels into another crate is a
boundary that was never drawn.

**Frontend.** Each `src/lib/features/<domain>/` has one `index.ts` naming its exports. Other
features import from that barrel and never from a deep path inside it, so a domain's internals stay
free to move. Components that exist only to serve one parent live in that feature's folder and are
not exported from its barrel.

**Both.** A module's public surface is part of its diff. Widening one — adding a `pub`, adding a
barrel export — is a reviewable decision with a reason, not a side effect of needing the symbol
somewhere else. The usual right answer to "I need this private thing" is that the caller belongs
inside the boundary, or that the boundary is wrong and should be redrawn deliberately.

#### 24.6.6 How this is checked

Structure is checked in review, backed by two commands that make drift visible. The implementation
plan's Appendix A carries them as build gates (§A.1) and as checklist lines (§A.2, §A.3, §A.4).

```bash
# Rust files over the soft limit, worst first
find backend/crates -name '*.rs' -not -path '*/target/*' -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 400' | sort -rn

# Frontend files over their soft limits
find frontend/src -name '*.svelte' -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 250' | sort -rn
find frontend/src \( -name '*.ts' -o -name '*.svelte.ts' \) -print0 \
  | xargs -0 wc -l | awk '$2 != "total" && $1 > 300' | sort -rn
```

The commands find only §24.6.4. §24.6.1, §24.6.2, §24.6.3, and §24.6.5 are read by a human, which is
why they are stated as sentences a reviewer can apply rather than thresholds a script can measure.
