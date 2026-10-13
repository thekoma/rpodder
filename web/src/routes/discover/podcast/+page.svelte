<script lang="ts">
  import {
    ArrowLeft, Copy, Check, Plus, BookmarkCheck, RefreshCw, Globe, Download, ExternalLink, ChevronLeft, ChevronRight,
  } from '@lucide/svelte';
  import { getPodcastEpisodes, forceUpdateSingleFeed, type PodcastEpisodesResponse } from '$lib/api';
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';
  import PlayButton from '$lib/components/PlayButton.svelte';
  import Cover from '$lib/components/Cover.svelte';
  import { podcastPositions } from '$lib/positions';
  import { isPlayable, isSafeMediaUrl, normalizeUrl, type KnownPosition } from '$lib/playerLogic';
  import { shownProgress } from '$lib/progress.svelte';
  import { formatDate, formatDuration, minutesLeft, plainText, plural } from '$lib/format';
  import { notify, notifyError } from '$lib/toast.svelte';
  import {
    fetchHoldings, getWebSettingsCached, groupIndex, subscribe, unsubscribe, type Holdings as HoldingsData,
  } from '$lib/webDevice';

  let data = $state<PodcastEpisodesResponse | null>(null);
  // The ?url= value `data` was loaded for (podcastUrl changes before the new data arrives).
  let dataUrl = $state('');
  let loading = $state(true);
  let isSubscribed = $state(false);
  let subscribing = $state(false);
  let refreshing = $state(false);
  let refreshingUrl = $state('');
  let podcastUrl = $state('');
  let currentPage = $state(0);
  let positions = $state.raw(new Map<string, KnownPosition>());
  let holdings = $state.raw<HoldingsData | null>(null);
  let copied = $state(false);
  let showAbout = $state(false);
  let lastUrl = '';

  // React to URL changes
  $effect(() => {
    if (!browser) return;
    const params = new URLSearchParams($page.url.search);
    const url = params.get('url') || '';
    if (url === lastUrl) return;
    lastUrl = url;
    podcastUrl = url;
    data = null;
    dataUrl = '';
    positions = new Map();
    holdings = null;
    isSubscribed = false;
    showAbout = false;
    if (!url) { loading = false; return; }
    currentPage = 0;
    loadData(0).then(() => { if (data && dataUrl === url) loadPositions(url); });
    refreshSubscribed().catch(() => {});
  });

  function loadData(pg: number): Promise<void> {
    const url = podcastUrl;
    loading = true;
    currentPage = pg;
    return getPodcastEpisodes(url, pg).then(d => {
      if (url !== podcastUrl) return;
      data = d;
      dataUrl = url;
      loading = false;
    }).catch(() => { if (url === podcastUrl) loading = false; });
  }

  async function loadPositions(url: string) {
    const username = auth.username;
    if (!username) return;
    let sync = false;
    try {
      sync = (await getWebSettingsCached(username)).syncPositions;
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return; }
    }
    try {
      const result = await podcastPositions(username, url, sync);
      if (url === podcastUrl) positions = result.positions;
    } catch {
      // auth errors already handled by podcastPositions
    }
  }

  async function refreshSubscribed() {
    const username = auth.username;
    if (!username) return;
    const url = podcastUrl;
    // One source for both: subscribed means some device holds the feed.
    const h = await fetchHoldings(username);
    if (url !== podcastUrl) return;
    isSubscribed = (h.held.get(normalizeUrl(url))?.size ?? 0) > 0;
    holdings = h;
  }

  async function toggleSubscribe() {
    const username = auth.username;
    if (!username) return;
    subscribing = true;
    const wasSubscribed = isSubscribed;
    // Pinned: the page may switch to another podcast while this runs.
    const url = podcastUrl;
    const title = data?.podcast.title || url;
    try {
      if (wasSubscribed) {
        // Fresh: an app may have synced since the page loaded.
        const h = await fetchHoldings(username);
        if (url !== podcastUrl) return;
        holdings = h;
        const owners = h.devices.filter(d => h.held.get(normalizeUrl(url))?.has(d.id));
        if (owners.length === 0) {
          notifyError('Not subscribed on any device.');
        } else {
          const names = owners.map(d => d.caption || d.id).join(', ');
          if (confirm(`Unsubscribe from ${title} on: ${names}?`)) {
            await unsubscribe(username, owners, url);
            notify(`Removed ${title}`);
          }
        }
      } else {
        await subscribe(username, url);
        notify(`Subscribed to ${title}`);
      }
      await refreshSubscribed();
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return; }
      notifyError(wasSubscribed ? "Couldn't unsubscribe, try again." : "Couldn't subscribe, try again.");
      await refreshSubscribed().catch(() => {});
    } finally {
      subscribing = false;
    }
  }

  async function refreshFeed() {
    const url = podcastUrl;
    refreshing = true;
    refreshingUrl = url;
    try {
      if (!(await forceUpdateSingleFeed(url))) {
        notifyError("Couldn't refresh the feed.");
        return;
      }
      // The update runs in the background on the server; give it a moment.
      await new Promise(r => setTimeout(r, 3000));
      const d = await getPodcastEpisodes(url, currentPage);
      if (url !== podcastUrl) return;
      if (!d) { notifyError("Feed refreshed, but the episode list couldn't be reloaded."); return; }
      data = d;
      dataUrl = url;
      notify('Feed refreshed');
    } catch {
      notifyError("Couldn't refresh the feed.");
    } finally {
      refreshing = false;
    }
  }

  async function copyFeedUrl() {
    try {
      await navigator.clipboard.writeText(podcastUrl);
      copied = true;
      setTimeout(() => { copied = false; }, 2000);
    } catch {
      notifyError("Couldn't copy, select the address instead.");
    }
  }

  let heldBy = $derived(holdings?.held.get(normalizeUrl(podcastUrl)) ?? new Set<string>());

  let rows = $derived.by(() => {
    if (!data) return [];
    return data.episodes.map((episode, i) => {
      const p = episode.url ? shownProgress(episode.url, episode.duration, positions.get(normalizeUrl(episode.url))) : { position: 0, total: episode.duration ?? 0, loaded: false };
      const played = p.position > 0 && p.total > 0 && p.position >= p.total - 30;
      const started = !played && p.position > 0 && p.total > 0;
      return { episode, n: currentPage * data!.per_page + i + 1, ...p, played, started, summary: plainText(episode.description) };
    });
  });
  let about = $derived(plainText(data?.podcast.description));
  // Durations share one scale across the page.
  let longest = $derived(Math.max(1, ...rows.map(r => r.episode.duration ?? 0)));
