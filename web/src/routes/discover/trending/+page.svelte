<script lang="ts">
  import { getTrending, getSsoInfo, uploadSubscriptionChanges, getDevices, type ExternalPodcast } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { browser } from '$app/environment';

  const DOCS_URL = 'https://thekoma.github.io/rpodder/admin-guide/external-search/';

  let podcasts = $state<ExternalPodcast[]>([]);
  let loading = $state(true);
  let subscribing = $state<string | null>(null);
  let subscribeMsg = $state('');
  let lang = $state('');
  let podcastindexConfigured = $state(true);

  $effect(() => {
    if (!browser) return;
    getSsoInfo().then(info => { podcastindexConfigured = info.podcastindex; });
    loadTrending();
  });

  function loadTrending() {
    loading = true;
    getTrending(lang || undefined, 30).then(p => {
      podcasts = p;
      loading = false;
    }).catch(() => { loading = false; });
  }

  async function subscribe(podcast: ExternalPodcast) {
    if (!auth.username) return;
    subscribing = podcast.title;
    const devices = await getDevices(auth.username);
    const deviceId = devices.length > 0 ? devices[0].id : 'web';
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, [podcast.url], []);
    subscribing = null;
    subscribeMsg = ok ? `Subscribed to ${podcast.title}` : 'Failed';
    setTimeout(() => { subscribeMsg = ''; }, 3000);
  }

  function setLang(newLang: string) {
    lang = newLang;
    loadTrending();
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold">Trending Podcasts</h1>
      <p class="text-xs text-text-dim mt-1">Powered by Podcast Index</p>
    </div>
    <div class="flex gap-1">
      <button onclick={() => setLang('')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === '' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        All
      </button>
      <button onclick={() => setLang('en')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === 'en' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        English
      </button>
      <button onclick={() => setLang('it')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === 'it' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        Italiano
      </button>
      <button onclick={() => setLang('de')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === 'de' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        Deutsch
      </button>
      <button onclick={() => setLang('es')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === 'es' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        Español
      </button>
      <button onclick={() => setLang('fr')}
        class="px-3 py-1 text-xs rounded-full cursor-pointer transition-colors {lang === 'fr' ? 'bg-brand text-bg' : 'bg-surface border border-border text-text-dim hover:text-text'}">
        Français
      </button>
    </div>
  </div>

  {#if subscribeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{subscribeMsg}</div>
  {/if}

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if podcasts.length === 0}
    {#if !podcastindexConfigured}
      <div class="text-center py-12 space-y-3">
        <p class="text-text-dim">Trending podcasts require an external search provider.</p>
        <a href={DOCS_URL} target="_blank" rel="noopener noreferrer"
          class="inline-flex items-center gap-1.5 text-sm text-brand hover:underline">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /></svg>
          Learn how to configure Podcast Index
        </a>
      </div>
    {:else}
      <p class="text-text-dim text-center py-8">No trending podcasts available.</p>
    {/if}
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
      {#each podcasts as podcast}
        <div class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors flex gap-3">
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="flex gap-3 flex-1 min-w-0 no-underline text-text">
            {#if podcast.logo_url}
              <img src={podcast.logo_url} alt="" class="w-14 h-14 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-14 h-14 rounded-lg bg-brand-dim flex items-center justify-center text-xl shrink-0">🎙</div>
            {/if}
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold text-sm line-clamp-1">{podcast.title}</h3>
              {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
              {#if podcast.description}<p class="text-xs text-text-dim mt-1 line-clamp-2">{podcast.description}</p>{/if}
              {#if podcast.language}
                <span class="text-xs text-text-dim/50 uppercase">{podcast.language}</span>
              {/if}
            </div>
          </a>
          {#if auth.loggedIn}
            <button
              onclick={() => subscribe(podcast)}
              disabled={subscribing === podcast.title}
              class="self-center px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md hover:opacity-90 disabled:opacity-50 cursor-pointer shrink-0"
            >
              {subscribing === podcast.title ? '...' : '+ Add'}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
