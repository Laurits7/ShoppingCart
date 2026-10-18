# Brands and pictures

The same thing often comes from different producers: *milk by Alma* or *milk by Farmi*. In
ShoppingCart, an item in your [catalogue](shopping.md#your-item-catalogue) such as **Milk** can
have several **products**, each with a brand, a name, a picture and optionally a barcode.
Products and pictures are personal, like the catalogue.

## On a list
An item is either **generic** ("Milk", any will do) or **a specific product**
("Milk · Alma Piim 2.5%"). Items with a picture show a small one at the end of the row; tap it
for a big one, so whoever goes shopping grabs the right package. You can have two different
milks on a list at once: adding the same brand again adds to its amount, another brand gets its
own row.

To choose, open the item's **⋯** menu → **Brand…**: pick one of the products, **Any brand**, add
a new one, or find one on Open Food Facts. While you type in the add box, each suggestion lists
its brands underneath, with pictures; tap one to add that brand.

## Preferred brand
In the same sheet, star a product as **Preferred** (everywhere) or **At** this shop ("at Rimi I
buy Farmi"). A generic item then shows the preferred product's picture and "usually Alma …" as
a hint, while any brand still does.

## Getting pictures
- **Find on Open Food Facts** searches [Open Food Facts](https://world.openfoodfacts.org), a
  free and open database of food products with photos, by name or barcode. Pick a result and
  its brand, name, barcode and picture are filled in. Your server does the searching and
  downloads the picture, then keeps it, so your browser never contacts Open Food Facts and your
  lists keep working offline. Open Food Facts pictures are licensed CC BY-SA; the credit is
  shown on the big picture.
- **Photo** takes a picture with your phone's camera; **Choose** uploads one. Good for local
  products that aren't in Open Food Facts. Pictures are shrunk on the server (to about 160 and
  640 pixels) and saved as new JPEG files, so location and other photo data are dropped.
- An item can also have its own picture without a brand: at the bottom of the brand sheet, or
  in **Settings → Item catalogue → Brands and picture…**.

Open Food Facts allows only a few searches a minute, so results are kept for a day. If it's
unreachable, only finding pictures is affected.

## Barcodes
The barcode button in the add box adds a product by its barcode. Over HTTPS, in browsers that
support it (Chrome or Edge on Android, some desktops), it opens the camera; elsewhere you type
the digits printed under the barcode. If one of your products has that barcode it is added;
otherwise it's looked up on Open Food Facts and added as a new product. The same button in
**Find on Open Food Facts** looks a barcode up.

## Imports
Items from the [API](../api.md) (and later Mealie) stay generic, unless their text is exactly the
name of one of your products ("Alma Piim 2.5%"); then that product is used.

## Turning lookups off
Set `PRODUCT_LOOKUP=off` (see [Configuration](../getting-started/configuration.md)) and nothing
leaves your server; your own photos still work.
