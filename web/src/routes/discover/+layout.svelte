<script lang="ts">
  import { page } from '$app/stores';

  let { children } = $props();

  const tabs = [
    { href: '/discover', label: 'Browse', match: (p: string) => p === '/discover' },
    { href: '/discover/toplist', label: 'Top Podcasts', match: (p: string) => p === '/discover/toplist' },
    { href: '/discover/trending', label: 'Trending', match: (p: string) => p === '/discover/trending' },
  ];
</script>

<div class="space-y-6">
  <!-- Tab navigation -->
  <div class="flex items-center gap-1 border-b border-border">
    {#each tabs as tab}
      <a
        href={tab.href}
        class="px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px no-underline
          {tab.match($page.url.pathname)
            ? 'border-brand text-brand'
            : 'border-transparent text-text-dim hover:text-text hover:border-border'}"
      >
        {tab.label}
      </a>
    {/each}
  </div>

  {@render children()}
</div>
