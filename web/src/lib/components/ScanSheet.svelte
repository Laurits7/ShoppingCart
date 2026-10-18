<script lang="ts">
  // Scan a barcode with the camera (HTTPS and BarcodeDetector), or type it.
  import { canScan, cleanBarcode, scan } from '../barcode'
  import { toast } from '../toast.svelte'
  import Sheet from './Sheet.svelte'

  let { open = $bindable(false), title = 'Scan a barcode', onfound }: { open: boolean; title?: string; onfound: (code: string) => void } = $props()
  let typed = $state('')
  let video = $state<HTMLVideoElement>()
  let failed = $state('')
  const camera = canScan()

  $effect(() => {
    if (!open || !camera || !video) return
    let stop: (() => void) | undefined
    let cancelled = false
    failed = ''
    scan(video, (code) => {
      open = false
      onfound(code)
    })
      .then((s) => (cancelled ? s() : (stop = s)))
      .catch(() => (failed = 'The camera could not be started. Type the barcode instead.'))
    return () => {
      cancelled = true
      stop?.()
    }
  })

  function submit(e: Event) {
    e.preventDefault()
    const code = cleanBarcode(typed)
    if (!code) return toast('A barcode is 8 to 14 digits', 'error')
    typed = ''
    open = false
    onfound(code)
  }
</script>

<Sheet bind:open {title}>
  {#if camera && !failed}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video bind:this={video} playsinline muted></video>
    <p class="muted small">Point the camera at the barcode.</p>
  {:else}
    <p class="muted small">
      {failed || 'This browser can’t scan barcodes (scanning needs HTTPS and a browser like Chrome on Android).'} The digits are printed under
      the barcode.
    </p>
  {/if}
  <form class="row" onsubmit={submit}>
    <input bind:value={typed} inputmode="numeric" autocomplete="off" placeholder="Type the barcode, e.g. 4740125120011" aria-label="Barcode" maxlength="20" />
    <button class="btn" type="submit">Look up</button>
  </form>
</Sheet>

<style>
  video {
    width: 100%;
    max-height: 50dvh;
    border-radius: var(--radius-sm);
    background: #000;
    object-fit: cover;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
  .row {
    display: flex;
    gap: 8px;
  }
</style>
