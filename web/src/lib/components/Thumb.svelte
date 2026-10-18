<script lang="ts">
  // A small square picture from our server (never a third-party address).
  import { imageUrl } from '../products'

  let { id, size = 36, alt = '' }: { id: string; size?: number; alt?: string } = $props()
</script>

<img src={imageUrl(id)} width={size} height={size} {alt} loading="lazy" decoding="async" style:width="{size}px" style:height="{size}px" />

<style>
  img {
    flex: none;
    object-fit: cover;
    border-radius: 6px;
    background: var(--surface-2);
  }
</style>
