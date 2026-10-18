<script lang="ts">
  import { formatAmount, parse } from '../../lib/amount'
  import type { CatalogItem } from '../../lib/api/types/CatalogItem'
  import { normalizeName } from '../../lib/catalog'
  import Icon from '../../lib/components/Icon.svelte'
  import Thumb from '../../lib/components/Thumb.svelte'
  import { label } from '../../lib/products'
  import { shopping } from '../../lib/shopping.svelte'

  // Compact: inside a list card on the summary page, without the frequent-item chips.
  let { listId, shopId, compact = false, placeholder = 'Add an item, e.g. milk 2 or flour 1 kg' }: { listId: string; shopId: string | null; compact?: boolean; placeholder?: string } = $props()
  let text = $state('')
  let selected = $state(-1)
  let input: HTMLInputElement
  let scanOpen = $state(false)

  const suggestions = $derived(text.trim() ? shopping.suggest(text, shopId) : [])
  const chips = $derived(compact || text.trim() ? [] : shopping.frequentFor(listId))
  const onList = (c: CatalogItem, productId: string | null = null) => {
    const n = normalizeName(c.name)
    return shopping.itemsOf(listId).find((i) => !i.checked_at && normalizeName(i.name) === n && (i.product_id ?? null) === productId)
  }

  $effect(() => {
    // A new query starts without a highlighted suggestion.
    void text
    selected = -1
  })

  function addText() {
    const p = parse(text.trim())
    if (!p.name) return
    shopping.add(listId, { name: shopping.known(p.name)?.name ?? p.name, amount: p.amount, unit: p.unit })
    text = ''
  }

  /** Add a catalogue entry; the text stays, so tapping again adds more. */
  function pick(c: CatalogItem) {
    shopping.add(listId, { name: c.name, amount: null, unit: null })
    input.focus()
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' && suggestions.length) {
      e.preventDefault()
      selected = (selected + 1) % suggestions.length
    } else if (e.key === 'ArrowUp' && suggestions.length) {
      e.preventDefault()
      selected = selected <= 0 ? suggestions.length - 1 : selected - 1
    } else if (e.key === 'Escape' && text) {
      e.preventDefault()
      text = ''
    }
  }

  function submit(e: Event) {
    e.preventDefault()
    if (selected >= 0 && suggestions[selected]) {
      shopping.add(listId, { name: suggestions[selected].name, amount: null, unit: null })
      text = ''
    } else addText()
  }
</script>

<form class="quick" class:compact onsubmit={submit}>
  <div class="box">
    <Icon name="plus" />
    <input
      bind:this={input}
      bind:value={text}
      {onkeydown}
      {placeholder}
      aria-label={placeholder}
      autocomplete="off"
      enterkeyhint="done"
      maxlength="200"
      role="combobox"
      aria-expanded={suggestions.length > 0}
      aria-controls="suggestions-{listId}"
      aria-activedescendant={selected >= 0 ? `suggestion-${listId}-${selected}` : undefined}
    />
    {#if text}
      <button type="button" class="icon-btn" aria-label="Clear" onclick={() => ((text = ''), input.focus())}><Icon name="x" size={18} /></button>
    {:else}
      <button type="button" class="icon-btn" aria-label="Scan a barcode" title="Scan a barcode" onclick={() => (scanOpen = true)}><Icon name="barcode" size={18} /></button>
    {/if}
  </div>
  {#if suggestions.length}
    <ul id="suggestions-{listId}" role="listbox" aria-label="Suggestions">
      {#each suggestions as c, i (c.id)}
        {@const open = onList(c)}
        <li id="suggestion-{listId}-{i}" role="option" aria-selected={i === selected}>
          <button type="button" class:selected={i === selected} onclick={() => pick(c)}>
            <span class="name">{c.name}</span>
            {#if c.category_name}<span class="muted small">{c.category_name}</span>{/if}
            <span class="spacer"></span>
            {#if open}<span class="badge on">{formatAmount(open.amount, open.unit)} on list</span>{:else}<span class="muted small">{formatAmount(c.amount, c.unit)}</span>{/if}
          </button>
        </li>
        {#each shopping.productsOf(c.id).slice(0, 3) as p (p.id)}
          {@const pOpen = onList(c, p.id)}
          <li role="option" aria-selected="false" class="product">
            <button type="button" onclick={() => (shopping.add(listId, { name: c.name, amount: null, unit: null, productId: p.id }), input.focus())}>
              {#if p.image_id}<Thumb id={p.image_id} size={28} />{/if}
              <span class="name">{c.name} · {label(p.brand, p.name)}</span>
              <span class="spacer"></span>
              {#if pOpen}<span class="badge on">{formatAmount(pOpen.amount, pOpen.unit)} on list</span>{/if}
            </button>
          </li>
        {/each}
      {/each}
    </ul>
  {:else if chips.length}
    <div class="chips" aria-label="Often bought here">
      {#each chips as c (c.id)}
        <button type="button" class="chip" onclick={() => shopping.add(listId, { name: c.name, amount: null, unit: null })}><Icon name="plus" size={14} />{c.name}</button>
      {/each}
    </div>
  {/if}
</form>

{#if scanOpen}
  {#await import('../../lib/components/ScanSheet.svelte') then m}
    <m.default bind:open={scanOpen} title="Scan to add" onfound={(code: string) => shopping.addScanned(listId, code)} />
  {/await}
{/if}

<style>
  .quick {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 16px;
  }
  .compact {
    margin: 0;
  }
  .compact .box {
    border-radius: var(--radius-sm);
  }
  .compact .box input {
    min-height: 44px;
    font-size: 15px;
  }
  .box {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-left: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .box:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .box input {
    border: 0;
    background: transparent;
    min-height: 50px;
    font-size: 16px;
    color: var(--text);
  }
  .box input:focus-visible {
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  ul button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 46px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    text-align: left;
  }
  ul button:hover,
  ul button.selected {
    background: var(--surface-2);
  }
  li.product button {
    padding-left: 28px;
    min-height: 40px;
    font-size: 14px;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: 13px;
  }
  .badge.on {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 36px;
    padding: 4px 12px 4px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    font-size: 14px;
  }
  .chip:hover {
    background: var(--surface-2);
  }
</style>
