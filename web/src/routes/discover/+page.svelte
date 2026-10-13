<script lang="ts">
  import { Search, LoaderCircle, Rss, BookOpen, Plus, Check } from '@lucide/svelte';
  import { searchAll, getToplist, getTopTags, getPodcastsForTag, getSsoInfo, type PodcastInfo, type TagInfo, type ExternalPodcast } from '#lib/api.js';
  import { auth, isAuthError, sessionExpired } from '#lib/auth.svelte.js';
  import { subscribe } from '#lib/webDevice.js';
  import { browser } from '$app/env';
  import { goto } from '$app/navigation';
  import Cover from '#lib/components/Cover.svelte';
  import AddFeed from '#lib/components/AddFeed.svelte';
  import { looksLikeFeedUrl, plainText, uniqueByUrl } from '#lib/format.js';
  import { notify, notifyError } from '#lib/toast.svelte.js';

  let query = $state('');
  let localResults = $state<PodcastInfo[]>([]);
  let externalResults = $state<ExternalPodcast[]>([]);
  let topPodcasts = $state<PodcastInfo[]>([]);
  let tags = $state<TagInfo[]>([]);
  let categoryPodcasts = $state<Map<string, PodcastInfo[]>>(new Map());
  let searching = $state(false);
  let loading = $state(true);
  let subscribing = $state<string | null>(null);
  let added = $state<Set<string>>(new Set());
  let podcastindexConfigured = $state(true);
  let searchTimeout: ReturnType<typeof setTimeout>;
  let feedUrl = $state('');

  const DOCS_URL = 'https://thekoma.github.io/rpodder/admin-guide/external-search/';

  let isUrl = $derived(looksLikeFeedUrl(query));

  $effect(() => {
    if (!browser) return;
    loading = true;
    getSsoInfo().then(info => { podcastindexConfigured = info.podcastindex; });
    Promise.all([getToplist(6), getTopTags(30)]).then(([top, t]) => {
      topPodcasts = top;
      tags = t;
      loading = false;
      for (const cat of t.slice(0, 3)) {
        getPodcastsForTag(cat.tag, 6).then(podcasts => {
          categoryPodcasts = new Map([...categoryPodcasts, [cat.tag, podcasts]]);
        });
      }
    }).catch(() => { loading = false; });
  });

  function handleSearch() {
    clearTimeout(searchTimeout);
    if (isUrl) { feedUrl = query.trim(); localResults = []; externalResults = []; searching = false; return; }
    if (!query.trim()) { localResults = []; externalResults = []; searching = false; return; }
    searching = true;
    const q = query;
    searchTimeout = setTimeout(() => {
      searchAll(q).then(r => {
        if (q !== query) return;
        localResults = r.local;
        externalResults = uniqueByUrl(r.external);
        searching = false;
      }).catch(() => { searching = false; });
    }, 300);
  }

  async function subscribeUrl(url: string, title: string) {
    const username = auth.username;
    if (!username) { goto('/login'); return; }
    subscribing = url;
    try {
      await subscribe(username, url);
      added = new Set([...added, url]);
      notify(`Subscribed to ${title}`);
    } catch (e) {
      if (isAuthError(e)) { subscribing = null; sessionExpired(username); return; }
      notifyError(`Couldn't subscribe to ${title}, try again.`);
    }
    subscribing = null;
  }
</script>

<svelte:head><title>Discover · rpodder</title></svelte:head>

