-- SPDX-FileCopyrightText: 2026 Afisharr contributors
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- 0002 — the complete schema. Sixty-eight STRICT tables and their indexes,
-- exactly as PRD §19 specifies them, plus the three seeded principal rows.
--
-- SQLite DDL is transactional and sqlx wraps each migration file in one
-- transaction, so a failure part-way through leaves nothing behind.

-- ---------------------------------------------------------------------------
-- Instance, settings, secrets, and versioned policy (PRD §19.5)
-- ---------------------------------------------------------------------------

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
CREATE TABLE secrets (
    name        TEXT PRIMARY KEY,               -- 'plex.token', 'tmdb.apiKey', 'trakt.refresh'
    ciphertext  BLOB    NOT NULL,
    nonce       BLOB    NOT NULL,
    algorithm   TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    last_used_at INTEGER
) STRICT;

-- ---------------------------------------------------------------------------
-- Concurrency (PRD §19.4)
-- ---------------------------------------------------------------------------

CREATE TABLE leases (
    name           TEXT    PRIMARY KEY,        -- 'pass:placement:lib_01J9Z…', 'job:overlay-sweep'
    owner          TEXT    NOT NULL,           -- process instance id + task id
    acquired_at    INTEGER NOT NULL,
    expires_at     INTEGER NOT NULL,
    heartbeat_at   INTEGER NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Versioned policy and registry (PRD §19.5)
-- ---------------------------------------------------------------------------

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
-- ---------------------------------------------------------------------------
-- Identity and access (PRD §19.6)
-- ---------------------------------------------------------------------------

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
-- ---------------------------------------------------------------------------
-- Plex topology and the library cache (PRD §19.7)
-- ---------------------------------------------------------------------------

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
CREATE TABLE library_item_ids (
    library_item_id TEXT NOT NULL REFERENCES library_items(id) ON DELETE CASCADE,
    id_space        TEXT NOT NULL,                   -- 'tmdb','tvdb','imdb','anidb','mal','anilist','plex'
    id_value        TEXT NOT NULL,
    source          TEXT NOT NULL,                   -- 'plexGuid','agent','mapping','manual'
    recorded_at     INTEGER NOT NULL,
    PRIMARY KEY (library_item_id, id_space, id_value)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_library_item_ids__lookup ON library_item_ids(id_space, id_value);
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
CREATE TABLE id_mappings (
    from_space   TEXT NOT NULL,
    from_value   TEXT NOT NULL,
    to_space     TEXT NOT NULL,
    to_value     TEXT NOT NULL,
    -- `-1` is the whole title, matching the sentinel
    -- `ux_lifecycle_subjects__identity` uses. Declared rather than nullable:
    -- every PRIMARY KEY column of a STRICT, WITHOUT ROWID table is implicitly
    -- NOT NULL, so a nullable `season` advertised a NULL the table refuses and
    -- left "not season-scoped" with no value it would accept.
    season       INTEGER NOT NULL DEFAULT -1,
    dataset      TEXT NOT NULL,                    -- which mapping dataset supplied this
    imported_at  INTEGER NOT NULL,
    PRIMARY KEY (from_space, from_value, to_space, season)
) STRICT, WITHOUT ROWID;
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
-- ---------------------------------------------------------------------------
-- Discovered field cache (PRD §19.8)
-- ---------------------------------------------------------------------------

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
CREATE TABLE definition_field_uses (
    definition_id   TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    field_key       TEXT NOT NULL,
    layer           TEXT NOT NULL CHECK (layer IN ('Static','Discovered')),
    authored_library_id TEXT REFERENCES libraries(id) ON DELETE SET NULL,
    json_pointer    TEXT NOT NULL,               -- where in the body, for precise GUI highlighting
    PRIMARY KEY (definition_id, json_pointer)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_definition_field_uses__field ON definition_field_uses(field_key);
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
-- ---------------------------------------------------------------------------
-- Definitions (PRD §19.9)
-- ---------------------------------------------------------------------------

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
CREATE TABLE definition_validations (
    definition_id     TEXT PRIMARY KEY REFERENCES definitions(id) ON DELETE CASCADE,
    body_hash         TEXT NOT NULL,              -- which body this verdict applies to
    registry_version  INTEGER NOT NULL,
    status            TEXT NOT NULL CHECK (status IN ('Valid','Degraded','Invalid')),
    issues_json       TEXT NOT NULL CHECK (json_valid(issues_json)),
    checked_at        INTEGER NOT NULL
) STRICT;

CREATE INDEX ix_definition_validations__status ON definition_validations(status) WHERE status <> 'Valid';
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
CREATE TABLE managed_collection_items (
    managed_collection_id TEXT NOT NULL REFERENCES managed_collections(id) ON DELETE CASCADE,
    library_item_id       TEXT NOT NULL REFERENCES library_items(id) ON DELETE CASCADE,
    ordinal               INTEGER NOT NULL,
    added_at              INTEGER NOT NULL,
    PRIMARY KEY (managed_collection_id, library_item_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_managed_collection_items__ordinal ON managed_collection_items(managed_collection_id, ordinal);
CREATE INDEX ix_managed_collection_items__item    ON managed_collection_items(library_item_id);
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
-- ---------------------------------------------------------------------------
-- Packs (PRD §19.10)
-- ---------------------------------------------------------------------------

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
-- ---------------------------------------------------------------------------
-- Sources, health, and contribution freezing (PRD §19.11)
-- ---------------------------------------------------------------------------

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
-- ---------------------------------------------------------------------------
-- Lifecycle tables (PRD §19.12)
-- ---------------------------------------------------------------------------

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
CREATE TABLE lifecycle_references (
    subject_id     TEXT NOT NULL REFERENCES lifecycle_subjects(id) ON DELETE CASCADE,
    definition_id  TEXT NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    first_seen_at  INTEGER NOT NULL,
    last_seen_at   INTEGER NOT NULL,
    PRIMARY KEY (subject_id, definition_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_lifecycle_references__definition ON lifecycle_references(definition_id);
CREATE TABLE lifecycle_subject_ids (
    subject_id  TEXT NOT NULL REFERENCES lifecycle_subjects(id) ON DELETE CASCADE,
    id_space    TEXT NOT NULL,
    id_value    TEXT NOT NULL,
    PRIMARY KEY (subject_id, id_space)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_lifecycle_subject_ids__lookup ON lifecycle_subject_ids(id_space, id_value);
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
-- ---------------------------------------------------------------------------
-- Placement tables (PRD §19.13)
-- ---------------------------------------------------------------------------

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
CREATE TABLE placement_visibility (
    participant_id  TEXT NOT NULL REFERENCES placement_participants(id) ON DELETE CASCADE,
    surface         TEXT NOT NULL CHECK (surface IN ('Home','Recommended')),
    principal_id    TEXT NOT NULL REFERENCES principals(id) ON DELETE CASCADE,
    PRIMARY KEY (participant_id, surface, principal_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_placement_visibility__principal ON placement_visibility(principal_id);
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
-- ---------------------------------------------------------------------------
-- Assets and rendering (PRD §19.14)
-- ---------------------------------------------------------------------------

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
-- ---------------------------------------------------------------------------
-- Jobs, scheduling, and observability (PRD §19.15)
-- ---------------------------------------------------------------------------

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

-- ---------------------------------------------------------------------------
-- Seeded rows (PRD §19.6)
--
-- The three whole-audience principals. Their ULIDs are fixed constants so a
-- definition body that names an audience means the same thing in every
-- installation; `created_at` is 0 because these rows precede the instance
-- rather than being created by it. Mirrored in
-- `afisharr_core::identifier::principals`.
-- ---------------------------------------------------------------------------

INSERT INTO principals (id, kind, label, created_at) VALUES
    ('00000000000000000000000001', 'Everyone',  'Everyone',  0),
    ('00000000000000000000000002', 'Owner',     'Owner',     0),
    ('00000000000000000000000003', 'SharedAll', 'SharedAll', 0);
