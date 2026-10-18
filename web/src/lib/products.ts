// Which product and picture an item shows. Mirror of crates/domain/src/products.rs; keep
// the two in step (the tests share their cases).

export type PictureFrom = 'pinned' | 'preferred' | 'catalog'

/** The product a generic item is bought as: preferred at this shop, else everywhere. */
export const preferred = (atShop: string | null | undefined, everywhere: string | null | undefined) => atShop ?? everywhere ?? null

/** Each argument is that source's picture: `undefined` = no such source, `null` = it has no
 * picture. A pinned product without a picture falls back to the catalogue item's picture,
 * never to another brand's. */
export function picture(
  pinned: string | null | undefined,
  pref: string | null | undefined,
  catalog: string | null | undefined,
): { image: string; from: PictureFrom } | null {
  if (pinned !== undefined) {
    if (pinned) return { image: pinned, from: 'pinned' }
  } else if (pref) return { image: pref, from: 'preferred' }
  return catalog ? { image: catalog, from: 'catalog' } : null
}

/** "Alma · Piim 2.5% 1 l", or whichever part there is. */
export function label(brand: string, name: string): string {
  const b = brand.trim()
  const n = name.trim()
  return b && n ? `${b} · ${n}` : b || n
}

export const imageUrl = (id: string, large = false) => `/api/v1/images/${id}${large ? '?size=large' : ''}`

/** What a lookup result is used for: a new product, a picture for a product, or a picture
 * for the catalogue item itself. */
export type LookupMode = { kind: 'product' } | { kind: 'product-picture'; productId: string } | { kind: 'catalog-picture' }
