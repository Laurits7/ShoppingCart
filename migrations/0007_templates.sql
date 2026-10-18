-- Templates and the inbox (D-65, owner request): a list is a shopping `list`, a `template`
-- (usual items to start lists from, several per shop) or the user's `inbox` (items that came
-- without a list, e.g. from Mealie; no shop). Staples become a template per shop.
--
-- `lists.shop_id` becomes optional, which SQLite can only do by rebuilding the table (its
-- documented procedure: copy, drop, rename). Foreign keys are switched off around migrations
-- and checked afterwards (db.rs), as that procedure needs.

CREATE TABLE lists_new (
    id          TEXT PRIMARY KEY,
    shop_id     TEXT REFERENCES shops(id),
    owner_id    TEXT NOT NULL REFERENCES users(id),
    kind        TEXT NOT NULL DEFAULT 'list' CHECK (kind IN ('list', 'template', 'inbox')),
    name        TEXT NOT NULL,
    position    TEXT NOT NULL,
    archived_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    rev         INTEGER NOT NULL,
    CHECK ((kind = 'inbox') = (shop_id IS NULL))
);
INSERT INTO lists_new (id, shop_id, owner_id, kind, name, position, archived_at, created_at, updated_at, deleted_at, rev)
    SELECT id, shop_id, owner_id, 'list', name, position, archived_at, created_at, updated_at, deleted_at, rev FROM lists;
DROP TABLE lists;
ALTER TABLE lists_new RENAME TO lists;
CREATE INDEX lists_shop ON lists(shop_id);
CREATE INDEX lists_owner ON lists(owner_id);
CREATE INDEX lists_rev ON lists(rev);
CREATE UNIQUE INDEX lists_inbox ON lists(owner_id) WHERE kind = 'inbox' AND deleted_at IS NULL;

-- Each shop's staples become its template "Usual things", with the categories of the shop's
-- (former) default list.
UPDATE meta_rev SET rev = rev + 1 WHERE id = 1;
INSERT INTO lists (id, shop_id, owner_id, kind, name, position, created_at, updated_at, rev)
    SELECT upper(hex(randomblob(13))), p.id, p.owner_id, 'template', 'Usual things', 'V',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           (SELECT rev FROM meta_rev WHERE id = 1)
    FROM shops p
    WHERE p.deleted_at IS NULL
      AND EXISTS (SELECT 1 FROM staples s WHERE s.shop_id = p.id AND s.deleted_at IS NULL);
INSERT INTO categories (id, list_id, name, position, created_at, updated_at, rev)
    SELECT upper(hex(randomblob(13))), t.id, c.name, c.position, t.created_at, t.created_at, t.rev
    FROM lists t
    JOIN shops p ON p.id = t.shop_id
    JOIN categories c ON c.list_id = p.default_list_id AND c.deleted_at IS NULL
    WHERE t.kind = 'template';
INSERT INTO items (id, list_id, category_id, catalog_item_id, product_id, name, amount, unit, note, position,
                   added_by, source, created_at, updated_at, rev)
    SELECT upper(hex(randomblob(13))), t.id,
           (SELECT c.id FROM categories c WHERE c.list_id = t.id AND lower(c.name) = lower(s.category_name) LIMIT 1),
           s.catalog_item_id, s.product_id, s.name, s.amount, s.unit, '', s.position,
           t.owner_id, 'staple', t.created_at, t.created_at, t.rev
    FROM staples s JOIN lists t ON t.shop_id = s.shop_id AND t.kind = 'template'
    WHERE s.deleted_at IS NULL;
DROP TABLE staples;
