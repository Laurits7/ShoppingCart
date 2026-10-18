<script lang="ts">
  // The large picture of an item, with the credit Open Food Facts pictures need (CC BY-SA).
  import { imageUrl } from '../products'
  import { shopping } from '../shopping.svelte'
  import Sheet from './Sheet.svelte'

  let { open = $bindable(false), image, title, note }: { open: boolean; image: string | null; title: string; note?: string | null } = $props()
  const credit = $derived(image ? shopping.credit(image) : null)
</script>

<Sheet bind:open {title}>
  {#if image}
    <img src={imageUrl(image, true)} alt={title} />
    {#if note}<p class="muted small">{note}</p>{/if}
    {#if credit}
      <p class="muted small">
        Picture from <a href={credit} target="_blank" rel="noopener noreferrer">Open Food Facts</a>,
        <a href="https://creativecommons.org/licenses/by-sa/3.0/" target="_blank" rel="noopener noreferrer">CC BY-SA</a>.
      </p>
    {/if}
  {/if}
</Sheet>

<style>
  img {
    width: 100%;
    max-height: 60dvh;
    object-fit: contain;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 13px;
  }
</style>
