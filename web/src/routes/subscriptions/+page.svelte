<script lang="ts">
  import { Plus, FileDown, Lock, BookmarkMinus, Search, Compass, X } from '@lucide/svelte';
  import { auth, isAuthError, sessionExpired } from '#lib/auth.svelte.js';
  import { getAllSubscriptions, getHistory, getPodcastInfo, getSubscriptionUpgrades, type PodcastInfo, type UpgradeableSub } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import ContinueListening from '#lib/components/ContinueListening.svelte';
  import Cover from '#lib/components/Cover.svelte';
  import Holdings from '#lib/components/Holdings.svelte';
  import AddFeed from '#lib/components/AddFeed.svelte';
  import { normalizeUrl } from '#lib/playerLogic.js';
  import { feedFallbackTitle, formatDate, groupByLetter, plural } from '#lib/format.js';
  import { notify, notifyError } from '#lib/toast.svelte.js';
  import {
    fetchHoldings, groupIndex, replaceSubscriptions, unsubscribe as unsubscribeFrom, type Holdings as HoldingsData,
  } from '#lib/webDevice.js';

  interface SubInfo {
    url: string;
    info: PodcastInfo | null;
  }

  let subs = $state<SubInfo[]>([]);
  let upgrades = $state<UpgradeableSub[]>([]);
  let holdings = $state.raw<HoldingsData | null>(null);
  // Feed → newest play in the first page of history. When that page is full,
  // older plays exist that it doesn't show, so a missing feed is "unknown".
  let lastPlayed = $state.raw(new Map<string, string>());
  let historyPartial = $state(false);
  let loading = $state(true);
  let loaded = $state(false);
  let busy = $state(false);
  let adding = $state(false);
  let filter = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    const username = auth.username;
    if (!username) { goto('/login'); return; }

    reload(username).catch(() => {}).finally(() => { loading = false; });
  });

  // Re-read subscriptions, upgrades and holdings, keeping fetched metadata.
  async function reload(username: string) {
    const known = new Map(subs.map(s => [normalizeUrl(s.url), s.info]));
    const [urls, upg] = await Promise.all([getAllSubscriptions(username), getSubscriptionUpgrades()]);
    const seen = new Set<string>();
    subs = urls
      .filter(url => { const k = normalizeUrl(url); if (seen.has(k)) return false; seen.add(k); return true; })
      .map(url => ({ url, info: known.get(normalizeUrl(url)) ?? null }));
    upgrades = upg;

    getHistory(username, 0).then(items => {
      const m = new Map<string, string>();
      for (const it of items) {
        if (it.action !== 'play') continue;
        const k = normalizeUrl(it.podcast_url);
        if (!m.has(k)) m.set(k, it.timestamp);
      }
      lastPlayed = m;
      historyPartial = items.length >= 50;
    }).catch(() => {});

    fetchHoldings(username).then(h => { holdings = h; }).catch(e => {
      if (isAuthError(e)) sessionExpired(username);
    });

    // Enrich with metadata in the background.
    for (const sub of subs) {
      if (sub.info) continue;
      const url = sub.url;
      getPodcastInfo(url).then(info => {
        if (!info) return;
        const i = subs.findIndex(s => s.url === url);
        if (i >= 0) subs[i] = { ...subs[i], info };
      });
    }
  }

  function titleOf(s: SubInfo): string {
    return s.info?.title && s.info.title !== s.url ? s.info.title : feedFallbackTitle(s.url);
  }

  let visible = $derived.by(() => {
    const q = filter.trim().toLocaleLowerCase();
    return subs
      .map(s => ({ ...s, title: titleOf(s) }))
      .filter(s => !q || s.title.toLocaleLowerCase().includes(q) || (s.info?.author ?? '').toLocaleLowerCase().includes(q) || s.url.toLocaleLowerCase().includes(q));
  });

  // Guide cards: the catalog drawer is divided by the letter a title is filed under.
  let drawers = $derived(groupByLetter(visible, r => r.title));

  let deviceCount = $derived(holdings?.devices.length ?? 0);

  function heldBy(url: string): Set<string> {
    return holdings?.held.get(normalizeUrl(url)) ?? new Set();
  }

  function fail(e: unknown, username: string, msg: string): boolean {
    if (isAuthError(e)) { sessionExpired(username); return true; }
    notifyError(msg);
    return false;
  }

  async function unsubscribe(sub: SubInfo & { title: string }) {
    const username = auth.username;
    if (!username || busy) return;
    busy = true;
    try {
      // Fresh: an app may have synced since the page loaded.
      const h = await fetchHoldings(username);
      holdings = h;
      const owners = h.devices.filter(d => h.held.get(normalizeUrl(sub.url))?.has(d.id));
      if (owners.length === 0) {
        notifyError('Not subscribed on any device.');
      } else {
        const names = owners.map(d => d.caption || d.id).join(', ');
        if (confirm(`Unsubscribe from ${sub.title} on: ${names}?`)) {
          await unsubscribeFrom(username, owners, sub.url);
          notify(`Removed ${sub.title}`);
        }
      }
      await reload(username);
    } catch (e) {
      if (fail(e, username, "Couldn't unsubscribe, try again.")) return;
      await reload(username).catch(() => {});
    } finally {
      busy = false;
    }
  }

  async function upgrade(list: UpgradeableSub[]) {
    const username = auth.username;
    if (!username || busy || list.length === 0) return;
    busy = true;
    try {
      await replaceSubscriptions(username, list.map(u => ({ from: u.http_url, to: u.https_url })));
      notify(list.length === 1 ? `${list[0].title || 'Feed'} now uses HTTPS` : `${list.length} feeds now use HTTPS`);
      await reload(username);
    } catch (e) {
      if (fail(e, username, "Couldn't switch to HTTPS, try again.")) return;
      await reload(username).catch(() => {});
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>Library · rpodder</title></svelte:head>

<div class="space-y-8">
  <header class="flex flex-wrap items-end justify-between gap-4">
    <div>
      <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">Library</h1>
      {#if !loading}
        <p class="text-sm text-ink-2 mt-1">
          <span>{plural(subs.length, 'podcast')}</span>
          {#if deviceCount > 0}<span class="text-ink-3 mx-1">·</span><span>synced to {plural(deviceCount, 'device')}</span>{/if}
        </p>
      {/if}
    </div>
    <div class="flex items-center gap-2">
      {#if auth.username && subs.length > 0}
        <a href="/subscriptions/{encodeURIComponent(auth.username)}.opml" download="rpodder-{auth.username}.opml" rel="external"
          class="btn btn-secondary" title="Download all subscriptions as OPML">
          <FileDown class="w-4 h-4" /> Export OPML
        </a>
      {/if}
      <button class="btn btn-primary" aria-expanded={adding} onclick={() => { adding = !adding; }}>
        {#if adding}<X class="w-4 h-4" /> Close{:else}<Plus class="w-4 h-4" /> Add feed{/if}
      </button>
    </div>
  </header>

  {#if adding}
    <div class="catalog-card p-4 sm:p-5">
      <AddFeed autofocus />
      <p class="hint mt-3">Looking for something new? <a href="/discover" class="underline hover:text-ink">Search the directory</a>.</p>
    </div>
  {/if}

  {#if upgrades.length > 0}
    <div class="notice">
      <Lock class="w-4 h-4 mt-0.5 text-warning shrink-0" />
      <div class="flex-1 min-w-0 space-y-2">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <p class="font-semibold">{upgrades.length === 1 ? 'A secure address is available for one feed' : `Secure addresses are available for ${upgrades.length} feeds`}</p>
          <button onclick={() => upgrade(upgrades)} disabled={busy} class="btn btn-secondary btn-sm">
            Switch {upgrades.length === 1 ? 'it' : 'all'} to HTTPS
          </button>
        </div>
        {#if upgrades.length > 1}
          <ul class="divide-y divide-rule">
            {#each upgrades as u (u.http_url)}
              <li class="flex items-center justify-between gap-3 py-1.5">
                <span class="text-sm truncate">{u.title || u.http_url}</span>
                <button onclick={() => upgrade([u])} disabled={busy} class="btn btn-quiet btn-sm shrink-0">Switch</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="text-sm text-ink-2 truncate">{upgrades[0].title || upgrades[0].http_url}</p>
        {/if}
      </div>
    </div>
  {/if}

  <ContinueListening />

  <section aria-labelledby="catalog-heading" class="space-y-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h2 id="catalog-heading" class="text-base font-bold">Subscriptions</h2>
      {#if subs.length > 8}
        <div class="relative w-full sm:w-64">
          <Search class="w-4 h-4 text-ink-3 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
          <input type="search" bind:value={filter} placeholder="Filter library" aria-label="Filter library" class="field pl-9 h-9 text-sm" />
        </div>
      {/if}
    </div>

    {#if holdings && holdings.devices.length > 0 && subs.length > 0}
      <ul class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-2" aria-label="Devices">
        {#each holdings.devices as d (d.id)}
          {@const g = groupIndex(holdings.groups, d.id)}
          <li class="flex items-center gap-1.5">
            <span class="w-2.5 h-2.5 rounded-[2px]" style="background: {g >= 0 ? `var(--group-${(g % 4) + 1})` : 'var(--ink-2)'}"></span>
            {d.caption || d.id}{#if g >= 0}<span class="text-ink-3">&nbsp;({holdings.groups[g].name || 'sync group'})</span>{/if}
          </li>
        {/each}
        <li class="flex items-center gap-1.5 text-ink-3">
          <span class="w-2.5 h-2.5 rounded-[2px] border border-ink-3"></span> not on that device
        </li>
      </ul>
    {/if}

    {#if loading}
      <div class="panel divide-y divide-rule" aria-busy="true" aria-label="Loading library">
        {#each [0, 1, 2, 3] as i (i)}
          <div class="flex items-center gap-4 px-4 py-3">
            <div class="skeleton w-14 h-14"></div>
            <div class="flex-1 space-y-2"><div class="skeleton h-3.5 w-1/2"></div><div class="skeleton h-3 w-1/4"></div></div>
          </div>
        {/each}
      </div>
    {:else if subs.length === 0}
      <div class="catalog-card p-6 sm:p-8 space-y-5">
        <div class="space-y-2 max-w-prose">
          <h3 class="text-lg font-bold">Your library is empty</h3>
          <p class="text-ink-2">Podcasts you subscribe to in AntennaPod, gPodder or Kasts appear here once those apps sync with rpodder. You can also add a feed yourself:</p>
        </div>
        <div class="max-w-xl"><AddFeed /></div>
        <a href="/discover" class="btn btn-secondary"><Compass class="w-4 h-4" /> Browse the directory</a>
      </div>
    {:else if visible.length === 0}
      <p class="panel px-4 py-6 text-center text-ink-2">Nothing in your library matches “{filter}”.</p>
    {:else}
      <ul class="panel overflow-hidden">
        {#each drawers as drawer, di (drawer.letter)}
              {#each drawer.rows as sub, ri (sub.url)}
                <li class="group flex items-center gap-3 sm:gap-4 pr-2 py-2.5 hover:bg-panel-2 transition-colors
                  {di > 0 && ri === 0 ? 'border-t border-rule-strong' : (ri > 0 ? 'border-t border-card-rule' : '')}">
                  <!-- Guide card: the letter the drawer is filed under, on its first row -->
                  <span class="w-9 sm:w-11 self-stretch flex justify-center pt-1 shrink-0" aria-hidden="true">
                    {#if ri === 0}<span class="tag h-5 px-1.5 bg-brass-soft text-brass data">{drawer.letter}</span>{/if}
                  </span>
                  <a href="/discover/podcast?url={encodeURIComponent(sub.url)}" class="flex items-center gap-3 sm:gap-4 flex-1 min-w-0">
                    <Cover src={sub.info?.logo_url} title={sub.title} size="md" />
                    <span class="min-w-0">
                      <span class="block font-semibold leading-snug line-clamp-2 group-hover:underline">{sub.title}</span>
                      <span class="block text-sm text-ink-2 truncate">{sub.info?.author || feedFallbackTitle(sub.url)}</span>
                      {#if holdings}
                        <span class="sm:hidden flex items-center gap-2 mt-1">
                          <Holdings devices={holdings.devices} held={heldBy(sub.url)} groupOf={(id) => groupIndex(holdings!.groups, id)} compact />
                          {#if lastPlayed.get(normalizeUrl(sub.url))}<span class="text-xs text-ink-3 data">{formatDate(lastPlayed.get(normalizeUrl(sub.url)))}</span>{/if}
                        </span>
                      {/if}
                    </span>
                  </a>
                  {#if lastPlayed.get(normalizeUrl(sub.url))}
                    <span class="hidden md:block w-24 text-right text-xs text-ink-3 data whitespace-nowrap" title="Last played">
                      {formatDate(lastPlayed.get(normalizeUrl(sub.url)))}
                    </span>
                  {:else}
                    <span class="hidden md:block w-24 text-right text-xs text-ink-3 data" title={historyPartial ? 'No play in your recent history' : 'Never played'}>
                      {historyPartial ? '—' : 'Not played'}
                    </span>
                  {/if}
                  {#if holdings}
                    <span class="hidden sm:inline-flex">
                      <Holdings devices={holdings.devices} held={heldBy(sub.url)} groupOf={(id) => groupIndex(holdings!.groups, id)} />
                    </span>
                  {/if}
                  <button onclick={() => unsubscribe(sub)} disabled={busy}
                    class="btn btn-quiet btn-icon shrink-0 text-ink-3 hover:!text-danger" aria-label="Unsubscribe from {sub.title}" title="Unsubscribe">
                    <BookmarkMinus class="w-[1.125rem] h-[1.125rem]" />
                  </button>
                </li>
              {/each}
        {/each}
      </ul>
    {/if}
  </section>
</div>
