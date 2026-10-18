<script lang="ts">
  // A modal sheet: slides up from the bottom on phones, a centred dialog on wide screens.
  // Built on <dialog>, so focus stays inside, Escape closes and the page behind is inert.
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'

  let { open = $bindable(false), title, children }: { open: boolean; title: string; children: Snippet } = $props()
  let dlg: HTMLDialogElement

  $effect(() => {
    if (open && !dlg.open) dlg.showModal()
    else if (!open && dlg.open) dlg.close()
  })
</script>

<dialog bind:this={dlg} aria-label={title} onclose={() => (open = false)} onclick={(e) => e.target === dlg && dlg.close()}>
  {#if open}
    <div class="sheet" data-autoscroll>
      <header>
        <h2>{title}</h2>
        <button class="icon-btn" aria-label="Close" onclick={() => dlg.close()}><Icon name="x" /></button>
      </header>
      {@render children()}
    </div>
  {/if}
</dialog>

<style>
  dialog {
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    width: 100%;
    max-width: 560px;
    max-height: 100dvh;
    margin: auto auto 0;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  .sheet {
    background: var(--surface);
    border-radius: var(--radius) var(--radius) 0 0;
    padding: 8px 16px calc(20px + env(safe-area-inset-bottom));
    max-height: 88dvh;
    overflow-y: auto;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    position: sticky;
    top: -8px;
    background: var(--surface);
    padding: 8px 0 4px;
    z-index: 1;
  }
  @media (min-width: 640px) {
    dialog {
      margin: auto;
    }
    .sheet {
      border-radius: var(--radius);
      padding-bottom: 20px;
    }
  }
</style>
