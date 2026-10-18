-- Phase 3b: pictures, products (brands) and preferred products (SPEC §5.2, §6.4b).

-- A picture, stored resized as data/images/<sha256>.jpg (thumbnail) and <sha256>-l.jpg
-- (large), never the original. Rows are per owner; identical pictures share the files.
CREATE TABLE images (
    id         TEXT PRIMARY KEY,
    owner_id   TEXT NOT NULL REFERENCES users(id),
    sha256     TEXT NOT NULL,
    width      INTEGER NOT NULL,
    height     INTEGER NOT NULL,
    bytes      INTEGER NOT NULL,
    -- 'upload' or 'openfoodfacts'; source_ref is the page to credit (CC BY-SA).
    source     TEXT NOT NULL CHECK (source IN ('upload', 'openfoodfacts')),
    source_ref TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (owner_id, sha256)
);

-- A branded version of a catalogue item ("Milk" → "Alma Piim 2.5% 1 l"), per user like the
-- catalogue (D-21).
CREATE TABLE products (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id),
    catalog_item_id TEXT NOT NULL REFERENCES catalog_items(id),
    brand           TEXT NOT NULL DEFAULT '',
    name            TEXT NOT NULL DEFAULT '',
    barcode         TEXT,
    image_id        TEXT REFERENCES images(id),
    source          TEXT NOT NULL CHECK (source IN ('manual', 'openfoodfacts')),
    source_ref      TEXT,
    position        TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    rev             INTEGER NOT NULL
);
CREATE INDEX products_catalog ON products(catalog_item_id);
CREATE INDEX products_barcode ON products(user_id, barcode);

-- The preferred product of a catalogue item: everywhere (shop_id = '') or at one shop.
CREATE TABLE product_preferences (
    catalog_item_id TEXT NOT NULL REFERENCES catalog_items(id),
    shop_id         TEXT NOT NULL DEFAULT '',
    product_id      TEXT NOT NULL REFERENCES products(id),
    PRIMARY KEY (catalog_item_id, shop_id)
);

ALTER TABLE catalog_items ADD COLUMN image_id TEXT REFERENCES images(id);
-- Empty: any brand ("milk"); set: this product ("milk by Alma").
ALTER TABLE items ADD COLUMN product_id TEXT REFERENCES products(id);
ALTER TABLE staples ADD COLUMN product_id TEXT REFERENCES products(id);

-- Product lookups (Open Food Facts) are cached for a day.
CREATE TABLE lookup_cache (
    key        TEXT PRIMARY KEY,
    body       TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);