<div class="space-y-10">
  <!-- Search -->
  <div class="space-y-2">
    <div class="relative">
      <Search class="w-5 h-5 text-ink-3 absolute left-3.5 top-1/2 -translate-y-1/2 pointer-events-none" />
      <input type="search" bind:value={query} oninput={handleSearch}
        placeholder="Search podcasts, or paste a feed address" aria-label="Search podcasts or paste a feed address"
        class="field h-12 pl-11 text-base" />
      {#if searching}
        <LoaderCircle class="w-5 h-5 text-ink-3 absolute right-3.5 top-1/2 -translate-y-1/2 animate-spin" aria-label="Searching" />
      {/if}
    </div>
  </div>

  {#if isUrl}
    <div class="catalog-card p-4 sm:p-5 space-y-3">
      <div class="flex items-center gap-2 font-semibold"><Rss class="w-4 h-4 text-primary" /> Add this feed to your library</div>
      {#if auth.loggedIn}
        <AddFeed bind:value={feedUrl} />
      {:else}
        <p class="text-ink-2 text-sm"><a href="/login" class="underline">Sign in</a> to subscribe by address.</p>
      {/if}
    </div>
  {:else if query.trim()}
    <div class="space-y-8">
      {#if localResults.length > 0}
        <section aria-labelledby="local-heading" class="space-y-3">
          <h2 id="local-heading" class="text-base font-bold">On this server</h2>
          <ul class="panel divide-y divide-rule">
            {#each localResults as podcast (podcast.url)}
              <li>
                <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group flex gap-4 px-4 py-3 hover:bg-panel-2 transition-colors">
                  <Cover src={podcast.logo_url} title={podcast.title} size="md" />
                  <span class="min-w-0">
                    <span class="block font-semibold leading-snug line-clamp-1 group-hover:underline">{podcast.title}</span>
                    {#if podcast.author}<span class="block text-sm text-ink-2 truncate">{podcast.author}</span>{/if}
                    {#if podcast.description}<span class="block text-sm text-ink-3 mt-0.5 line-clamp-1">{plainText(podcast.description)}</span>{/if}
                  </span>
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if externalResults.length > 0}
        <section aria-labelledby="external-heading" class="space-y-3">
          <h2 id="external-heading" class="text-base font-bold">From Podcast Index</h2>
          <ul class="panel divide-y divide-rule">
            {#each externalResults as podcast (podcast.url)}
              <li class="flex items-center gap-4 pl-4 pr-3 py-3 hover:bg-panel-2 transition-colors">
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
                    <button onclick={() => subscribeUrl(podcast.url, podcast.title)} disabled={subscribing === podcast.url}
                      class="btn btn-secondary btn-sm shrink-0">
                      <Plus class="w-4 h-4" /> {subscribing === podcast.url ? 'Adding…' : 'Subscribe'}
                    </button>
                  {/if}
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if !searching && localResults.length === 0 && externalResults.length === 0}
        <div class="panel p-6 space-y-2 text-center">
          <p class="font-semibold">No podcasts match “{query}”</p>
          <p class="text-sm text-ink-2">Try fewer words, or paste the feed address if you have it.</p>
          {#if !podcastindexConfigured}
            <a href={DOCS_URL} target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1.5 text-sm font-semibold text-primary hover:underline">
              <BookOpen class="w-4 h-4" /> Add Podcast Index to search beyond this server
            </a>
          {/if}
        </div>
      {/if}
    </div>
  {:else if loading}
    <div class="grid grid-cols-3 sm:grid-cols-6 gap-4" aria-busy="true">
      {#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton aspect-square"></div>{/each}
    </div>
  {:else}
    {#if topPodcasts.length > 0}
      <section aria-labelledby="popular-heading" class="space-y-4">
        <div class="flex items-baseline justify-between gap-3">
          <h2 id="popular-heading" class="text-base font-bold">Popular on this server</h2>
          <a href="/discover/toplist" class="text-sm font-semibold text-primary hover:underline">All top podcasts</a>
        </div>
        <ul class="grid grid-cols-3 sm:grid-cols-6 gap-x-4 gap-y-5">
          {#each topPodcasts as podcast (podcast.url)}
            <li>
              <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group block space-y-2">
                <Cover src={podcast.logo_url} title={podcast.title} size="fill" />
                <span class="block text-sm font-semibold leading-snug line-clamp-2 group-hover:underline">{podcast.title}</span>
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if categoryPodcasts.size > 0}
      <div class="grid gap-8 lg:grid-cols-3">
        {#each [...categoryPodcasts.entries()] as [category, podcasts] (category)}
          <section aria-label={category} class="space-y-3">
            <div class="flex items-baseline justify-between gap-3">
              <h2 class="text-base font-bold capitalize">{category}</h2>
              <a href="/discover/tag/{encodeURIComponent(category)}" class="text-sm font-semibold text-primary hover:underline">More</a>
            </div>
            <ul class="panel divide-y divide-rule">
              {#each podcasts as podcast (podcast.url)}
                <li>
                  <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group flex items-center gap-3 px-3 py-2.5 hover:bg-panel-2 transition-colors">
                    <Cover src={podcast.logo_url} title={podcast.title} size="xs" />
                    <span class="min-w-0">
                      <span class="block text-sm font-semibold line-clamp-1 group-hover:underline">{podcast.title}</span>
                      {#if podcast.author}<span class="block text-xs text-ink-2 truncate">{podcast.author}</span>{/if}
                    </span>
                  </a>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    {/if}

    {#if tags.length > 0}
      <section aria-labelledby="tags-heading" class="space-y-3">
        <h2 id="tags-heading" class="text-base font-bold">Categories</h2>
        <ul class="flex flex-wrap gap-2">
          {#each tags as tag (tag.tag)}
            <li>
              <a href="/discover/tag/{encodeURIComponent(tag.tag)}"
                class="inline-flex items-center gap-2 h-8 px-3 rounded-[6px] border border-rule bg-panel text-sm font-semibold capitalize hover:border-primary hover:text-primary transition-colors">
                {tag.tag} <span class="text-xs text-brass data">{tag.usage}</span>
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if topPodcasts.length === 0 && tags.length === 0}
      <div class="catalog-card p-6 space-y-3 max-w-2xl">
        <h2 class="text-lg font-bold">The directory is still empty</h2>
        <p class="text-ink-2">It fills up as people on this server subscribe. Search above, or add a feed by its address:</p>
        {#if auth.loggedIn}<AddFeed />{/if}
      </div>
    {/if}
  {/if}
</div>
