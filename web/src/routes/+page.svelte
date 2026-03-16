<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getHealth, getToplist, type PodcastInfo, type HealthInfo } from '$lib/api';
  import { browser } from '$app/environment';

  let health = $state<HealthInfo | null>(null);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);
  let loaded = $state(false);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    Promise.all([getHealth(), getToplist(12)]).then(([h, top]) => {
      health = h;
      topPodcasts = top;
      loading = false;
    }).catch(() => { loading = false; });
  });
</script>

<div class="space-y-8">
  <div class="text-center py-12">
    <h1 class="text-4xl font-bold text-brand mb-3">🎧 rpodder</h1>
    <p class="text-text-dim text-lg max-w-xl mx-auto">Sync your podcasts across all your devices. Open, self-hosted, compatible with AntennaPod, gPodder, and Kasts.</p>
    {#if !auth.loggedIn}
      <div class="mt-6"><a href="/login" class="px-6 py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity">Sign in</a></div>
    {:else}
      <div class="mt-6 flex gap-3 justify-center">
        <a href="/subscriptions" class="px-6 py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity">My Subscriptions</a>
        <a href="/discover" class="px-6 py-2.5 bg-surface border border-border text-text rounded-lg hover:bg-surface-hover transition-colors">Discover Podcasts</a>
      </div>
    {/if}
  </div>

  {#if health}
    <div class="flex justify-center gap-6 text-sm text-text-dim">
      <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-success"></span> Server online</span>
      <span>v{health.version}</span>
      <span class="capitalize">{health.database}</span>
    </div>
  {/if}

  {#if topPodcasts.length > 0}
    <div>
      <h2 class="text-xl font-semibold mb-4">Popular Podcasts</h2>
      <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
        {#each topPodcasts as podcast}
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="bg-surface border border-border rounded-lg p-4 hover:bg-surface-hover transition-colors group no-underline text-text block">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt={podcast.title} class="w-full aspect-square object-cover rounded-md mb-3" loading="lazy" />
            {:else}
              <div class="w-full aspect-square rounded-md mb-3 bg-brand-dim flex items-center justify-center text-4xl">🎙️</div>
            {/if}
            <h3 class="font-medium text-sm line-clamp-2">{podcast.title}</h3>
            {#if podcast.author}<p class="text-xs text-text-dim mt-1 truncate">{podcast.author}</p>{/if}
          </a>
        {/each}
      </div>
    </div>
  {:else if !loading}
    <p class="text-center text-text-dim">No podcasts indexed yet. Subscribe via a podcast app to get started.</p>
  {/if}
</div>
