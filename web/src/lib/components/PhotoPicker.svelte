<script lang="ts">
  // Take a photo with the phone's camera or choose a picture; it's uploaded and resized on
  // the server, and `onpicked` gets the stored picture's id.
  import { shopping } from '../shopping.svelte'
  import Icon from './Icon.svelte'

  let { onpicked, label = 'Photo' }: { onpicked: (imageId: string) => void; label?: string } = $props()
  let busy = $state(false)
  let camera: HTMLInputElement
  let files: HTMLInputElement

  async function picked(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    busy = true
    const img = await shopping.uploadImage(file)
    busy = false
    if (img) onpicked(img.id)
  }
</script>

<span class="picker">
  <button type="button" class="btn small" disabled={busy} onclick={() => camera.click()}><Icon name="camera" size={16} /> {busy ? 'Uploading…' : label}</button>
  <button type="button" class="btn small" disabled={busy} onclick={() => files.click()}><Icon name="image" size={16} /> Choose</button>
  <input bind:this={camera} type="file" accept="image/jpeg,image/png,image/webp" capture="environment" hidden onchange={picked} />
  <input bind:this={files} type="file" accept="image/jpeg,image/png,image/webp" hidden onchange={picked} />
</span>

<style>
  .picker {
    display: inline-flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
