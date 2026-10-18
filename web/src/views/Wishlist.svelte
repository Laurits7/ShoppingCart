<script lang="ts">
  // The Wishlist tab (SPEC §6.9): my wishlists, wishlists others share with me, and gift
  // ideas per person. A wishlist opens at /wishlist/{id}.
  import { api, message } from '../lib/api/client'
  import type { UserSummary } from '../lib/api/types/UserSummary'
  import type { Wishlist } from '../lib/api/types/Wishlist'
  import Icon from '../lib/components/Icon.svelte'
  import Sheet from '../lib/components/Sheet.svelte'
  import { draggable, dropList } from '../lib/dnd.svelte'
  import { router } from '../lib/router.svelte'
  import { session } from '../lib/session.svelte'
  import { shopping } from '../lib/shopping.svelte'
  import { wishes } from '../lib/wishes.svelte'
  import WishlistView from './wishlist/WishlistView.svelte'

  let { wishlistId }: { wishlistId?: string } = $props()
  let error = $state('')
  if (!shopping.loaded) shopping.load().catch((e) => (error = message(e)))

  const current = $derived(wishlistId ? wishes.wishlist(wishlistId) : undefined)
  const persons = $derived([...wishes.persons].sort((a, b) => a.name.localeCompare(b.name)))
  const openCount = (w: Wishlist) => wishes.openOf(w.id).length
  const takenCount = (w: Wishlist) => (w.sees_claims ? wishes.openOf(w.id).filter((i) => wishes.claimOf(i.id)).length : 0)

  // New wishlist / gift list / person.
  let newOwn = $state('')
  let giftFor = $state<string | null>(null)
  let giftTitle = $state('')
  let personOpen = $state(false)
  let personName = $state('')
  let personUser = $state('')
  let directory = $state<UserSummary[]>([])

  async function createOwn(e: Event) {
    e.preventDefault()
    const t = newOwn.trim()
    if (!t) return
    const w = await wishes.createWishlist('own', t, null)
    newOwn = ''
    if (w) router.go(`/wishlist/${w.id}`)
  }

  async function createGift(e: Event) {
    e.preventDefault()
    const t = giftTitle.trim()
    if (!t || !giftFor) return
    const w = await wishes.createWishlist('gift', t, giftFor)
    giftTitle = ''
    giftFor = null
    if (w) router.go(`/wishlist/${w.id}`)
  }

  async function openPerson() {
    personOpen = true
    personName = personUser = ''
    directory = await api.get<UserSummary[]>('/users/directory').catch(() => [])
  }

  async function createPerson(e: Event) {
    e.preventDefault()
    const n = personName.trim()
    if (!n) return
    const p = await wishes.createPerson(n, personUser || null)
    if (p) {
      personOpen = false
      giftFor = p.id
      giftTitle = `Ideas for ${p.name}`
    }
  }

  function moveMine(id: string, index: number) {
    const w = wishes.wishlist(id)
    if (w) wishes.moveWishlist(w, index)
  }
</script>

