-- Phase 4: sharing lists (later wishlists) with users and households (SPEC §5.4, §6.8).
-- A household share reaches its current and future members. Revoking tombstones the row
-- (with a new rev), so live clients learn about it.

CREATE TABLE shares (
    id             TEXT PRIMARY KEY,
    resource_type  TEXT NOT NULL CHECK (resource_type IN ('list', 'wishlist')),
    resource_id    TEXT NOT NULL,
    principal_type TEXT NOT NULL CHECK (principal_type IN ('user', 'household')),
    principal_id   TEXT NOT NULL,
    role           TEXT NOT NULL CHECK (role IN ('editor', 'viewer')),
    created_by     TEXT NOT NULL REFERENCES users(id),
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    deleted_at     TEXT,
    rev            INTEGER NOT NULL
);
CREATE UNIQUE INDEX shares_live ON shares(resource_type, resource_id, principal_type, principal_id)
    WHERE deleted_at IS NULL;
CREATE INDEX shares_principal ON shares(principal_type, principal_id);

-- Finding what a write changed by its revs (for live updates) needs these.
CREATE INDEX items_rev ON items(rev);
CREATE INDEX categories_rev ON categories(rev);
CREATE INDEX lists_rev ON lists(rev);
CREATE INDEX shops_rev ON shops(rev);
CREATE INDEX staples_rev ON staples(rev);
CREATE INDEX catalog_items_rev ON catalog_items(rev);
CREATE INDEX products_rev ON products(rev);
CREATE INDEX shares_rev ON shares(rev);
CREATE INDEX households_rev ON households(rev);
