-- Phase 6: integrations (SPEC §5.5, §6.10). Personal: each belongs to one user.

CREATE TABLE integrations (
    id           TEXT PRIMARY KEY,
    owner_id     TEXT NOT NULL REFERENCES users(id),
    provider     TEXT NOT NULL CHECK (provider IN ('mealie')),
    -- Provider settings as JSON (never the token).
    settings     TEXT NOT NULL DEFAULT '{}',
    -- The token, encrypted (secrets.rs); never returned to clients.
    secret       TEXT,
    enabled      INTEGER NOT NULL DEFAULT 0,
    last_sync_at TEXT,
    last_error   TEXT,
    last_error_at TEXT,
    -- When the background job runs it next (back-off after failures).
    next_run_at  TEXT,
    failures     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    rev          INTEGER NOT NULL
);
CREATE INDEX integrations_owner ON integrations(owner_id);

-- What has been imported, so nothing is imported twice (SPEC §5.5).
CREATE TABLE external_links (
    integration_id TEXT NOT NULL REFERENCES integrations(id),
    external_id    TEXT NOT NULL,
    item_id        TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    PRIMARY KEY (integration_id, external_id)
);
