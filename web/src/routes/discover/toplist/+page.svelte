<script lang="ts">
  import { getToplist, type PodcastInfo } from '#lib/api.js';
  import { browser } from '$app/env';
  import Cover from '#lib/components/Cover.svelte';

  let podcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);

  $effect(() => {
    if (!browser) return;
    getToplist(50).then(p => { podcasts = p; }).catch(() => {}).finally(() => { loading = false; });
  });

  let top = $derived(Math.max(1, podcasts[0]?.subscribers ?? 1));
</script>

<svelte:head><title>Top podcasts · rpodder</title></svelte:head>

{#if loading}
  <div class="panel divide-y divide-rule" aria-busy="true">
    {#each [0, 1, 2, 3, 4] as i (i)}
      <div class="flex items-center gap-4 px-4 py-3"><div class="skeleton w-12 h-12"></div><div class="skeleton h-4 flex-1 max-w-sm"></div></div>
    {/each}
  </div>
{:else if podcasts.length === 0}
  <p class="panel p-6 text-ink-2">Nothing indexed yet. The ranking fills up as people on this server subscribe.</p>
{:else}
  <ol class="panel divide-y divide-rule">
    {#each podcasts as podcast, i (podcast.url)}
      <li>
        <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group grid grid-cols-[2rem_auto_1fr] sm:grid-cols-[2.5rem_auto_1fr_12rem] items-center gap-x-4 px-4 py-2.5 hover:bg-panel-2 transition-colors">
          <span class="data text-sm text-ink-3 text-right">{i + 1}</span>
          <Cover src={podcast.logo_url} title={podcast.title} size="sm" />
          <span class="min-w-0">
            <span class="block font-semibold leading-snug truncate group-hover:underline">{podcast.title}</span>
            {#if podcast.author}<span class="block text-sm text-ink-2 truncate">{podcast.author}</span>{/if}
            <span class="sm:hidden block text-xs text-ink-3 data mt-0.5">{podcast.subscribers} subscribers</span>
          </span>
          <span class="hidden sm:flex items-center justify-end gap-3">
            <span class="h-1.5 flex-1 max-w-[7rem] rounded-full bg-rule overflow-hidden" aria-hidden="true">
              <span class="block h-full bg-primary" style="width: {Math.min(100, (podcast.subscribers / top) * 100)}%"></span>
            </span>
            <span class="data text-sm text-ink-2 w-10 text-right">{podcast.subscribers}</span>
          </span>
        </a>
      </li>
    {/each}
  </ol>
{/if}
