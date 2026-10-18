<script lang="ts">
  import { dismiss, toasts } from '../toast.svelte'
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast" class:error={t.kind === 'error'}>
      <span>{t.text}</span>
      {#if t.action}
        <button
          class="action"
          onclick={() => {
            t.action!.run()
            dismiss(t.id)
          }}>{t.action.label}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: calc(var(--nav-h) + 16px + env(safe-area-inset-bottom));
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 100;
    width: min(420px, calc(100vw - 32px));
    pointer-events: none;
  }
  @media (min-width: 900px) {
    .toasts {
      bottom: 24px;
    }
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 12px;
    justify-content: space-between;
    background: var(--text);
    color: var(--bg);
    padding: 12px 16px;
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    font-size: 14px;
    animation: up 180ms ease-out;
  }
  .toast.error {
    background: var(--danger);
    color: #fff;
  }
  .action {
    font-weight: 700;
    color: inherit;
    text-decoration: underline;
  }
  @keyframes up {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }
</style>
