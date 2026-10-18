<script lang="ts">
  import { session } from '../lib/session.svelte'
</script>

<h1>Shopping</h1>
<div class="card empty">
  <p>Hi {session.me?.display_name}! Shops, lists and categories arrive in the next phase.</p>
  <p class="muted">Meanwhile you can set up people and households in <a href="/settings">Settings</a>.</p>
</div>

<style>
  h1 {
    margin-bottom: 16px;
  }
  .empty {
    padding: 24px;
  }
  p {
    margin: 0 0 8px;
  }
</style>
