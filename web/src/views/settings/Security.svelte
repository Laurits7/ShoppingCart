<script lang="ts">
  import { api, message } from '../../lib/api/client'
  import type { SessionInfo } from '../../lib/api/types/SessionInfo'
  import { toast } from '../../lib/toast.svelte'

  let current = $state('')
  let next = $state('')
  let sessions = $state<SessionInfo[]>([])

  async function loadSessions() {
    try {
      sessions = await api.get<SessionInfo[]>('/sessions')
    } catch (err) {
      toast(message(err), 'error')
    }
  }
  loadSessions()

  async function changePassword(e: Event) {
    e.preventDefault()
    try {
      await api.post('/me/password', { current_password: current, new_password: next })
      current = next = ''
      toast('Password changed. Other devices are signed out and API tokens revoked.')
      loadSessions()
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  async function revoke(s: SessionInfo) {
    try {
      await api.del(`/sessions/${s.id}`)
      sessions = sessions.filter((x) => x.id !== s.id)
    } catch (err) {
      toast(message(err), 'error')
    }
  }

  /** "Firefox on Linux"-style label from a user agent. */
  function device(ua: string | null): string {
    if (!ua) return 'Unknown device'
    const browser = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'Browser'
    const os = /Android/.test(ua) ? 'Android' : /iPhone|iPad/.test(ua) ? 'iOS' : /Windows/.test(ua) ? 'Windows' : /Mac OS/.test(ua) ? 'macOS' : /Linux/.test(ua) ? 'Linux' : ''
    return os ? `${browser} on ${os}` : browser
  }
  const when = (iso: string) => new Date(iso).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })
</script>

<section class="card settings-section">
  <h2>Password</h2>
  <form class="row" onsubmit={changePassword}>
    <input type="password" aria-label="Current password" placeholder="Current password" autocomplete="current-password" bind:value={current} required />
    <input type="password" aria-label="New password" placeholder="New password (10+ characters)" autocomplete="new-password" minlength="10" bind:value={next} required />
    <button class="btn" type="submit">Change</button>
  </form>
</section>

<section class="card settings-section">
  <h2>Signed-in devices</h2>
  <ul>
    {#each sessions as s (s.id)}
      <li>
        <span>
          {device(s.user_agent)}{#if s.current} <strong>(this device)</strong>{/if}
          <br /><span class="muted small">Last active {when(s.last_seen_at)}</span>
        </span>
        {#if !s.current}<button class="btn small" onclick={() => revoke(s)}>Sign out</button>{/if}
      </li>
    {/each}
  </ul>
</section>
