CREATE TABLE usage_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    store_id BLOB NOT NULL,
    identity_salt BLOB NOT NULL,
    parser_version INTEGER NOT NULL,
    published_epoch INTEGER NOT NULL,
    preparing_source_key BLOB,
    last_success_at_ms INTEGER,
    stable_event_count INTEGER NOT NULL
);

CREATE TABLE usage_sources (
    source_key BLOB PRIMARY KEY,
    root_key BLOB NOT NULL,
    published_fingerprint BLOB,
    published_prefix_bytes INTEGER NOT NULL,
    coverage INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE usage_keys (
    event_key BLOB PRIMARY KEY,
    first_epoch INTEGER NOT NULL
) WITHOUT ROWID;
CREATE INDEX usage_keys_epoch ON usage_keys (first_epoch, event_key);

CREATE TABLE usage_observations (
    id INTEGER PRIMARY KEY,
    event_key BLOB NOT NULL,
    source_key BLOB NOT NULL,
    evidence_key BLOB NOT NULL,
    first_epoch INTEGER NOT NULL,
    event_at_ms INTEGER,
    model TEXT COLLATE BINARY,
    model_key BLOB,
    uncached_input INTEGER,
    cache_read INTEGER,
    cache_write INTEGER,
    cache_write_1h INTEGER,
    output INTEGER,
    category_flags INTEGER NOT NULL
);
CREATE UNIQUE INDEX usage_observations_identity
    ON usage_observations (event_key, source_key, evidence_key);
CREATE INDEX usage_observations_epoch
    ON usage_observations (first_epoch, event_key, id);
CREATE INDEX usage_observations_revision
    ON usage_observations (event_key, uncached_input, cache_read, cache_write, cache_write_1h, output, id);

CREATE TABLE usage_canonical (
    id INTEGER PRIMARY KEY,
    event_key BLOB NOT NULL,
    from_epoch INTEGER NOT NULL,
    until_epoch INTEGER,
    event_at_ms INTEGER,
    model TEXT COLLATE BINARY,
    uncached_input INTEGER,
    cache_read INTEGER,
    cache_write INTEGER,
    cache_write_1h INTEGER,
    output INTEGER,
    category_flags INTEGER NOT NULL,
    resolution INTEGER NOT NULL
);
CREATE UNIQUE INDEX usage_canonical_identity ON usage_canonical (event_key, from_epoch);
CREATE INDEX usage_canonical_day ON usage_canonical (event_at_ms, id);
CREATE INDEX usage_canonical_model ON usage_canonical (model, event_at_ms, id);
CREATE INDEX usage_canonical_from ON usage_canonical (from_epoch, id);
CREATE INDEX usage_canonical_until ON usage_canonical (until_epoch, id);

PRAGMA user_version = 1;
