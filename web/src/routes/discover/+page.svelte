<script lang="ts">
  import { searchPodcasts, getToplist, getTopTags, uploadSubscriptionChanges, getDevices, type PodcastInfo, type TagInfo } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';

  let query = $state('');
  let results = $state<PodcastInfo[]>([]);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let tags = $state<TagInfo[]>([]);
  let searching = $state(false);
  let loaded = $state(false);
  let subscribing = $state<string | null>(null);
  let subscribeMsg = $state('');
  let searchTimeout: ReturnType<typeof setTimeout>;

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    Promise.all([getToplist(20), getTopTags(20)]).then(([top, t]) => {
      topPodcasts = top;
      tags = t;
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
    subscribeMsg = ok ? `Subscribed to ${podcast.title}` : 'Failed to subscribe';
    setTimeout(() => { subscribeMsg = ''; }, 3000);
  }
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Discover Podcasts</h1>

  {#if subscribeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{subscribeMsg}</div>
  {/if}

  <div class="relative">
    <input
      type="search"
      bind:value={query}
      oninput={handleSearch}
      placeholder="Search podcasts..."
      class="w-full px-4 py-3 bg-surface border border-border rounded-xl text-text placeholder:text-text-dim focus:outline-none focus:border-brand transition-colors text-lg"
    />
    {#if searching}
      <div class="absolute right-4 top-1/2 -translate-y-1/2 text-text-dim text-sm">Searching...</div>
    {/if}
  </div>

  {#if query.trim() && results.length > 0}
    <div>
      <h2 class="text-sm text-text-dim mb-3">Search results</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        {#each results as podcast}
          <div class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors flex gap-4">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt={podcast.title} class="w-20 h-20 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-20 h-20 rounded-lg bg-brand-dim flex items-center justify-center text-3xl shrink-0">🎙️</div>
            {/if}
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold line-clamp-1">{podcast.title}</h3>
              {#if podcast.author}<p class="text-sm text-text-dim truncate">{podcast.author}</p>{/if}
              {#if podcast.description}<p class="text-xs text-text-dim mt-1 line-clamp-2">{podcast.description}</p>{/if}
              {#if auth.loggedIn}
                <button
                  onclick={() => subscribe(podcast)}
                  disabled={subscribing === podcast.title}
                  class="mt-2 px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer"
                >
                  {subscribing === podcast.title ? 'Subscribing...' : '+ Subscribe'}
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {:else if query.trim() && !searching}
    <p class="text-text-dim text-center py-8">No podcasts found for "{query}"</p>
  {/if}

  {#if !query.trim() && tags.length > 0}
    <div>
      <h2 class="text-lg font-semibold mb-3">Categories</h2>
      <div class="flex flex-wrap gap-2">
        {#each tags as tag}
          <button onclick={() => { query = tag.tag; handleSearch(); }} class="px-3 py-1.5 bg-brand-dim text-brand text-sm rounded-full hover:opacity-80 transition-opacity cursor-pointer">
            {tag.tag} <span class="text-xs opacity-60 ml-1">{tag.usage}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if !query.trim() && topPodcasts.length > 0}
    <div>
      <h2 class="text-lg font-semibold mb-3">Top Podcasts</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        {#each topPodcasts as podcast}
          <div class="bg-surface border border-border rounded-xl p-4 flex gap-4">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt={podcast.title} class="w-16 h-16 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-16 h-16 rounded-lg bg-brand-dim flex items-center justify-center text-2xl shrink-0">🎙️</div>
            {/if}
            <div class="min-w-0">
              <h3 class="font-medium text-sm line-clamp-1">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
