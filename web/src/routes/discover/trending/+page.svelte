<script lang="ts">
  import { BookOpen, Plus, Check } from '@lucide/svelte';
  import { getTrending, getSsoInfo, type ExternalPodcast } from '$lib/api';
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { subscribe as subscribeEverywhere } from '$lib/webDevice';
  import { browser } from '$app/environment';
  import { untrack } from 'svelte';
  import Cover from '$lib/components/Cover.svelte';
  import { plainText, uniqueByUrl } from '$lib/format';
  import { notify, notifyError } from '$lib/toast.svelte';

  const DOCS_URL = 'https://thekoma.github.io/rpodder/admin-guide/external-search/';
  const LANGS = [
    { code: '', label: 'All' },
    { code: 'en', label: 'English' },
    { code: 'it', label: 'Italiano' },
    { code: 'de', label: 'Deutsch' },
    { code: 'es', label: 'Español' },
    { code: 'fr', label: 'Français' },
  ];

  let podcasts = $state<ExternalPodcast[]>([]);
  let loading = $state(true);
  let subscribing = $state<string | null>(null);
  let added = $state<Set<string>>(new Set());
  let lang = $state('');
  let podcastindexConfigured = $state(true);

  $effect(() => {
    if (!browser) return;
    getSsoInfo().then(info => { podcastindexConfigured = info.podcastindex; });
    // Language changes reload through setLang; the effect only runs once.
    untrack(loadTrending);
  });

  function loadTrending() {
    loading = true;
    const wanted = lang;
    getTrending(wanted || undefined, 30).then(p => {
      if (wanted === lang) podcasts = uniqueByUrl(p);
    }).catch(() => {}).finally(() => { if (wanted === lang) loading = false; });
  }

  async function subscribe(podcast: ExternalPodcast) {
    const username = auth.username;
    if (!username) return;
    subscribing = podcast.url;
    try {
      await subscribeEverywhere(username, podcast.url);
      added = new Set([...added, podcast.url]);
      notify(`Subscribed to ${podcast.title}`);
    } catch (e) {
      if (isAuthError(e)) { subscribing = null; sessionExpired(username); return; }
      notifyError(`Couldn't subscribe to ${podcast.title}, try again.`);
    }
    subscribing = null;
  }

  function setLang(code: string) {
    if (code === lang) return;
    lang = code;
    loadTrending();
  }
</script>

<svelte:head><title>Trending · rpodder</title></svelte:head>

<div class="space-y-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <p class="text-sm text-ink-2">What listeners everywhere are picking up, from Podcast Index.</p>
    <div role="radiogroup" aria-label="Language" class="inline-flex flex-wrap p-0.5 rounded-[6px] bg-panel-2 border border-rule">
      {#each LANGS as l (l.code)}
        <button role="radio" aria-checked={lang === l.code} onclick={() => setLang(l.code)}
          class="h-8 px-3 rounded-[4px] text-sm font-semibold cursor-pointer transition-colors
            {lang === l.code ? 'bg-panel text-ink shadow-[var(--shadow)]' : 'text-ink-2 hover:text-ink'}">{l.label}</button>
      {/each}
    </div>
  </div>

  {#if loading}
    <div class="panel divide-y divide-rule" aria-busy="true">
      {#each [0, 1, 2, 3] as i (i)}
        <div class="flex items-center gap-4 px-4 py-3"><div class="skeleton w-14 h-14"></div><div class="flex-1 space-y-2"><div class="skeleton h-4 w-1/2"></div><div class="skeleton h-3 w-1/3"></div></div></div>
      {/each}
    </div>
  {:else if podcasts.length === 0}
    {#if !podcastindexConfigured}
      <div class="catalog-card p-6 space-y-3 max-w-2xl">
        <h2 class="text-lg font-bold">Trending needs Podcast Index</h2>
        <p class="text-ink-2">An administrator can add a free Podcast Index key to show what's popular beyond this server.</p>
        <a href={DOCS_URL} target="_blank" rel="noopener noreferrer" class="btn btn-secondary"><BookOpen class="w-4 h-4" /> How to set it up</a>
      </div>
    {:else}
      <p class="panel p-6 text-ink-2">No trending podcasts right now. Try another language.</p>
    {/if}
  {:else}
    <ol class="panel divide-y divide-rule">
      {#each podcasts as podcast, i (podcast.url)}
        <li class="flex items-center gap-4 pl-4 pr-3 py-3 hover:bg-panel-2 transition-colors">
          <span class="hidden sm:block data text-sm text-ink-3 w-6 text-right">{i + 1}</span>
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group flex gap-4 flex-1 min-w-0">
            <Cover src={podcast.logo_url} title={podcast.title} size="md" />
            <span class="min-w-0">
              <span class="block font-semibold leading-snug line-clamp-1 group-hover:underline">{podcast.title}</span>
              {#if podcast.author}<span class="block text-sm text-ink-2 truncate">{podcast.author}</span>{/if}
              {#if podcast.description}<span class="block text-sm text-ink-3 mt-0.5 line-clamp-1">{plainText(podcast.description)}</span>{/if}
            </span>
          </a>
          {#if auth.loggedIn}
            {#if added.has(podcast.url)}
              <span class="btn btn-sm text-success shrink-0"><Check class="w-4 h-4" /> Added</span>
            {:else}
              <button onclick={() => subscribe(podcast)} disabled={subscribing === podcast.url} class="btn btn-secondary btn-sm shrink-0">
                <Plus class="w-4 h-4" /> {subscribing === podcast.url ? 'Adding…' : 'Subscribe'}
              </button>
            {/if}
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>
