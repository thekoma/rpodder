<script lang="ts">
  import '../app.css';
  import {
    LibraryBig, Compass, History, MonitorSmartphone, ShieldCheck, Settings, BookOpen, LogOut, ChevronDown, CircleUserRound,
  } from '@lucide/svelte';
  import { auth } from '#lib/auth.svelte.js';
  import { logout as apiLogout, getBuildInfo, getMe, type BuildInfo } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import { page } from '$app/state';
  import MiniPlayer from '#lib/components/MiniPlayer.svelte';
  import Toasts from '#lib/components/Toasts.svelte';
  import ThemeSwitch from '#lib/components/ThemeSwitch.svelte';
  import { player } from '#lib/player.svelte.js';
  import { localClear } from '#lib/positions.js';
  import { getWebSettings, peekWebSettings } from '#lib/webDevice.js';

  let { children } = $props();
  let buildInfo = $state<BuildInfo | null>(null);
  let accountMenu = $state<HTMLElement | null>(null);

  const DOCS_URL = 'https://thekoma.github.io/rpodder/';

  const nav = [
    { href: '/subscriptions', label: 'Library', icon: LibraryBig, match: (p: string) => p === '/' || p.startsWith('/subscriptions') },
    { href: '/discover', label: 'Discover', icon: Compass, match: (p: string) => p.startsWith('/discover') },
    { href: '/history', label: 'History', icon: History, match: (p: string) => p.startsWith('/history') },
    { href: '/devices', label: 'Devices', icon: MonitorSmartphone, match: (p: string) => p.startsWith('/devices') },
  ];

  let path = $derived(page.url.pathname);
  let authPage = $derived(['/login', '/register', '/reset-password'].includes(path));
  let hasPlayer = $derived(auth.loggedIn && !!player.episode);

  // Close the account menu on navigation.
  $effect(() => {
    path;
    accountMenu?.hidePopover?.();
  });

  $effect(() => {
    if (!browser) return;
    if (auth.loggedIn) {
      getMe().then(me => {
        if (me) auth.setAdmin(me.is_admin);
      });
      getBuildInfo().then(h => { buildInfo = h; });
    } else {
      buildInfo = null;
    }
  });

  async function syncSetting(username: string): Promise<boolean | null> {
    const cached = peekWebSettings(username);
    if (cached) return cached.syncPositions;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const timeout = new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('timeout')), 2000); });
      return (await Promise.race([getWebSettings(username), timeout])).syncPositions;
    } catch {
      return null;
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  async function handleLogout() {
    accountMenu?.hidePopover?.();
    const username = auth.username;
    if (username) {
      const sync = await syncSetting(username);
      await player.close({ timeoutMs: 3000 });
      // D7: with sync off the local positions are the only copy, keep them.
      if (sync === true) localClear(username);
      await apiLogout(username);
    }
    auth.logout();
    goto('/login');
  }
</script>

<div class="min-h-dvh flex flex-col bg-ground text-ink">
  <!-- Shell bar -->
  <header class="sticky top-0 z-50 bg-shell text-shell-ink">
    <div class="max-w-6xl mx-auto h-14 px-4 flex items-center gap-6">
      <a href={auth.loggedIn ? '/subscriptions' : '/'} class="flex items-center gap-2 shrink-0 hover:opacity-90 transition-opacity">
        <img src="/logo.svg" alt="" class="h-7 w-auto invert" />
        <span class="text-[1.0625rem] font-bold tracking-[-0.01em]">rpodder</span>
      </a>

      {#if auth.loggedIn}
        <nav aria-label="Main" class="hidden md:flex items-stretch h-full gap-1">
          {#each nav as item (item.href)}
            {@const active = item.match(path)}
            <a href={item.href} aria-current={active ? 'page' : undefined}
              class="relative flex items-center px-3 text-sm font-semibold transition-colors
                {active ? 'text-shell-ink' : 'text-shell-ink-2 hover:text-shell-ink'}">
              {item.label}
              {#if active}<span class="absolute left-3 right-3 bottom-0 h-[3px] bg-red rounded-t-[2px]"></span>{/if}
            </a>
          {/each}
          {#if auth.isAdmin}
            {@const active = path.startsWith('/admin')}
            <a href="/admin" aria-current={active ? 'page' : undefined}
              class="relative flex items-center px-3 text-sm font-semibold transition-colors
                {active ? 'text-shell-ink' : 'text-shell-ink-2 hover:text-shell-ink'}">
              Admin
              {#if active}<span class="absolute left-3 right-3 bottom-0 h-[3px] bg-red rounded-t-[2px]"></span>{/if}
            </a>
          {/if}
        </nav>

        <div class="ml-auto">
          <button popovertarget="account-menu"
            class="flex items-center gap-2 h-9 pl-2 pr-2.5 rounded-[6px] text-sm font-semibold text-shell-ink hover:bg-shell-2 transition-colors cursor-pointer">
            <CircleUserRound class="w-5 h-5 text-shell-ink-2" />
            <span class="max-w-[10rem] truncate">{auth.username}</span>
            <ChevronDown class="w-4 h-4 text-shell-ink-2" />
          </button>
        </div>
      {:else if !authPage}
        <a href="/login" class="ml-auto btn btn-sm bg-shell-ink text-shell hover:opacity-90">Sign in</a>
      {/if}
    </div>
  </header>

  {#if auth.loggedIn}
    <div id="account-menu" popover bind:this={accountMenu}
      class="m-0 fixed inset-auto top-14 right-3 sm:right-[max(0.75rem,calc((100vw-72rem)/2+0.75rem))] w-72 p-0 rounded-[6px] border border-rule bg-panel text-ink shadow-[var(--shadow-lift)]">
      <div class="px-4 pt-3 pb-3 border-b border-rule">
        <p class="font-bold truncate">{auth.username}</p>
        <p class="text-xs text-ink-3">{auth.isAdmin ? 'Administrator' : 'Member'}</p>
      </div>
      <div class="p-1.5">
        <a href="/settings" class="flex items-center gap-2.5 px-2.5 h-9 rounded-[4px] text-sm hover:bg-panel-2">
          <Settings class="w-4 h-4 text-ink-2" /> Settings
        </a>
        {#if auth.isAdmin}
          <a href="/admin" class="md:hidden flex items-center gap-2.5 px-2.5 h-9 rounded-[4px] text-sm hover:bg-panel-2">
            <ShieldCheck class="w-4 h-4 text-ink-2" /> Admin
          </a>
        {/if}
        <a href={DOCS_URL} target="_blank" rel="noopener noreferrer" class="flex items-center gap-2.5 px-2.5 h-9 rounded-[4px] text-sm hover:bg-panel-2">
          <BookOpen class="w-4 h-4 text-ink-2" /> Documentation
        </a>
      </div>
      <div class="px-3 py-3 border-t border-rule">
        <ThemeSwitch />
      </div>
      <div class="p-1.5 border-t border-rule">
        <button onclick={handleLogout} class="w-full flex items-center gap-2.5 px-2.5 h-9 rounded-[4px] text-sm text-danger hover:bg-danger-soft cursor-pointer">
          <LogOut class="w-4 h-4" /> Log out
        </button>
      </div>
    </div>
  {/if}

  <Toasts />

  <main class="flex-1 w-full max-w-6xl mx-auto px-4 pt-6 sm:pt-8
    {auth.loggedIn ? (hasPlayer ? 'pb-44 md:pb-28' : 'pb-24 md:pb-10') : 'pb-10'}">
    {@render children()}

    {#if buildInfo}
      <footer class="mt-16 pt-4 border-t border-rule text-xs text-ink-3 data flex flex-wrap gap-x-3 gap-y-1">
        <span>rpodder {buildInfo.version}</span>
        {#if buildInfo.build_tag !== 'dev'}<span>{buildInfo.build_tag}</span>{/if}
        {#if buildInfo.build_sha !== 'local' && buildInfo.build_sha !== 'unknown'}<span>{buildInfo.build_sha.substring(0, 7)}</span>{/if}
        <span>{buildInfo.database}</span>
      </footer>
    {/if}
  </main>

  <MiniPlayer />

  <!-- Phone tab bar -->
  {#if auth.loggedIn}
    <nav aria-label="Main" class="md:hidden fixed inset-x-0 bottom-0 z-40 bg-panel border-t border-rule pb-[env(safe-area-inset-bottom)]">
      <div class="grid grid-cols-4 h-14">
        {#each nav as item (item.href)}
          {@const active = item.match(path)}
          {@const Icon = item.icon}
          <a href={item.href} aria-current={active ? 'page' : undefined}
            class="relative flex flex-col items-center justify-center gap-0.5 text-[0.6875rem] font-semibold
              {active ? 'text-ink' : 'text-ink-3'}">
            {#if active}<span class="absolute top-0 inset-x-6 h-[3px] bg-red rounded-b-[2px]"></span>{/if}
            <Icon class="w-5 h-5" />
            {item.label}
          </a>
        {/each}
      </div>
    </nav>
  {/if}
</div>
