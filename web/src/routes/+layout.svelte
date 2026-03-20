<script lang="ts">
  import '../app.css';
  import { auth } from '$lib/auth.svelte';
  import { logout as apiLogout, getHealth, getMe, type HealthInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';

  let { children } = $props();
  let buildInfo = $state<HealthInfo | null>(null);
  let menuOpen = $state(false);

  // Close menu on navigation
  $effect(() => {
    $page.url.pathname;
    menuOpen = false;
  });

  $effect(() => {
    if (!browser) return;
    getHealth().then(h => { buildInfo = h; });
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

  function isActive(path: string): boolean {
    if (path === '/') return $page.url.pathname === '/';
    return $page.url.pathname.startsWith(path);
  }
</script>

<div class="min-h-screen bg-bg text-text">
  <!-- Nav -->
  <nav class="border-b border-border bg-surface sticky top-0 z-50">
    <div class="max-w-6xl mx-auto px-4 h-14 flex items-center justify-between">
      <!-- Left: logo + docs -->
      <div class="flex items-center gap-3">
        <a href="/" class="flex items-center gap-2 text-brand font-bold text-lg hover:opacity-80 transition-opacity">
          <img src="/logo.svg" alt="" class="h-7 w-auto invert" />
          rpodder
        </a>
        <a href="https://thekoma.github.io/rpodder/" target="_blank" rel="noopener noreferrer"
          class="text-text-dim hover:text-text transition-colors" title="Documentation">
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
        </a>
      </div>

      {#if auth.loggedIn}
        <!-- Desktop nav -->
        <div class="hidden md:flex items-center gap-4">
          <a href="/discover" class="text-sm transition-colors {isActive('/discover') ? 'text-brand' : 'text-text-dim hover:text-text'}">Discover</a>
          <a href="/subscriptions" class="text-sm transition-colors {isActive('/subscriptions') ? 'text-brand' : 'text-text-dim hover:text-text'}">Subscriptions</a>
          <a href="/history" class="text-sm transition-colors {isActive('/history') ? 'text-brand' : 'text-text-dim hover:text-text'}">History</a>
          <a href="/devices" class="text-sm transition-colors {isActive('/devices') ? 'text-brand' : 'text-text-dim hover:text-text'}">Devices</a>
          {#if auth.isAdmin}
            <a href="/admin" class="text-sm transition-colors {isActive('/admin') ? 'text-brand' : 'text-text-dim hover:text-text'}">Admin</a>
          {/if}
          <div class="flex items-center gap-2 ml-2 pl-4 border-l border-border">
            <a href="/settings" class="text-sm text-text-dim hover:text-text transition-colors">{auth.username}</a>
            {#if auth.isAdmin}
              <span class="text-xs px-1.5 py-0.5 rounded-full bg-brand-dim text-brand">admin</span>
            {/if}
            <button onclick={handleLogout}
              class="text-xs text-danger hover:text-red-400 transition-colors cursor-pointer">Logout</button>
          </div>
        </div>

        <!-- Mobile hamburger -->
        <button onclick={() => { menuOpen = !menuOpen; }}
          class="md:hidden p-2 text-text-dim hover:text-text transition-colors cursor-pointer"
          aria-label="Menu">
          {#if menuOpen}
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
          {:else}
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5" /></svg>
          {/if}
        </button>
      {/if}
    </div>

    <!-- Mobile menu dropdown -->
    {#if menuOpen && auth.loggedIn}
      <div class="md:hidden border-t border-border bg-surface">
        <div class="max-w-6xl mx-auto px-4 py-3 space-y-1">
          <a href="/discover" class="block px-3 py-2 rounded-lg text-sm transition-colors {isActive('/discover') ? 'bg-brand-dim text-brand' : 'text-text-dim hover:bg-surface-hover hover:text-text'}">Discover</a>
          <a href="/subscriptions" class="block px-3 py-2 rounded-lg text-sm transition-colors {isActive('/subscriptions') ? 'bg-brand-dim text-brand' : 'text-text-dim hover:bg-surface-hover hover:text-text'}">Subscriptions</a>
          <a href="/history" class="block px-3 py-2 rounded-lg text-sm transition-colors {isActive('/history') ? 'bg-brand-dim text-brand' : 'text-text-dim hover:bg-surface-hover hover:text-text'}">History</a>
          <a href="/devices" class="block px-3 py-2 rounded-lg text-sm transition-colors {isActive('/devices') ? 'bg-brand-dim text-brand' : 'text-text-dim hover:bg-surface-hover hover:text-text'}">Devices</a>
          {#if auth.isAdmin}
            <a href="/admin" class="block px-3 py-2 rounded-lg text-sm transition-colors {isActive('/admin') ? 'bg-brand-dim text-brand' : 'text-text-dim hover:bg-surface-hover hover:text-text'}">Admin</a>
          {/if}
          <div class="border-t border-border mt-2 pt-2 flex items-center justify-between px-3 py-2">
            <div class="flex items-center gap-2">
              <a href="/settings" class="text-sm text-text-dim hover:text-text transition-colors">{auth.username}</a>
              {#if auth.isAdmin}
                <span class="text-xs px-1.5 py-0.5 rounded-full bg-brand-dim text-brand">admin</span>
              {/if}
            </div>
            <button onclick={handleLogout}
              class="text-xs text-danger hover:text-red-400 transition-colors cursor-pointer">Logout</button>
          </div>
        </div>
      </div>
    {/if}
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
