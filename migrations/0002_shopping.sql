-- Phase 2: shops, lists, categories, items, staples and the item catalogue (SPEC §5.2).
-- Positions are fractional-index keys (domain::order). Amounts are REAL, kept to three
-- decimals; a NULL unit means pieces.

CREATE TABLE shops (
    id              TEXT PRIMARY KEY,
    owner_id        TEXT NOT NULL REFERENCES users(id),
    name            TEXT NOT NULL,
    color           TEXT,
    position        TEXT NOT NULL,
    default_list_id TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    rev             INTEGER NOT NULL
);
CREATE INDEX shops_owner ON shops(owner_id);

CREATE TABLE lists (
    id          TEXT PRIMARY KEY,
    shop_id     TEXT NOT NULL REFERENCES shops(id),
    owner_id    TEXT NOT NULL REFERENCES users(id),
    name        TEXT NOT NULL,
    position    TEXT NOT NULL,
    archived_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    rev         INTEGER NOT NULL
);
CREATE INDEX lists_shop ON lists(shop_id);
CREATE INDEX lists_owner ON lists(owner_id);

CREATE TABLE categories (
    id         TEXT PRIMARY KEY,
    list_id    TEXT NOT NULL REFERENCES lists(id),
    name       TEXT NOT NULL,
    position   TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    rev        INTEGER NOT NULL
);
CREATE INDEX categories_list ON categories(list_id);

-- Per user (D-20). normalized_name is unique among a user's live entries.
CREATE TABLE catalog_items (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id),
    name            TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    category_name   TEXT,
    amount          REAL NOT NULL DEFAULT 1,
    unit            TEXT,
    step            REAL,
    use_count       INTEGER NOT NULL DEFAULT 0,
    last_used_at    TEXT,
    shop_id         TEXT,
    hidden          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    rev             INTEGER NOT NULL
);
CREATE UNIQUE INDEX catalog_items_name ON catalog_items(user_id, normalized_name)
    WHERE deleted_at IS NULL;

-- How often each catalogue item was added at each shop: frequent-item chips and the shop
-- boost in suggestions (D-27).
CREATE TABLE catalog_shop_uses (
    catalog_item_id TEXT NOT NULL REFERENCES catalog_items(id),
    shop_id         TEXT NOT NULL REFERENCES shops(id),
    use_count       INTEGER NOT NULL,
    last_used_at    TEXT NOT NULL,
    PRIMARY KEY (catalog_item_id, shop_id)
);
CREATE INDEX catalog_shop_uses_shop ON catalog_shop_uses(shop_id);

CREATE TABLE items (
    id              TEXT PRIMARY KEY,
    list_id         TEXT NOT NULL REFERENCES lists(id),
    category_id     TEXT,
    catalog_item_id TEXT,
    name            TEXT NOT NULL,
    amount          REAL NOT NULL DEFAULT 1,
    unit            TEXT,
    step            REAL,
    note            TEXT NOT NULL DEFAULT '',
    position        TEXT NOT NULL,
    checked_at      TEXT,
    checked_by      TEXT,
    added_by        TEXT NOT NULL REFERENCES users(id),
    source          TEXT NOT NULL CHECK (source IN ('manual', 'staple', 'mealie', 'api')),
    external_ref    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    rev             INTEGER NOT NULL
);
CREATE INDEX items_list ON items(list_id);

-- A create that was folded into an existing item (duplicate guard): retrying it with the
-- same client id must not add the amount again. Kept for a day.
CREATE TABLE item_merges (
    id         TEXT PRIMARY KEY,
    item_id    TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE staples (
    id              TEXT PRIMARY KEY,
    shop_id         TEXT NOT NULL REFERENCES shops(id),
    catalog_item_id TEXT,
    name            TEXT NOT NULL,
    amount          REAL NOT NULL DEFAULT 1,
    unit            TEXT,
    category_name   TEXT,
    position        TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    rev             INTEGER NOT NULL
);
CREATE INDEX staples_shop ON staples(shop_id);
