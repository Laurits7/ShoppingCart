-- ShoppingCart initial schema (Phase 1: accounts and households).
-- Conventions (WORKPLAN A.3): ULID text ids; UTC RFC 3339 timestamps; synced entities have
-- created_at/updated_at/deleted_at/rev, with rev taken from the global counter in meta_rev.

CREATE TABLE meta_rev (
    id  INTEGER PRIMARY KEY CHECK (id = 1),
    rev INTEGER NOT NULL
);
INSERT INTO meta_rev (id, rev) VALUES (1, 0);

CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name  TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin      INTEGER NOT NULL DEFAULT 0,
    locale        TEXT NOT NULL DEFAULT '',
    -- Free-form UI preferences (theme, last list…), JSON object.
    prefs         TEXT NOT NULL DEFAULT '{}',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT,
    rev           INTEGER NOT NULL
);

CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    token_hash   TEXT NOT NULL UNIQUE,
    user_id      TEXT NOT NULL REFERENCES users(id),
    created_at   TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    expires_at   TEXT NOT NULL,
    user_agent   TEXT
);
CREATE INDEX sessions_user ON sessions(user_id);

CREATE TABLE api_tokens (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id),
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    scope        TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    created_at   TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at   TEXT
);
CREATE INDEX api_tokens_user ON api_tokens(user_id);

CREATE TABLE households (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    created_by TEXT REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    rev        INTEGER NOT NULL
);

CREATE TABLE household_members (
    household_id TEXT NOT NULL REFERENCES households(id),
    user_id      TEXT NOT NULL REFERENCES users(id),
    role         TEXT NOT NULL CHECK (role IN ('owner', 'member')),
    added_at     TEXT NOT NULL,
    PRIMARY KEY (household_id, user_id)
);
CREATE INDEX household_members_user ON household_members(user_id);
