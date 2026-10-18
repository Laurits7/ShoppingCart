// Barcode scanning with the browser's BarcodeDetector (Chrome on Android, some desktops),
// which needs HTTPS. Everywhere else the barcode is typed.

type Detected = { rawValue: string }
type Detector = { detect: (source: CanvasImageSource) => Promise<Detected[]> }
type DetectorClass = new (opts: { formats: string[] }) => Detector

const FORMATS = ['ean_13', 'ean_8', 'upc_a', 'upc_e']

function detectorClass(): DetectorClass | null {
  const c = (globalThis as unknown as { BarcodeDetector?: DetectorClass }).BarcodeDetector
  return c && globalThis.isSecureContext && typeof navigator.mediaDevices?.getUserMedia === 'function' ? c : null
}

export const canScan = () => detectorClass() !== null

/** A barcode as typed: digits only, 8 to 14 of them; null otherwise. */
export function cleanBarcode(s: string): string | null {
  const d = s.replace(/[\s-]/g, '')
  return /^\d{8,14}$/.test(d) ? d : null
}

/** Watch the rear camera in `video` until a product barcode is seen. Returns a stop function. */
export async function scan(video: HTMLVideoElement, onfound: (code: string) => void): Promise<() => void> {
  const Detector = detectorClass()
  if (!Detector) throw new Error('scanning is not supported here')
  const detector = new Detector({ formats: FORMATS })
  const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' }, audio: false })
  video.srcObject = stream
  await video.play()
  let stopped = false
  const stop = () => {
    stopped = true
    stream.getTracks().forEach((t) => t.stop())
    video.srcObject = null
  }
  const tick = async () => {
    if (stopped) return
    try {
      const found = await detector.detect(video)
      const code = found.map((f) => cleanBarcode(f.rawValue)).find(Boolean)
      if (code) {
        navigator.vibrate?.(30)
        stop()
        onfound(code)
        return
      }
    } catch {
      /* a frame that couldn't be read */
    }
    setTimeout(tick, 250)
  }
  tick()
  return stop
}
