<script lang="ts">
  import '../app.css';
  import { auth } from '$lib/auth.svelte';
  import { logout as apiLogout, getHealth, getMe, type HealthInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let { children } = $props();
  let buildInfo = $state<HealthInfo | null>(null);

  $effect(() => {
    if (!browser) return;
    getHealth().then(h => { buildInfo = h; });
    // Refresh admin status from server
    if (auth.loggedIn) {
      getMe().then(me => {
        if (me) auth.setAdmin(me.is_admin);
      });
    }
  });

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
          <a href="/history" class="text-sm text-text-dim hover:text-text transition-colors">History</a>
          <a href="/devices" class="text-sm text-text-dim hover:text-text transition-colors">Devices</a>
          {#if auth.isAdmin}
            <a href="/admin" class="text-sm text-text-dim hover:text-text transition-colors">Admin</a>
          {/if}
          <div class="flex items-center gap-2 ml-2 pl-4 border-l border-border">
            <a href="/settings" class="text-sm text-text-dim hover:text-text transition-colors">{auth.username}</a>
            {#if auth.isAdmin}
              <span class="text-xs px-1.5 py-0.5 rounded-full bg-brand-dim text-brand">admin</span>
            {/if}
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

  <!-- Footer with build info -->
  {#if buildInfo}
    <footer class="border-t border-border mt-8 py-4 text-center text-xs text-text-dim">
      rpodder v{buildInfo.version}
      {#if buildInfo.build_tag !== 'dev'}
        · <span class="text-brand">{buildInfo.build_tag}</span>
      {/if}
      {#if buildInfo.build_sha !== 'local' && buildInfo.build_sha !== 'unknown'}
        · <code class="text-text-dim/60">{buildInfo.build_sha.substring(0, 7)}</code>
      {/if}
      · {buildInfo.database}
    </footer>
  {/if}
</div>
