<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getAllSubscriptions, getPodcastInfo, getDevices, uploadSubscriptionChanges, type PodcastInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  interface SubInfo {
    url: string;
    info: PodcastInfo | null;
  }

  let subs = $state<SubInfo[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let deviceId = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    const username = auth.username;
    if (!username) { goto('/login'); return; }

    Promise.all([getAllSubscriptions(username), getDevices(username)]).then(([urls, devs]) => {
      subs = urls.map(url => ({ url, info: null }));
      deviceId = devs.length > 0 ? devs[0].id : 'web';
      loading = false;

      // Enrich with metadata in background
      for (let i = 0; i < subs.length; i++) {
        getPodcastInfo(subs[i].url).then(info => {
          if (info) subs[i] = { ...subs[i], info };
        });
      }
    }).catch(() => { loading = false; });
  });

  async function unsubscribe(url: string) {
    if (!auth.username) return;
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, [], [url]);
    if (ok) {
      subs = subs.filter(s => s.url !== url);
    }
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">My Subscriptions</h1>
    <a href="/discover" class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity">+ Discover</a>
  </div>

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if subs.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📭</span>
      <h2 class="text-lg font-medium mt-4">No subscriptions yet</h2>
      <p class="text-text-dim mt-2">Search for podcasts and subscribe to start syncing.</p>
      <a href="/discover" class="inline-block mt-4 px-6 py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity">Discover Podcasts</a>
    </div>
  {:else}
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      {#each subs as sub}
        <div class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors group relative">
          <div class="flex gap-3">
            {#if sub.info?.logo_url}
              <img src={sub.info.logo_url} alt="" class="w-16 h-16 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-16 h-16 rounded-lg bg-brand-dim flex items-center justify-center text-2xl shrink-0">🎙️</div>
            {/if}
            <div class="min-w-0 flex-1">
              <h3 class="font-medium text-sm line-clamp-2">{sub.info?.title || sub.url.split('/').pop() || sub.url}</h3>
              {#if sub.info?.author}
                <p class="text-xs text-text-dim mt-1 truncate">{sub.info.author}</p>
              {/if}
            </div>
          </div>
          <button
            onclick={() => unsubscribe(sub.url)}
            class="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-danger hover:text-red-400 bg-bg/80 px-2 py-1 rounded cursor-pointer"
          >
            Unsubscribe
          </button>
        </div>
      {/each}
    </div>
    <p class="text-sm text-text-dim">{subs.length} subscription{subs.length !== 1 ? 's' : ''}</p>
  {/if}
</div>
