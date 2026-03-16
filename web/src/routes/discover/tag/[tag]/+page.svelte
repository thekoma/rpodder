<script lang="ts">
  import { getPodcastsForTag, uploadSubscriptionChanges, getDevices, type PodcastInfo } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';

  let podcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let tag = $state('');
  let subscribing = $state<string | null>(null);
  let subscribeMsg = $state('');

  $effect(() => {
    if (!browser) return;
    const newTag = window.location.pathname.split('/').pop() || '';
    if (newTag === tag && loaded) return;
    tag = newTag;
    loaded = true;
    loading = true;
    getPodcastsForTag(decodeURIComponent(tag), 50).then(p => {
      podcasts = p;
      loading = false;
    }).catch(() => { loading = false; });
  });

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

<div class="space-y-6">
  <div class="flex items-center gap-3">
    <a href="/discover" class="text-text-dim hover:text-text transition-colors">← Directory</a>
    <h1 class="text-2xl font-bold capitalize">{decodeURIComponent(tag)}</h1>
  </div>

  {#if subscribeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{subscribeMsg}</div>
  {/if}

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if podcasts.length === 0}
    <p class="text-text-dim text-center py-8">No podcasts found for this tag.</p>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
      {#each podcasts as podcast}
        <div class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors flex gap-3">
          {#if podcast.logo_url}
            <img src={podcast.logo_url} alt="" class="w-14 h-14 rounded-lg object-cover shrink-0" loading="lazy" />
          {:else}
            <div class="w-14 h-14 rounded-lg bg-brand-dim flex items-center justify-center text-xl shrink-0">🎙️</div>
          {/if}
          <div class="flex-1 min-w-0">
            <h3 class="font-semibold text-sm line-clamp-1">{podcast.title}</h3>
            {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
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
  {/if}
</div>
