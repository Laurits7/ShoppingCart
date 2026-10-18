import { describe, expect, it } from 'vitest'
import { canScan, cleanBarcode } from './barcode'

describe('barcodes', () => {
  it('cleans typed barcodes like the server', () => {
    expect(cleanBarcode('4740 0120-12345')).toBe('4740012012345')
    expect(cleanBarcode('1234567')).toBe(null)
    expect(cleanBarcode('12345678a')).toBe(null)
  })

  it('does not offer the camera without BarcodeDetector', () => {
    expect(canScan()).toBe(false)
  })
})
