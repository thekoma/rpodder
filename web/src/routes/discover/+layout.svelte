<script lang="ts">
  import { page } from '$app/state';

  let { children } = $props();

  const tabs = [
    { href: '/discover', label: 'Browse', match: (p: string) => p === '/discover' || p.startsWith('/discover/tag') },
    { href: '/discover/toplist', label: 'Top podcasts', match: (p: string) => p === '/discover/toplist' },
    { href: '/discover/trending', label: 'Trending', match: (p: string) => p === '/discover/trending' },
  ];

  let onPodcast = $derived(page.url.pathname.startsWith('/discover/podcast'));
</script>

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
