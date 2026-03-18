<script lang="ts">
  import { getPodcastEpisodes, uploadSubscriptionChanges, getDevices, getAllSubscriptions, forceUpdateSingleFeed, type PodcastEpisodesResponse } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';

  let data = $state<PodcastEpisodesResponse | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let isSubscribed = $state(false);
  let subscribing = $state(false);
  let refreshing = $state(false);
  let refreshMsg = $state('');
  let podcastUrl = $state('');
  let currentPage = $state(0);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    const params = new URLSearchParams(window.location.search);
    podcastUrl = params.get('url') || '';
    if (!podcastUrl) { loading = false; return; }

    loadData(0);

    // Check if already subscribed
    if (auth.username) {
      getAllSubscriptions(auth.username).then(subs => {
        isSubscribed = subs.includes(podcastUrl);
      });
    }
  });

  function loadData(page: number) {
    loading = true;
    currentPage = page;
    getPodcastEpisodes(podcastUrl, page).then(d => {
      data = d;
      loading = false;
    }).catch(() => { loading = false; });
  }

  async function toggleSubscribe() {
    if (!auth.username) return;
    subscribing = true;
    const devices = await getDevices(auth.username);
    const deviceId = devices.length > 0 ? devices[0].id : 'web';
    if (isSubscribed) {
      await uploadSubscriptionChanges(auth.username, deviceId, [], [podcastUrl]);
      isSubscribed = false;
    } else {
      await uploadSubscriptionChanges(auth.username, deviceId, [podcastUrl], []);
      isSubscribed = true;
    }
    subscribing = false;
  }

  async function refreshFeed() {
    refreshing = true;
    await forceUpdateSingleFeed(podcastUrl);
    // Wait a bit for the background task to complete, then reload
    await new Promise(r => setTimeout(r, 3000));
    loaded = false; // Reset to trigger reload
    refreshing = false;
    // Re-fetch data
    getPodcastEpisodes(podcastUrl, currentPage).then(d => {
      if (d) data = d;
    });
  }

  function formatDuration(secs: number | undefined): string {
    if (!secs) return '';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    return `${m} min`;
  }

  function formatDate(iso: string | undefined): string {
    if (!iso) return '';
    try {
      return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
    } catch { return ''; }
  }
</script>

{#if loading && !data}
  <div class="text-center text-text-dim py-12">Loading...</div>
{:else if !data}
  <div class="text-center text-text-dim py-12">Podcast not found.</div>
{:else}
  <div class="space-y-6">
    <!-- Back link -->
    <a href="/discover" class="text-sm text-text-dim hover:text-text transition-colors">← Back to Directory</a>

    <!-- Podcast header -->
    <div class="bg-surface border border-border rounded-xl p-6">
      <div class="flex gap-6 flex-col md:flex-row">
        {#if data.podcast.logo_url}
          <img
            src={data.podcast.logo_url}
            alt={data.podcast.title}
            class="w-40 h-40 rounded-xl object-cover shrink-0 mx-auto md:mx-0"
          />
        {:else}
          <div class="w-40 h-40 rounded-xl bg-brand-dim flex items-center justify-center text-5xl shrink-0 mx-auto md:mx-0">🎙️</div>
        {/if}

        <div class="flex-1 min-w-0">
          <h1 class="text-2xl font-bold">{data.podcast.title}</h1>
          {#if data.podcast.author}
            <p class="text-text-dim mt-1">{data.podcast.author}</p>
          {/if}
          {#if data.podcast.description}
            <p class="text-sm text-text-dim mt-3 leading-relaxed">{data.podcast.description}</p>
          {/if}

          <div class="flex items-center gap-3 mt-4 flex-wrap">
            {#if auth.loggedIn}
              <button
                onclick={toggleSubscribe}
                disabled={subscribing}
                class="px-5 py-2 rounded-lg font-medium text-sm transition-all disabled:opacity-50 cursor-pointer {isSubscribed
                  ? 'bg-surface border border-border text-text-dim hover:text-danger hover:border-danger'
                  : 'bg-brand text-bg hover:opacity-90'}"
              >
                {#if subscribing}
                  ...
                {:else if isSubscribed}
                  ✓ Subscribed
                {:else}
                  + Subscribe
                {/if}
              </button>
            {/if}

            {#if auth.loggedIn}
              <button
                onclick={refreshFeed}
                disabled={refreshing}
                class="px-4 py-1.5 bg-surface border border-border text-text-dim text-xs rounded-lg hover:bg-surface-hover disabled:opacity-50 cursor-pointer"
              >
                {refreshing ? '↻ Refreshing...' : '↻ Refresh Feed'}
              </button>
            {/if}

            {#if data.podcast.language}
              <span class="px-2 py-1 bg-brand-dim text-brand text-xs rounded-full uppercase">{data.podcast.language}</span>
            {/if}
            {#if data.podcast.subscribers > 0}
              <span class="text-xs text-text-dim">{data.podcast.subscribers} subscribers</span>
            {/if}
            {#if data.podcast.website}
              <a href={data.podcast.website} target="_blank" rel="noopener" class="text-xs text-brand hover:underline">Website ↗</a>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <!-- Episodes -->
    <div>
      <h2 class="text-lg font-semibold mb-3">Episodes</h2>

      {#if data.episodes.length === 0}
        <p class="text-text-dim text-center py-8">No episodes indexed yet. Check back after the next feed update.</p>
      {:else}
        <div class="bg-surface border border-border rounded-xl divide-y divide-border">
          {#each data.episodes as episode, i}
            <div class="px-5 py-4 hover:bg-surface-hover transition-colors">
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-text-dim shrink-0">{currentPage * data.per_page + i + 1}</span>
                    <h3 class="font-medium text-sm line-clamp-1">{episode.title}</h3>
                  </div>
                  {#if episode.description}
                    <p class="text-xs text-text-dim mt-1 line-clamp-2">{episode.description}</p>
                  {/if}
                </div>
                <div class="flex items-center gap-3 shrink-0 text-xs text-text-dim">
                  {#if episode.duration}
                    <span>{formatDuration(episode.duration)}</span>
                  {/if}
                  {#if episode.released}
                    <span class="hidden sm:inline">{formatDate(episode.released)}</span>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
        </div>

        <!-- Pagination -->
        {#if data.episodes.length >= data.per_page || currentPage > 0}
          <div class="flex justify-center gap-3 mt-4">
            {#if currentPage > 0}
              <button
                onclick={() => loadData(currentPage - 1)}
                class="px-4 py-2 bg-surface border border-border text-text-dim text-sm rounded-lg hover:bg-surface-hover cursor-pointer"
              >
                ← Previous
              </button>
            {/if}
            <span class="px-4 py-2 text-sm text-text-dim">Page {currentPage + 1}</span>
            {#if data.episodes.length >= data.per_page}
              <button
                onclick={() => loadData(currentPage + 1)}
                class="px-4 py-2 bg-surface border border-border text-text-dim text-sm rounded-lg hover:bg-surface-hover cursor-pointer"
              >
                Next →
              </button>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}
