<script lang="ts">
  import { api, message } from '../../lib/api/client'
  import type { Me } from '../../lib/api/types/Me'
  import { session } from '../../lib/session.svelte'
  import { toast } from '../../lib/toast.svelte'

  let users = $state<Me[]>([])
  let username = $state('')
  let displayName = $state('')
  let password = $state('')

  api.get<Me[]>('/users').then(
    (u) => (users = u),
    (e) => toast(message(e), 'error'),
  )

  async function create(e: Event) {
    e.preventDefault()
    try {
      const u = await api.post<Me>('/users', { username, password, display_name: displayName || null })
      users = [...users, u].sort((a, b) => a.username.localeCompare(b.username))
      username = displayName = password = ''
      toast(`Created ${u.username}. Give them their password.`)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function patch(u: Me, body: Record<string, unknown>, done: string) {
    try {
      const updated = await api.patch<Me>(`/users/${u.id}`, body)
      users = users.map((x) => (x.id === u.id ? updated : x))
      toast(done)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  function resetPassword(u: Me) {
    const pw = prompt(`New password for ${u.username} (at least 10 characters). They will be signed out everywhere.`)
    if (pw) patch(u, { password: pw }, 'Password reset')
  }

  async function deactivate(u: Me) {
    if (!confirm(`Deactivate ${u.username}? They can no longer sign in.`)) return
    try {
      await api.del(`/users/${u.id}`)
      users = users.filter((x) => x.id !== u.id)
    } catch (err) {
      toast(message(err), 'error')
    }
  }
</script>

<section class="card settings-section">
  <h2>Users</h2>
  <p class="muted small">Only admins see this. There is no sign-up: create accounts here and hand out the passwords.</p>
  <ul>
    {#each users as u (u.id)}
      <li>
        <span>{u.display_name} <span class="muted small">@{u.username}{u.is_admin ? ' · admin' : ''}</span></span>
        {#if u.id !== session.me?.id}
          <span class="row">
            <button class="btn small" onclick={() => resetPassword(u)}>Reset password</button>
            <button class="btn small" onclick={() => patch(u, { is_admin: !u.is_admin }, u.is_admin ? 'No longer admin' : 'Now an admin')}>{u.is_admin ? 'Remove admin' : 'Make admin'}</button>
            <button class="btn small danger" onclick={() => deactivate(u)}>Deactivate</button>
          </span>
        {/if}
      </li>
    {/each}
  </ul>
  <form class="grid" onsubmit={create}>
    <input aria-label="Username" placeholder="Username" autocapitalize="none" spellcheck="false" autocomplete="off" bind:value={username} required />
    <input aria-label="Name" placeholder="Name (optional)" maxlength="100" bind:value={displayName} />
    <input aria-label="Password" type="password" placeholder="Password (10+ characters)" autocomplete="new-password" minlength="10" bind:value={password} required />
    <button class="btn primary" type="submit">Create user</button>
  </form>
</section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 8px;
  }
</style>
