<script lang="ts">
  import { searchPodcasts, getToplist, getTopTags, type PodcastInfo, type TagInfo } from '$lib/api';
  import { onMount } from 'svelte';

  let query = $state('');
  let results = $state<PodcastInfo[]>([]);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let tags = $state<TagInfo[]>([]);
  let searching = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout>;

  onMount(async () => {
    const [top, t] = await Promise.all([
      getToplist(20),
      getTopTags(20),
    ]);
    topPodcasts = top;
    tags = t;
  });

  function handleSearch() {
    clearTimeout(searchTimeout);
    if (!query.trim()) {
      results = [];
      searching = false;
      return;
    }
    searching = true;
    searchTimeout = setTimeout(async () => {
      results = await searchPodcasts(query);
      searching = false;
    }, 300);
  }
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Discover Podcasts</h1>

  <!-- Search -->
  <div class="relative">
    <input
      type="search"
      bind:value={query}
      oninput={handleSearch}
      placeholder="Search podcasts..."
      class="w-full px-4 py-3 bg-surface border border-border rounded-xl text-text placeholder:text-text-dim focus:outline-none focus:border-brand transition-colors text-lg"
    />
    {#if searching}
      <div class="absolute right-4 top-1/2 -translate-y-1/2 text-text-dim text-sm">
        Searching...
      </div>
    {/if}
  </div>

  <!-- Search Results -->
  {#if query.trim() && results.length > 0}
    <div>
      <h2 class="text-sm text-text-dim mb-3">Search results</h2>
      <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
        {#each results as podcast}
          <div class="bg-surface border border-border rounded-lg p-4 hover:bg-surface-hover transition-colors group">
            {#if podcast.logo_url}
              <img
                src={podcast.logo_url}
                alt={podcast.title}
                class="w-full aspect-square object-cover rounded-md mb-3"
                loading="lazy"
              />
            {:else}
              <div class="w-full aspect-square rounded-md mb-3 bg-brand-dim flex items-center justify-center text-4xl">
                🎙️
              </div>
            {/if}
            <h3 class="font-medium text-sm line-clamp-2">{podcast.title}</h3>
            {#if podcast.author}
              <p class="text-xs text-text-dim mt-1 truncate">{podcast.author}</p>
            {/if}
            {#if podcast.subscribers > 0}
              <p class="text-xs text-text-dim mt-1">{podcast.subscribers} subscribers</p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {:else if query.trim() && !searching}
    <p class="text-text-dim text-center py-8">No podcasts found for "{query}"</p>
  {/if}

  <!-- Tags -->
  {#if !query.trim() && tags.length > 0}
    <div>
      <h2 class="text-lg font-semibold mb-3">Categories</h2>
      <div class="flex flex-wrap gap-2">
        {#each tags as tag}
          <span class="px-3 py-1.5 bg-brand-dim text-brand text-sm rounded-full hover:opacity-80 transition-opacity cursor-pointer">
            {tag.tag}
            <span class="text-xs opacity-60 ml-1">{tag.usage}</span>
          </span>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Top Podcasts -->
  {#if !query.trim() && topPodcasts.length > 0}
    <div>
      <h2 class="text-lg font-semibold mb-3">Top Podcasts</h2>
      <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
        {#each topPodcasts as podcast}
          <div class="bg-surface border border-border rounded-lg p-4 hover:bg-surface-hover transition-colors group">
            {#if podcast.logo_url}
              <img
                src={podcast.logo_url}
                alt={podcast.title}
                class="w-full aspect-square object-cover rounded-md mb-3"
                loading="lazy"
              />
            {:else}
              <div class="w-full aspect-square rounded-md mb-3 bg-brand-dim flex items-center justify-center text-4xl">
                🎙️
              </div>
            {/if}
            <h3 class="font-medium text-sm line-clamp-2">{podcast.title}</h3>
            {#if podcast.author}
              <p class="text-xs text-text-dim mt-1 truncate">{podcast.author}</p>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
