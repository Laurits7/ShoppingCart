<script lang="ts">
  import type { WishItem } from '../../lib/api/types/WishItem'
  import type { Wishlist } from '../../lib/api/types/Wishlist'
  import Icon from '../../lib/components/Icon.svelte'
  import Sheet from '../../lib/components/Sheet.svelte'
  import { draggable, dropList, dnd, type DragItem } from '../../lib/dnd.svelte'
  import { router } from '../../lib/router.svelte'
  import { wishes } from '../../lib/wishes.svelte'
  import ShareSheet from '../shopping/ShareSheet.svelte'
  import WishRow from './WishRow.svelte'
  import WishSheet from './WishSheet.svelte'

  let { wishlist }: { wishlist: Wishlist } = $props()
  const owner = $derived(wishlist.role === 'owner')
  const canEdit = $derived(wishlist.role !== 'viewer')
  const subtitle = $derived(
    wishlist.kind === 'gift'
      ? `Gift ideas for ${wishlist.recipient_name}${owner ? '' : ` · ${wishlist.owner_name}'s`}`
      : owner
        ? 'Your wishlist'
        : `${wishlist.owner_name}'s wishlist`,
  )

  const sections = $derived(wishes.sectionsOf(wishlist.id))
  const open = $derived(wishes.openOf(wishlist.id))
  const received = $derived(wishes.receivedOf(wishlist.id))
  const groups = $derived.by(() => {
    const known = new Set(sections.map((s) => s.id))
    // While dragging a wish, empty sections show as places to drop it.
    const showEmpty = dnd.item?.kind === 'wish'
    const out = sections.map((s) => ({ id: s.id as string | null, name: s.name, items: open.filter((i) => i.section_id === s.id) }))
    const rest = open.filter((i) => !i.section_id || !known.has(i.section_id))
    return [...out.filter((g) => g.items.length || showEmpty || canEdit), { id: null, name: sections.length ? 'Other' : '', items: rest }]
  })

  let editing = $state<WishItem | null>(null)
  let sheetOpen = $state(false)
  let menuOpen = $state(false)
  let shareOpen = $state(false)
  let showReceived = $state(false)
  let title = $state('')
  let newSection = $state('')
  const edit = (i: WishItem | null) => ((editing = i), (sheetOpen = true))

  const recipientUser = $derived(wishes.persons.find((p) => p.id === wishlist.recipient_person_id)?.user_id ?? null)

  const wishDrop = (sectionId: string | null) => ({
    accepts: (it: DragItem) => it.kind === 'wish' && it.wishlistId === wishlist.id && canEdit,
    drop: (it: DragItem, index: number) => moveWish(it.id, sectionId, index),
    keyMove: (id: string, index: number) => moveWish(id, sectionId, index),
  })
  function moveWish(id: string, sectionId: string | null, index: number) {
    const i = wishes.items.find((x) => x.id === id)
    if (i) wishes.moveItem(i, sectionId, index)
  }
  function moveSection(id: string, index: number) {
    const s = wishes.sections.find((x) => x.id === id)
    if (s) wishes.moveSection(s, index)
  }

  async function remove() {
    if (!confirm(`Delete "${wishlist.title}" and its ${open.length + received.length} wishes?`)) return
    menuOpen = false
    await wishes.deleteWishlist(wishlist)
    router.go('/wishlist', true)
  }
</script>

