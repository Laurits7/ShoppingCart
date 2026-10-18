# Brands and pictures

!!! note "Planned"
    This is Phase 3b of the [work plan](../WORKPLAN.md).

The same thing often comes from different producers: *milk by Alma* or *milk by Farmi*. In
ShoppingCart, an item in your [catalogue](shopping.md#your-item-catalogue) such as **Milk** can
have several **products**, each with a brand, a name, a picture and optionally a barcode.

## On a list
An item is either **generic** ("Milk", any will do) or **a specific product**
("Milk · Alma 2.5%"). Products show a small picture; tap it for a big one, so whoever goes
shopping grabs the right package. You can have two different milks on a list at once.

To choose, open the item's menu → **Brand**: pick one of the products, add a new one, or keep it
generic. When typing in quick add, products appear as their own suggestions with pictures.

## Preferred brand
Mark a product as **preferred** for an item, either everywhere or just for one shop ("at Rimi I
buy Farmi"). A generic item then shows the preferred product's picture as a hint.

## Getting pictures
- **Find pictures** searches [Open Food Facts](https://world.openfoodfacts.org), a free and
  open database of food products with photos, by name or barcode. Pick a result and its brand,
  name and picture are filled in. The picture is downloaded by your server and stored there,
  so your lists keep working offline. Open Food Facts pictures are licensed CC BY-SA; the
  attribution is shown on the big picture.
- **Take a photo** with your phone or **upload** one. Good for local products that aren't in
  Open Food Facts. Pictures are shrunk on the server and location data is removed.

## Barcodes
Over HTTPS, in browsers that support it (Chrome/Edge on Android and desktop; not all others),
**Scan** opens the camera: scan a package to find or create its product, or to add it to the
list. Elsewhere you can type the barcode.

## Turning lookups off
Set `PRODUCT_LOOKUP=off` (see [Configuration](../getting-started/configuration.md)) and nothing
leaves your server; your own photos still work.
