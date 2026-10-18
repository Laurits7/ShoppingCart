# Shops and lists

## Shops
A **shop** is a place you buy things: "Rimi", "Hardware store", "Pharmacy". Each person has
their own shops. The first time you open the Shopping tab it asks for one.

The **Shopping** tab shows everything on one page: each shop with its lists, and on each list
card the things still to buy. Tick items, change amounts and add new ones right there; each card
has its own add box. Tap a list's name to open it on its own, grouped by category, and
*All lists* to come back. Collapse a card with the arrow next to its name (remembered on this
device). New lists and shops are created at the bottom of each shop and of the page.

Every shop has one or more **lists** ("Weekly", "Party on Saturday"). One of them is the
**default list**, marked with a star: it's where new items go when you don't pick a list, for
example from [Mealie](mealie.md) or from the [API](../api.md). In a list's **⋯** menu you can
rename it, make it the default, archive it or delete it (the default list can't be archived or
deleted until another list is the default). The same menu renames, recolours and deletes the
shop; deleting a shop says how many lists and items go with it.


## Categories
A list is split into **categories** such as Dairy, Meat, Vegetables. A shop's first list starts
with a sensible set; a new list in a shop copies the categories of the shop's default list, so
you only set up the aisles once. Add, rename and delete categories in the list's **⋯** menu
(deleting one moves its items to *Other*). Only categories with something in them are shown;
on an open list, tap a category's name to collapse it. Items without a category are under *Other* at the end.

## Adding items
Type in the box at the top and press Enter. An amount at the end is recognised: `milk 2`,
`flour 1 kg`.

While you type, **suggestions from everything you've added before** appear right away; tap one
to add it. It goes straight into its usual category with its usual amount. With the box empty,
your most frequent items for this shop that aren't on the list yet are shown as chips: one tap
each.

Adding something that's already on the list adds to its amount instead of making a second
row, with a message like "Milk: 2 → 3" and an undo button. Grams and kilos (or millilitres and
litres) are added up: 1 kg of flour plus `flour 500g` makes 1.5 kg. Amounts in units that don't
mix, like 1 l of milk and 2 milk cartons, stay separate rows.

## Changing amounts
Every item has **−** and **+** buttons. The step fits the unit: 1 for pieces and packs, 0.5 for
kg and litres, 100 for grams and millilitres. Tap the amount itself to type an exact value or
change the unit (or give the item its own step). Pressing − at the lowest amount removes the
item (with undo).

The **⋯** button on an item edits its name, amount, unit, category and note, turns it into a
staple, or deletes it (with undo). Moving an item to another category is remembered: next time
it goes there by itself.

## Your item catalogue
Everything you've added is remembered in your personal **item catalogue**. In
**Settings → Item catalogue** you can rename entries, set their usual category, unit and step,
merge duplicates (say *Tomato* and *Tomatoes*) and hide or delete what you no longer buy.

## Shopping
Tap an item to put it **in the cart**; it moves to the bottom so the rest of the list stays
short (on the summary page the cart is folded into one line; tap it to see what's in it). Tap
again to take it out. When you're done, **Finish shopping** clears the cart (with
undo for a few seconds). Finished items stay in your catalogue.

!!! note "Coming in Phase 4"
    Working through a bad signal in the shop (changes saved on the phone and sent when the
    connection is back) arrives with live sync. For now a change that can't be sent shows an
    error and the list reloads.

## Staples
**Staples** are what you usually buy at a shop. The checklist button on a list card (or next to
the list's name when it's open) shows them; the ones not already on the list are pre-ticked, and one tap adds them with their
amount and category. Add staples there (`milk 2` works) or turn any item into a staple from its
**⋯** menu.

## Brands and pictures
Items can be a specific brand with a picture ("milk by Alma"); see
[Brands and pictures](products.md).

## Ordering and moving
Drag things by their grip (the dotted handle on the left) to put them in order: items within a
category or into another one, categories (on an open list), lists within a shop, shops, and
staples. With a mouse you can also just drag an item's row; on a phone, press and hold a row
for a moment to lift it, or drag the grip straight away. While you drag an item on an open
list, empty categories appear so you can drop into them, and collapsed ones open.

On the summary page you can drag an item from one list card into another, even in another
shop; it keeps its category if the other list has one of the same name.

To move several items at once, open the list, tap **Select…**, pick the items and choose
**Move to…**. A single item can be moved from its **⋯** menu too.

With a keyboard: focus a grip, press ++space++ to pick the thing up, the arrow keys to move it
and ++space++ to drop it (++escape++ puts it back). Screen readers announce each step.

## Sharing
Lists are private until you share them; see [Households and sharing](sharing.md).
