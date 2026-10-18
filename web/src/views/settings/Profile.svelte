<script lang="ts">
  import { api, message } from '../../lib/api/client'
  import type { Me } from '../../lib/api/types/Me'
  import { session, type Theme } from '../../lib/session.svelte'
  import { toast } from '../../lib/toast.svelte'

  let displayName = $state(session.me?.display_name ?? '')

  async function saveName(e: Event) {
    e.preventDefault()
    try {
      session.me = await api.patch<Me>('/me', { display_name: displayName })
      toast('Name saved')
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function setTheme(t: Theme) {
    try {
      await session.setPref('theme', t === 'system' ? null : t)
    } catch (err) {
      toast(message(err), 'error')
    }
  }
</script>

<section class="card settings-section">
  <h2>Profile</h2>
  <p class="muted small">Signed in as <strong>{session.me?.username}</strong>{session.me?.is_admin ? ' (admin)' : ''}.</p>
  <form class="row" onsubmit={saveName}>
    <input aria-label="Your name" bind:value={displayName} maxlength="100" required />
    <button class="btn" type="submit" disabled={displayName.trim() === session.me?.display_name}>Save</button>
  </form>
  <label>
    <span>Theme</span>
    <select value={session.theme} onchange={(e) => setTheme(e.currentTarget.value as Theme)}>
      <option value="system">Same as the device</option>
      <option value="light">Light</option>
      <option value="dark">Dark</option>
    </select>
  </label>
</section>
