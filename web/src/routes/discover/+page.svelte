<script lang="ts">
  import { searchPodcasts, getToplist, getTopTags, getPodcastsForTag, uploadSubscriptionChanges, getDevices, type PodcastInfo, type TagInfo } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';

  let query = $state('');
  let results = $state<PodcastInfo[]>([]);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let tags = $state<TagInfo[]>([]);
  let categoryPodcasts = $state<Map<string, PodcastInfo[]>>(new Map());
  let searching = $state(false);
  let loading = $state(true);
  let subscribing = $state<string | null>(null);
  let subscribeMsg = $state('');
  let searchTimeout: ReturnType<typeof setTimeout>;

  $effect(() => {
    if (!browser) return;
    loading = true;
    Promise.all([getToplist(6), getTopTags(30)]).then(([top, t]) => {
      topPodcasts = top;
      tags = t;
      loading = false;
      // Load podcasts for top 4 categories
      const topCats = t.slice(0, 4);
      for (const cat of topCats) {
        getPodcastsForTag(cat.tag, 6).then(podcasts => {
          categoryPodcasts = new Map([...categoryPodcasts, [cat.tag, podcasts]]);
        });
      }
    }).catch(() => { loading = false; });
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
  {#if subscribeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{subscribeMsg}</div>
  {/if}

  <!-- Search bar (always visible) -->
  <div class="relative">
    <input
      type="search"
      bind:value={query}
      oninput={handleSearch}
      placeholder="Search podcasts..."
      class="w-full px-4 py-3 bg-surface border border-border rounded-xl text-text placeholder:text-text-dim focus:outline-none focus:border-brand transition-colors"
    />
    {#if searching}
      <div class="absolute right-4 top-1/2 -translate-y-1/2 text-text-dim text-sm">Searching...</div>
    {/if}
  </div>

  <!-- Search results -->
  {#if query.trim()}
    {#if results.length > 0}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
        {#each results as podcast}
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors flex gap-3 no-underline text-text">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt="" class="w-14 h-14 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-14 h-14 rounded-lg bg-brand-dim flex items-center justify-center text-xl shrink-0">🎙</div>
            {/if}
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold text-sm line-clamp-1">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
              {#if podcast.description}<p class="text-xs text-text-dim mt-1 line-clamp-2">{podcast.description}</p>{/if}
            </div>
          </a>
        {/each}
      </div>
    {:else if !searching}
      <p class="text-text-dim text-center py-8">No podcasts found for "{query}"</p>
    {/if}
  {:else if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else}
    <!-- Popular -->
    {#if topPodcasts.length > 0}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-lg font-semibold">Popular</h2>
          <a href="/discover/toplist" class="text-xs text-brand hover:underline">View all</a>
        </div>
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
          {#each topPodcasts as podcast}
            <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="bg-surface border border-border rounded-lg p-3 hover:bg-surface-hover transition-colors no-underline text-text block">
              {#if podcast.logo_url}
                <img src={podcast.logo_url} alt={podcast.title} class="w-full aspect-square object-cover rounded-md mb-2" loading="lazy" />
              {:else}
                <div class="w-full aspect-square rounded-md mb-2 bg-brand-dim flex items-center justify-center text-3xl">🎙</div>
              {/if}
              <h3 class="font-medium text-xs line-clamp-2">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate mt-0.5">{podcast.author}</p>{/if}
            </a>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Categories -->
    {#each [...categoryPodcasts.entries()] as [category, podcasts]}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-lg font-semibold capitalize">{category}</h2>
          <a href="/discover/tag/{category}" class="text-xs text-brand hover:underline">more</a>
        </div>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
          {#each podcasts as podcast}
            <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-surface-hover transition-colors no-underline text-text">
              {#if podcast.logo_url}
                <img src={podcast.logo_url} alt="" class="w-10 h-10 rounded object-cover shrink-0" loading="lazy" />
              {:else}
                <div class="w-10 h-10 rounded bg-brand-dim flex items-center justify-center text-sm shrink-0">🎙</div>
              {/if}
              <div class="min-w-0 flex-1">
                <span class="text-sm font-medium line-clamp-1">{podcast.title}</span>
                {#if podcast.author}<span class="text-xs text-text-dim block truncate">{podcast.author}</span>{/if}
              </div>
            </a>
          {/each}
        </div>
      </div>
    {/each}

    <!-- Tags -->
    {#if tags.length > 0}
      <div>
        <h2 class="text-lg font-semibold mb-3">Categories</h2>
        <div class="flex flex-wrap gap-2">
          {#each tags as tag}
            <a
              href="/discover/tag/{tag.tag}"
              class="px-3 py-1.5 bg-surface border border-border rounded-full text-sm text-text-dim hover:text-brand hover:border-brand transition-colors no-underline"
            >
              {tag.tag} <span class="text-xs opacity-60">{tag.usage}</span>
            </a>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>
