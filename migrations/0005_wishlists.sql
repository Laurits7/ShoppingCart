-- Phase 5: wishlists, gift lists and claims (SPEC §5.3, §6.9, §7.3).

-- Someone gifts are for: a name, and optionally their account here (a gift list about them
-- is then hidden from that account). Per owner.
CREATE TABLE persons (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id),
    name       TEXT NOT NULL,
    user_id    TEXT REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    rev        INTEGER NOT NULL
);
CREATE INDEX persons_owner ON persons(owner_id);
CREATE INDEX persons_user ON persons(user_id);
CREATE INDEX persons_rev ON persons(rev);

CREATE TABLE wishlists (
    id                  TEXT PRIMARY KEY,
    owner_id            TEXT NOT NULL REFERENCES users(id),
    kind                TEXT NOT NULL CHECK (kind IN ('own', 'gift')),
    recipient_person_id TEXT REFERENCES persons(id),
    title               TEXT NOT NULL,
    position            TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    deleted_at          TEXT,
    rev                 INTEGER NOT NULL,
    CHECK ((kind = 'gift') = (recipient_person_id IS NOT NULL))
);
CREATE INDEX wishlists_owner ON wishlists(owner_id);
CREATE INDEX wishlists_rev ON wishlists(rev);

CREATE TABLE wish_sections (
    id          TEXT PRIMARY KEY,
    wishlist_id TEXT NOT NULL REFERENCES wishlists(id),
    name        TEXT NOT NULL,
    position    TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    rev         INTEGER NOT NULL
);
CREATE INDEX wish_sections_list ON wish_sections(wishlist_id);
CREATE INDEX wish_sections_rev ON wish_sections(rev);

CREATE TABLE wish_items (
    id          TEXT PRIMARY KEY,
    wishlist_id TEXT NOT NULL REFERENCES wishlists(id),
    section_id  TEXT,
    title       TEXT NOT NULL,
    url         TEXT,
    price       REAL,
    currency    TEXT,
    note        TEXT NOT NULL DEFAULT '',
    priority    INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 2),
    position    TEXT NOT NULL,
    received_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    rev         INTEGER NOT NULL
);
CREATE INDEX wish_items_list ON wish_items(wishlist_id);
CREATE INDEX wish_items_rev ON wish_items(rev);

-- "I'll get this". Claims never touch their item's rev, so a wishlist's owner can't notice
-- them through changes (§6.9). At most one live claim per item.
CREATE TABLE claims (
    id           TEXT PRIMARY KEY,
    wish_item_id TEXT NOT NULL REFERENCES wish_items(id),
    user_id      TEXT NOT NULL REFERENCES users(id),
    status       TEXT NOT NULL CHECK (status IN ('planned', 'bought')),
    note         TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    rev          INTEGER NOT NULL
);
CREATE UNIQUE INDEX claims_live ON claims(wish_item_id) WHERE deleted_at IS NULL;
CREATE INDEX claims_rev ON claims(rev);
