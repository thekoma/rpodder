<script lang="ts">
  import { searchPodcasts, getToplist, getTopTags, getPodcastsForTag, uploadSubscriptionChanges, getDevices, type PodcastInfo, type TagInfo } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';

  let query = $state('');
  let results = $state<PodcastInfo[]>([]);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let tags = $state<TagInfo[]>([]);
  let categoryPodcasts = $state<Map<string, PodcastInfo[]>>(new Map());
  let searching = $state(false);
  let loaded = $state(false);
  let subscribing = $state<string | null>(null);
  let subscribeMsg = $state('');
  let showSearch = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout>;

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    // Check if search mode from URL
    const params = new URLSearchParams(window.location.search);
    if (params.get('search')) showSearch = true;

    Promise.all([getToplist(6), getTopTags(30)]).then(([top, t]) => {
      topPodcasts = top;
      tags = t;

      // Load podcasts for top 4 categories
      const topCats = t.slice(0, 4);
      for (const cat of topCats) {
        getPodcastsForTag(cat.tag, 6).then(podcasts => {
          categoryPodcasts = new Map([...categoryPodcasts, [cat.tag, podcasts]]);
        });
      }
    });
  });

  function handleSearch() {
    clearTimeout(searchTimeout);
    if (!query.trim()) { results = []; searching = false; return; }
    searching = true;
    searchTimeout = setTimeout(() => {
      searchPodcasts(query).then(r => { results = r; searching = false; });
    }, 300);
  }

  async function subscribe(podcast: PodcastInfo) {
    if (!auth.username) return;
    subscribing = podcast.title;
    const devices = await getDevices(auth.username);
    const deviceId = devices.length > 0 ? devices[0].id : 'web';
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, [podcast.url], []);
    subscribing = null;
    subscribeMsg = ok ? `Subscribed to ${podcast.title}` : 'Failed';
    setTimeout(() => { subscribeMsg = ''; }, 3000);
  }
</script>

<div class="space-y-8">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Podcast Directory</h1>
    <button
      onclick={() => { showSearch = !showSearch; }}
      class="px-4 py-2 bg-surface border border-border text-text-dim text-sm rounded-lg hover:bg-surface-hover transition-colors cursor-pointer"
    >
      {showSearch ? 'Browse' : '🔍 Search'}
    </button>
  </div>

  {#if subscribeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{subscribeMsg}</div>
  {/if}

  <!-- Search mode -->
  {#if showSearch}
    <div class="relative">
      <input
        type="search"
        bind:value={query}
        oninput={handleSearch}
        placeholder="Search podcasts..."
        autofocus
        class="w-full px-4 py-3 bg-surface border border-border rounded-xl text-text placeholder:text-text-dim focus:outline-none focus:border-brand transition-colors text-lg"
      />
      {#if searching}
        <div class="absolute right-4 top-1/2 -translate-y-1/2 text-text-dim text-sm">Searching...</div>
      {/if}
    </div>

    {#if query.trim() && results.length > 0}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
        {#each results as podcast}
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors flex gap-3 no-underline text-text">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt="" class="w-14 h-14 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-14 h-14 rounded-lg bg-brand-dim flex items-center justify-center text-xl shrink-0">🎙️</div>
            {/if}
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold text-sm line-clamp-1">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
              {#if podcast.description}<p class="text-xs text-text-dim mt-1 line-clamp-2">{podcast.description}</p>{/if}
            </div>
          </a>
        {/each}
      </div>
    {:else if query.trim() && !searching}
      <p class="text-text-dim text-center py-8">No podcasts found for "{query}"</p>
    {/if}
  {:else}
    <!-- Browse mode: categories -->

    <!-- Top podcasts -->
    {#if topPodcasts.length > 0}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-lg font-semibold">Popular</h2>
          <a href="/discover/toplist" class="text-xs text-brand hover:underline">View toplist →</a>
        </div>
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
          {#each topPodcasts as podcast}
            <div class="bg-surface border border-border rounded-lg p-3 hover:bg-surface-hover transition-colors cursor-pointer">
              {#if podcast.logo_url}
                <img src={podcast.logo_url} alt={podcast.title} class="w-full aspect-square object-cover rounded-md mb-2" loading="lazy" />
              {:else}
                <div class="w-full aspect-square rounded-md mb-2 bg-brand-dim flex items-center justify-center text-3xl">🎙️</div>
              {/if}
              <h3 class="font-medium text-xs line-clamp-2">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate mt-0.5">{podcast.author}</p>{/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Category sections -->
    {#each [...categoryPodcasts.entries()] as [category, podcasts]}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-lg font-semibold capitalize">{category}</h2>
          <a href="/discover/tag/{category}" class="text-xs text-brand hover:underline">more →</a>
        </div>
        <div class="space-y-1">
          {#each podcasts as podcast}
            <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-surface-hover transition-colors cursor-pointer">
              {#if podcast.logo_url}
                <img src={podcast.logo_url} alt="" class="w-8 h-8 rounded object-cover shrink-0" loading="lazy" />
              {:else}
                <div class="w-8 h-8 rounded bg-brand-dim flex items-center justify-center text-sm shrink-0">🎙️</div>
              {/if}
              <span class="text-sm truncate flex-1">{podcast.title}</span>
              {#if podcast.author}<span class="text-xs text-text-dim hidden md:inline truncate max-w-48">{podcast.author}</span>{/if}
            </div>
          {/each}
        </div>
      </div>
    {/each}

    <!-- Tag cloud -->
    {#if tags.length > 0}
      <div>
        <h2 class="text-lg font-semibold mb-3">Tags</h2>
        <div class="flex flex-wrap gap-2">
          {#each tags as tag}
            {@const size = Math.min(Math.max(tag.usage * 4, 12), 32)}
            <a
              href="/discover/tag/{tag.tag}"
              class="text-brand hover:text-text transition-colors no-underline"
              style="font-size: {size}px;"
            >
              {tag.tag}
            </a>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>