{#snippet card(w: Wishlist, movable: boolean)}
  <a
    class="card wl"
    href="/wishlist/{w.id}"
    data-id={movable ? w.id : undefined}
    data-label={w.title}
    use:draggable={{ item: () => ({ kind: 'wishlist', id: w.id }), disabled: !movable, handleOnly: true }}
  >
    {#if movable}<span class="grip" data-handle tabindex="0" role="button" aria-label="Move {w.title}" aria-roledescription="draggable"><Icon name="grip" size={16} /></span>{/if}
    <span class="text">
      <strong>{w.title}</strong>
      <span class="muted small">
        {w.owner_id !== session.me?.id ? `${w.owner_name} · ` : ''}{openCount(w)} wish{openCount(w) === 1 ? '' : 'es'}{#if takenCount(w)}
          · {takenCount(w)} taken{/if}{w.role === 'viewer' ? ' · view only' : ''}
      </span>
    </span>
    <Icon name="chevronRight" size={18} />
  </a>
{/snippet}

{#if error}
  <div class="card pad"><p>Could not load: {error}</p></div>
{:else if !shopping.loaded}
  <p class="muted" aria-busy="true">Loading…</p>
{:else if wishlistId}
  {#if current}
    {#key current.id}<WishlistView wishlist={current} />{/key}
  {:else}
    <h1>Wishlist not found</h1>
    <p class="muted">It may have been deleted or is no longer shared with you. <a href="/wishlist">All wishlists</a></p>
  {/if}
{:else}
  <h1 class="title">Wishlist</h1>

  <section>
    <h2>My wishlists</h2>
    <div use:dropList={{ accepts: (it) => it.kind === 'wishlist', drop: (it, i) => moveMine(it.id, i), keyMove: moveMine }}>
      {#each wishes.mine as w (w.id)}{@render card(w, true)}{/each}
    </div>
    {#if !wishes.mine.length}<p class="muted small">Things you'd like, by topic. Private until you share them, e.g. with your household.</p>{/if}
    <form class="row" onsubmit={createOwn}>
      <input bind:value={newOwn} placeholder="New wishlist, e.g. Birthday" aria-label="New wishlist" maxlength="200" />
      <button class="btn" type="submit">Create</button>
    </form>
  </section>

  {#if wishes.others.length}
    <section>
      <h2>From others</h2>
      {#each wishes.others as w (w.id)}{@render card(w, false)}{/each}
    </section>
  {/if}

  <section>
    <h2>Gift ideas</h2>
    <p class="muted small">Ideas for other people. A gift list is never shown to the person it's for, even if you share it with a household they're in.</p>
    {#each wishes.gifts as [name, lists] (name)}
      <h3>{name}</h3>
      {#each lists as w (w.id)}{@render card(w, false)}{/each}
    {/each}
    {#if giftFor}
      <form class="row" onsubmit={createGift}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={giftTitle} placeholder="Title" aria-label="Gift list title" maxlength="200" autofocus />
        <button class="btn primary" type="submit">Create</button>
        <button class="btn" type="button" onclick={() => (giftFor = null)}>Cancel</button>
      </form>
    {:else}
      <div class="row">
        <select aria-label="New gift list for" onchange={(e) => {
          const v = e.currentTarget.value
          e.currentTarget.value = ''
          if (v === '+') openPerson()
          else if (v) {
            giftFor = v
            giftTitle = `Ideas for ${persons.find((p) => p.id === v)?.name ?? ''}`
          }
        }}>
          <option value="">New gift list for…</option>
          {#each persons as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
          <option value="+">Someone new…</option>
        </select>
      </div>
    {/if}
  </section>
{/if}

<Sheet bind:open={personOpen} title="Someone to give to">
  <form class="fields" onsubmit={createPerson}>
    <label class="field wide"><span>Name</span><input bind:value={personName} placeholder="Grandma" maxlength="200" required /></label>
    <label class="field wide">
      <span>Their account here (optional)</span>
      <select bind:value={personUser}>
        <option value="">No account, or don't link</option>
        {#each directory.filter((u) => u.id !== session.me?.id) as u (u.id)}<option value={u.id}>{u.display_name} (@{u.username})</option>{/each}
      </select>
    </label>
    <p class="muted small wide">Linking their account makes sure gift lists about them stay hidden from them, wherever you share them.</p>
    <div class="actions wide"><button class="btn primary" type="submit">Add</button></div>
  </form>
</Sheet>

<style>
  .title {
    margin-bottom: 12px;
  }
  section {
    margin-bottom: 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h3 {
    font-size: 14px;
    color: var(--muted);
    margin: 8px 0 0;
  }
  .wl {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 56px;
    padding: 8px 12px;
    margin-bottom: 8px;
    color: inherit;
    text-decoration: none;
  }
  .wl:hover {
    background: var(--surface-2);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .small {
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .pad {
    padding: 20px;
  }
</style>
