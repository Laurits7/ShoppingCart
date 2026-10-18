<script lang="ts">
  // Move something up or down without dragging (WCAG 2.5.7), next to where it's edited.
  import Icon from './Icon.svelte'

  let { what, index, count, onmove }: { what: string; index: number; count: number; onmove: (to: number) => void } = $props()
</script>

<span class="updown">
  <button type="button" class="icon-btn" aria-label="Move {what} up" disabled={index <= 0} onclick={() => onmove(index - 1)}><Icon name="up" size={18} /></button>
  <button type="button" class="icon-btn" aria-label="Move {what} down" disabled={index < 0 || index >= count - 1} onclick={() => onmove(index + 1)}><Icon name="down" size={18} /></button>
</span>

<style>
  .updown {
    display: inline-flex;
    flex: none;
  }
  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