</script>

<svelte:head><title>{data?.podcast.title ?? 'Podcast'} · rpodder</title></svelte:head>

{#if loading && !data}
  <div class="space-y-6" aria-busy="true" aria-label="Loading podcast">
    <div class="catalog-card p-5 sm:p-6 flex gap-6">
      <div class="skeleton w-36 h-36 sm:w-40 sm:h-40 shrink-0"></div>
      <div class="flex-1 space-y-3 pt-2"><div class="skeleton h-6 w-2/3"></div><div class="skeleton h-4 w-1/3"></div><div class="skeleton h-4 w-full"></div></div>
    </div>
  </div>
{:else if !data}
  <div class="catalog-card p-8 max-w-xl space-y-3">
    <h1 class="text-xl font-bold">Podcast not found</h1>
    <p class="text-ink-2">rpodder couldn't read this feed. Check the address, or try again later if the publisher is down.</p>
    <a href="/discover" class="btn btn-secondary"><ArrowLeft class="w-4 h-4" /> Back to Discover</a>
  </div>
{:else}
  {@const podcast = data.podcast}
  <div class="space-y-8">
    <!-- The catalog card -->
    <article class="catalog-card shadow-[var(--shadow)]">
      <div class="p-5 sm:p-6 flex flex-col sm:flex-row gap-5 sm:gap-6">
        <Cover src={podcast.logo_url} title={podcast.title} size="lg" />
        <div class="flex-1 min-w-0 space-y-4">
          <div>
            <h1 class="text-[1.625rem] sm:text-[1.875rem] font-bold tracking-[-0.02em] leading-tight">{podcast.title}</h1>
            {#if podcast.author}<p class="text-ink-2 mt-1">{podcast.author}</p>{/if}
          </div>

          <div class="flex flex-wrap items-center gap-2">
            {#if auth.loggedIn}
              <button onclick={toggleSubscribe} disabled={subscribing}
                class="btn {isSubscribed ? 'btn-secondary' : 'btn-primary'}">
                {#if isSubscribed}<BookmarkCheck class="w-4 h-4 text-primary" /> Subscribed{:else}<Plus class="w-4 h-4" /> Subscribe{/if}
              </button>
              {#if auth.isAdmin}
                <button onclick={refreshFeed} disabled={refreshing && refreshingUrl === podcastUrl} class="btn btn-quiet">
                  <RefreshCw class="w-4 h-4 {refreshing && refreshingUrl === podcastUrl ? 'animate-spin' : ''}" /> {refreshing && refreshingUrl === podcastUrl ? 'Refreshing…' : 'Refresh feed'}
                </button>
              {/if}
            {:else}
              <a href="/login" class="btn btn-primary">Sign in to subscribe</a>
            {/if}
            {#if podcast.website && isSafeMediaUrl(podcast.website)}
              <a href={podcast.website} target="_blank" rel="noopener noreferrer" class="btn btn-quiet"><Globe class="w-4 h-4" /> Website</a>
            {/if}
          </div>

          <!-- Card fields: call number, holdings, circulation -->
          <dl class="grid gap-x-6 gap-y-2.5 text-sm sm:grid-cols-[7rem_1fr] border-t border-card-rule pt-4">
            <dt class="text-ink-3 font-semibold">Feed</dt>
            <dd class="flex items-center gap-1.5 min-w-0">
              <span class="data text-[0.8125rem] text-ink-2 truncate" title={podcastUrl}>{podcastUrl}</span>
              <button onclick={copyFeedUrl} class="btn btn-quiet btn-sm btn-icon shrink-0" aria-label="Copy feed address" title="Copy feed address">
                {#if copied}<Check class="w-4 h-4 text-success" />{:else}<Copy class="w-4 h-4" />{/if}
              </button>
            </dd>

            {#if holdings && holdings.devices.length > 0}
              <dt class="text-ink-3 font-semibold">On devices</dt>
              <dd class="flex flex-wrap gap-1.5">
                {#each holdings.devices as d (d.id)}
                  {@const on = heldBy.has(d.id)}
                  {@const g = groupIndex(holdings.groups, d.id)}
                  <span class="tag border {on ? 'border-rule-strong bg-panel text-ink' : 'border-dashed border-rule text-ink-3'}">
                    <span class="w-2 h-2 rounded-[2px] border"
                      style="border-color: {g >= 0 ? `var(--group-${(g % 4) + 1})` : 'var(--ink-3)'}; background: {on ? (g >= 0 ? `var(--group-${(g % 4) + 1})` : 'var(--ink-2)') : 'transparent'}"></span>
                    {d.caption || d.id}
                  </span>
                {/each}
              </dd>
            {/if}

            <dt class="text-ink-3 font-semibold">Listeners</dt>
            <dd class="text-ink-2">
              {plural(podcast.subscribers, 'subscriber')} on this server
            </dd>

            {#if podcast.language}
              <dt class="text-ink-3 font-semibold">Language</dt>
              <dd class="text-ink-2 uppercase data text-[0.8125rem]">{podcast.language}</dd>
            {/if}
          </dl>
        </div>
      </div>

      {#if about}
        <div class="px-5 sm:px-6 pb-5 sm:pb-6 -mt-1">
          <p class="text-[0.9375rem] text-ink-2 leading-relaxed max-w-[70ch] {showAbout ? '' : 'line-clamp-3'}">{about}</p>
          {#if about.length > 240}
            <button onclick={() => { showAbout = !showAbout; }} class="mt-1 text-sm font-semibold text-primary hover:underline cursor-pointer">
              {showAbout ? 'Show less' : 'Read more'}
            </button>
          {/if}
        </div>
      {/if}
    </article>

    <!-- Episodes -->
    <section aria-labelledby="episodes-heading" class="space-y-3">
      <div class="flex items-baseline justify-between gap-3">
        <h2 id="episodes-heading" class="text-base font-bold">Episodes</h2>
        {#if data.total > 0}<span class="text-sm text-brass font-semibold data">{data.total}</span>{/if}
      </div>

      {#if data.episodes.length === 0}
        <p class="panel p-6 text-ink-2">No episodes indexed yet. rpodder reads the feed in the background; check back in a few minutes.</p>
      {:else}
        <ol class="panel divide-y divide-card-rule">
          {#each rows as r (r.episode.url ?? r.n)}
            {@const ep = r.episode}
            <li class="flex gap-3 sm:gap-4 px-3 sm:px-4 py-3.5 {r.loaded ? 'bg-red-soft/40' : ''}">
              <PlayButton
                episode={{ url: ep.url ?? '', title: ep.title, podcastUrl: dataUrl, podcastTitle: podcast.title, imageUrl: podcast.logo_url, duration: ep.duration }}
                playable={isPlayable(ep)} />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-3">
                  <h3 class="font-semibold leading-snug {r.played ? 'text-ink-2' : ''}">{ep.title}</h3>
                  {#if ep.url && isSafeMediaUrl(ep.url)}
                    {#if ep.media}
                      <a href={ep.url} download target="_blank" rel="noopener noreferrer"
                        class="btn btn-quiet btn-sm btn-icon shrink-0 -mt-1 -mr-1" aria-label="Download {ep.title}" title="Download episode">
                        <Download class="w-4 h-4" />
                      </a>
                    {:else}
                      <a href={ep.url} target="_blank" rel="noopener noreferrer"
                        class="btn btn-quiet btn-sm btn-icon shrink-0 -mt-1 -mr-1" aria-label="Open {ep.title}" title="Open episode page">
                        <ExternalLink class="w-4 h-4" />
                      </a>
                    {/if}
                  {/if}
                </div>
                {#if r.summary}<p class="text-sm text-ink-2 mt-1 line-clamp-2 max-w-[75ch]">{r.summary}</p>{/if}
                <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs text-ink-3">
                  {#if ep.released}<span class="data">{formatDate(ep.released)}</span>{/if}
                  {#if ep.duration}
                    <span class="flex items-center gap-2 min-w-[9rem]">
                      <span class="block h-1 w-24 sm:w-32" aria-hidden="true">
                        <span class="block h-full rounded-full bg-rule overflow-hidden" style="width: {Math.max(12, (ep.duration / longest) * 100)}%">
                          {#if r.total > 0 && r.position > 0}
                            <span class="block h-full bg-red" style="width: {Math.min(100, (r.position / r.total) * 100)}%"></span>
                          {/if}
                        </span>
                      </span>
                      <span class="data">{formatDuration(ep.duration)}</span>
                    </span>
                  {/if}
                  {#if r.played}
                    <span class="stamp text-success">Played</span>
                  {:else if r.started}
                    <span class="stamp text-red">{minutesLeft(r.position, r.total)} min left</span>
                  {/if}
                </div>
              </div>
            </li>
          {/each}
        </ol>

        {#if data.episodes.length >= data.per_page || currentPage > 0}
          <nav aria-label="Episode pages" class="flex items-center justify-center gap-2 pt-2">
            <button onclick={() => loadData(currentPage - 1)} disabled={currentPage === 0 || loading} class="btn btn-secondary btn-sm">
              <ChevronLeft class="w-4 h-4" /> Newer
            </button>
            <span class="px-3 text-sm text-ink-2 data">Page {currentPage + 1}</span>
            <button onclick={() => loadData(currentPage + 1)} disabled={data.episodes.length < data.per_page || loading} class="btn btn-secondary btn-sm">
              Older <ChevronRight class="w-4 h-4" />
            </button>
          </nav>
        {/if}
      {/if}
    </section>
  </div>
{/if}
