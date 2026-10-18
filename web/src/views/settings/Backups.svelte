<script lang="ts">
  // Admins: the database backups in data/backups (SPEC §6.14).
  import { api, message } from '../../lib/api/client'
  import type { Backup } from '../../lib/api/types/Backup'
  import { toast } from '../../lib/toast.svelte'

  let backups = $state<Backup[]>([])
  let busy = $state(false)
  api.get<Backup[]>('/admin/backups').then((b) => (backups = b), (e) => toast(message(e), 'error'))

  async function now() {
    busy = true
    try {
      backups = await api.post<Backup[]>('/admin/backups')
      toast('Backup made')
    } catch (e) {
      toast(message(e), 'error')
    }
    busy = false
  }

  const size = (b: number) => (b > 1024 * 1024 ? `${(b / 1024 / 1024).toFixed(1)} MB` : `${Math.ceil(b / 1024)} KB`)
</script>

<section class="card settings-section">
  <h2>Backups</h2>
  <p class="muted small">
    A copy of the database is made every night in <code>data/backups</code>. Pictures and the key for stored tokens are in the <code>data</code> folder too:
    back up the whole folder to keep everything.
  </p>
  <ul>
    {#each backups as b (b.name)}
      <li>
        <span>{new Date(b.created_at).toLocaleString()} <span class="muted small">{size(b.bytes)}</span></span>
        <a class="btn small" href="/api/v1/admin/backups/{b.name}" download>Download</a>
      </li>
    {:else}
      <li class="muted small">None yet.</li>
    {/each}
  </ul>
  <div class="row"><button class="btn" onclick={now} disabled={busy}>{busy ? 'Backing up…' : 'Back up now'}</button></div>
</section>
