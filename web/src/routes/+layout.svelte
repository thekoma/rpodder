<script lang="ts">
  import '../app.css';
  import { auth } from '$lib/auth.svelte';
  import { logout as apiLogout } from '$lib/api';
  import { goto } from '$app/navigation';

  let { children } = $props();

  async function handleLogout() {
    if (auth.username) {
      await apiLogout(auth.username);
    }
    auth.logout();
    goto('/login');
  }
</script>

<div class="min-h-screen bg-bg text-text">
  <!-- Nav -->
  <nav class="border-b border-border bg-surface sticky top-0 z-50">
    <div class="max-w-6xl mx-auto px-4 h-14 flex items-center justify-between">
      <a href="/" class="flex items-center gap-2 text-brand font-bold text-lg hover:opacity-80 transition-opacity">
        <span class="text-2xl">🎧</span>
        rpodder
      </a>

      {#if auth.loggedIn}
        <div class="flex items-center gap-4">
          <a href="/discover" class="text-sm text-text-dim hover:text-text transition-colors">Discover</a>
          <a href="/subscriptions" class="text-sm text-text-dim hover:text-text transition-colors">Subscriptions</a>
          <a href="/devices" class="text-sm text-text-dim hover:text-text transition-colors">Devices</a>
          <div class="flex items-center gap-2 ml-2 pl-4 border-l border-border">
            <span class="text-sm text-text-dim">{auth.username}</span>
            <button
              onclick={handleLogout}
              class="text-xs text-danger hover:text-red-400 transition-colors cursor-pointer"
            >
              Logout
            </button>
          </div>
        </div>
      {/if}
    </div>
  </nav>

  <!-- Content -->
  <main class="max-w-6xl mx-auto px-4 py-6">
    {@render children()}
  </main>
</div>