<a class="back" href="/wishlist"><Icon name="chevronRight" size={18} /> All wishlists</a>
<header>
  <span class="titles">
    <span class="muted small">{subtitle}</span>
    <h1>{wishlist.title}</h1>
  </span>
  <span class="tools">
    {#if canEdit}<button class="btn primary" onclick={() => edit(null)}><Icon name="plus" size={18} /> Add</button>{/if}
    <button class="icon-btn" aria-label="Wishlist settings" onclick={() => ((title = wishlist.title), (menuOpen = true))}><Icon name="more" /></button>
  </span>
</header>
{#if !wishlist.sees_claims && owner}
  <p class="muted small hint">Others can mark things "I'll get this"; you won't see which, so it stays a surprise.</p>
{/if}

{#if !open.length && !received.length}
  <div class="card empty">
    <p><strong>No wishes yet.</strong></p>
    <p class="muted">{canEdit ? 'Add things with a link, price and note. Sections help: Books, Kitchen, Hiking…' : 'Nothing here yet.'}</p>
  </div>
{/if}

<div use:dropList={{ accepts: (it) => it.kind === 'wish_section' && it.wishlistId === wishlist.id, drop: (it, i) => moveSection(it.id, i), keyMove: moveSection }}>
  {#each groups.filter((g) => g.id) as g (g.id)}
    <section class="card" data-id={g.id} data-label={g.name} use:draggable={{ item: () => ({ kind: 'wish_section', id: g.id!, wishlistId: wishlist.id }), disabled: !canEdit, handleOnly: true }}>
      <h2>
        {#if canEdit}<span class="grip" data-handle tabindex="0" role="button" aria-label="Move section {g.name}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>{/if}
        {g.name} <span class="badge">{g.items.length}</span>
      </h2>
      <ul class:empty-drop={!g.items.length} use:dropList={wishDrop(g.id)}>
        {#each g.items as item (item.id)}<WishRow {item} {wishlist} onedit={edit} />{/each}
      </ul>
    </section>
  {/each}
</div>
{#each groups.filter((g) => !g.id && (g.items.length || dnd.item?.kind === 'wish')) as g ('none')}
  <section class="card">
    {#if g.name}<h2>{g.name} <span class="badge">{g.items.length}</span></h2>{/if}
    <ul class:empty-drop={!g.items.length} use:dropList={wishDrop(null)}>
      {#each g.items as item (item.id)}<WishRow {item} {wishlist} onedit={edit} />{/each}
    </ul>
  </section>
{/each}

{#if received.length}
  <section class="card archive">
    <h2><button class="link" aria-expanded={showReceived} onclick={() => (showReceived = !showReceived)}>Received <span class="badge">{received.length}</span></button></h2>
    {#if showReceived}
      <ul>{#each received as item (item.id)}<WishRow {item} {wishlist} onedit={edit} />{/each}</ul>
    {/if}
  </section>
{/if}

<WishSheet bind:open={sheetOpen} {wishlist} item={editing} />

<Sheet bind:open={menuOpen} title={wishlist.title}>
  {#if owner}
    <form class="row" onsubmit={(e) => (e.preventDefault(), title.trim() && wishes.renameWishlist(wishlist, title.trim()))}>
      <input bind:value={title} aria-label="Title" maxlength="200" />
      <button class="btn" type="submit">Rename</button>
    </form>
  {/if}
  <div class="actions">
    <button class="btn" onclick={() => ((menuOpen = false), (shareOpen = true))}>{owner ? 'Share…' : 'Who can see it…'}</button>
    {#if owner}<span class="spacer"></span><button class="btn danger" onclick={remove}>Delete wishlist</button>{/if}
  </div>
  {#if canEdit}
    <h3>Sections</h3>
    <ul class="sections">
      {#each sections as s (s.id)}
        <li>
          <input
            value={s.name}
            aria-label="Section name"
            maxlength="200"
            onchange={(e) => {
              const n = e.currentTarget.value.trim()
              if (n && n !== s.name) wishes.renameSection(s, n)
            }}
          />
          <button class="icon-btn" aria-label="Delete section {s.name}" onclick={() => wishes.deleteSection(s)}><Icon name="trash" size={18} /></button>
        </li>
      {/each}
    </ul>
    <form class="row" onsubmit={(e) => (e.preventDefault(), newSection.trim() && (wishes.createSection(wishlist.id, newSection.trim()), (newSection = '')))}>
      <input bind:value={newSection} placeholder="New section, e.g. Books" aria-label="New section" maxlength="200" />
      <button class="btn" type="submit">Add</button>
    </form>
  {/if}
</Sheet>

<ShareSheet
  bind:open={shareOpen}
  kind="wishlist"
  target={{ id: wishlist.id, name: wishlist.title, role: wishlist.role, owner_name: wishlist.owner_name }}
  hide={recipientUser}
/>

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    margin-bottom: 6px;
  }
  .back :global(svg) {
    transform: rotate(180deg);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 12px;
  }
  .titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  h1 {
    overflow-wrap: anywhere;
  }
  .tools {
    display: flex;
    gap: 4px;
    flex: none;
  }
  .small {
    font-size: 13px;
  }
  .hint {
    margin: -4px 0 12px;
  }
  .card {
    margin-bottom: 12px;
    overflow: hidden;
  }
  .empty {
    padding: 20px;
  }
  .empty p {
    margin: 0 0 6px;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    padding: 10px 12px 8px 4px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  ul.empty-drop {
    min-height: 48px;
    border-top: 1px dashed var(--border);
  }
  .archive {
    background: var(--surface-2);
  }
  .link {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    color: var(--muted);
    padding-left: 8px;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  h3 {
    font-size: 14px;
    margin: 8px 0 0;
  }
  .sections li {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }
</style>
