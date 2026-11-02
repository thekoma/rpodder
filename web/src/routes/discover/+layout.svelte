<script lang="ts">
  import { untrack } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import { auth } from '#lib/auth.svelte.js';
  import { getSsoInfo } from '#lib/api.js';

  let { children } = $props();

  // Logged out, the directory may be private (RPODDER_PUBLIC_DIRECTORY=false): ask before showing it.
  let publicDirectory = $state<boolean | null>(null);
  let visible = $derived(auth.loggedIn || publicDirectory === true);

  // Checked for a visitor who arrives logged out. A session that ends here already navigates
  // (sessionExpired goes to /login?expired=1); redirecting again would drop that notice.
  $effect(() => {
    if (!browser || untrack(() => auth.loggedIn) || publicDirectory !== null) return;
    getSsoInfo().then((info) => {
      publicDirectory = info.public_directory;
      if (!info.public_directory && page.url.pathname.startsWith('/discover')) {
        goto('/login', { replaceState: true });
      }
    });
  });

  const tabs = [
    { href: '/discover', label: 'Browse', match: (p: string) => p === '/discover' || p.startsWith('/discover/tag') },
    { href: '/discover/toplist', label: 'Top podcasts', match: (p: string) => p === '/discover/toplist' },
    { href: '/discover/trending', label: 'Trending', match: (p: string) => p === '/discover/trending' },
  ];

  let onPodcast = $derived(page.url.pathname.startsWith('/discover/podcast'));
</script>

{#if visible}
<div class="space-y-6">
  {#if !onPodcast}
    <div class="space-y-4">
      <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">Discover</h1>
      <nav aria-label="Discover sections" class="flex items-center gap-1 border-b border-rule overflow-x-auto">
        {#each tabs as tab (tab.href)}
          {@const active = tab.match(page.url.pathname)}
          <a href={tab.href} aria-current={active ? 'page' : undefined}
            class="relative px-3 py-2.5 text-sm font-semibold whitespace-nowrap transition-colors
              {active ? 'text-ink' : 'text-ink-2 hover:text-ink'}">
            {tab.label}
            {#if active}<span class="absolute left-3 right-3 -bottom-px h-[3px] bg-red rounded-t-[2px]"></span>{/if}
          </a>
        {/each}
      </nav>
    </div>
  {/if}

  {@render children()}
</div>
{/if}
