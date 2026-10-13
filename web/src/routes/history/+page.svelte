<script lang="ts">
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import { auth } from '#lib/auth.svelte.js';
  import { getHistory, type HistoryItem } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import { dayLabel, feedFallbackTitle, formatClock, formatTime } from '#lib/format.js';

  const PAGE_SIZE = 50;

  let items = $state<HistoryItem[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let currentPage = $state(0);
  let hasMore = $state(false);
  let failed = $state(false);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username) { goto('/login'); return; }
    loadPage(0);
  });

  function loadPage(page: number) {
    loading = true;
    failed = false;
    currentPage = page;
    getHistory(auth.username!, page).then(data => {
      items = data;
      hasMore = data.length >= PAGE_SIZE;
    }).catch(() => { failed = true; }).finally(() => { loading = false; });
    if (browser) window.scrollTo({ top: 0 });
  }

  // The stamp each action leaves on the slip.
  const stamps: Record<string, { label: string; tone: string }> = {
    play: { label: 'Played', tone: 'text-primary' },
    download: { label: 'Downloaded', tone: 'text-brass' },
    delete: { label: 'Deleted', tone: 'text-ink-3' },
    new: { label: 'Marked new', tone: 'text-red' },
  };

  let days = $derived.by(() => {
    const out: { label: string; items: HistoryItem[] }[] = [];
    for (const item of items) {
      const label = dayLabel(item.timestamp);
      const last = out[out.length - 1];
      if (last && last.label === label) last.items.push(item);
      else out.push({ label, items: [item] });
    }
    return out;
  });
</script>

<svelte:head><title>History · rpodder</title></svelte:head>

<div class="space-y-6">
  <header>
    <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">History</h1>
    <p class="text-sm text-ink-2 mt-1">Every play, download and delete your apps reported, newest first.</p>
  </header>

  {#if loading && items.length === 0}
    <div class="panel divide-y divide-rule" aria-busy="true">
      {#each [0, 1, 2, 3, 4] as i (i)}
        <div class="flex items-center gap-4 px-4 py-3"><div class="skeleton h-4 w-12"></div><div class="skeleton h-5 w-16"></div><div class="flex-1 space-y-2"><div class="skeleton h-4 w-1/2"></div><div class="skeleton h-3 w-1/4"></div></div></div>
      {/each}
    </div>
  {:else if failed}
    <div class="panel p-6 space-y-3">
      <p class="font-semibold">Couldn't load your history.</p>
      <button onclick={() => loadPage(currentPage)} class="btn btn-secondary btn-sm">Try again</button>
    </div>
  {:else if items.length === 0}
    <div class="catalog-card p-6 sm:p-8 max-w-2xl space-y-2">
      <h2 class="text-lg font-bold">Nothing on the slip yet</h2>
      <p class="text-ink-2">When you play or download an episode in a synced app, or here in the browser, it is stamped on this list.</p>
    </div>
  {:else}
    <div class="space-y-6 {loading ? 'opacity-60' : ''}">
      {#each days as day (day.label)}
        <section aria-label={day.label} class="space-y-2">
          <h2 class="text-sm font-bold text-ink-2">{day.label}</h2>
          <ol class="panel divide-y divide-card-rule">
            {#each day.items as item, i (i)}
              {@const stamp = stamps[item.action] ?? { label: item.action, tone: 'text-ink-2' }}
              <li>
                <a href="/discover/podcast?url={encodeURIComponent(item.podcast_url)}"
                  class="group grid grid-cols-[4.5rem_1fr] sm:grid-cols-[5rem_7.5rem_1fr_auto] items-center gap-x-3 gap-y-1 px-4 py-3 hover:bg-panel-2 transition-colors">
                  <span class="data text-xs text-ink-3 whitespace-nowrap">{formatTime(item.timestamp)}</span>
                  <span class="hidden sm:block"><span class="stamp {stamp.tone}">{stamp.label}</span></span>
                  <span class="min-w-0">
                    <span class="block font-semibold leading-snug line-clamp-1 group-hover:underline">{item.episode_title || 'Untitled episode'}</span>
                    <span class="block text-sm text-ink-2 truncate">{item.podcast_title || feedFallbackTitle(item.podcast_url)}</span>
                  </span>
                  <span class="col-start-2 sm:col-start-auto flex items-center gap-2">
                    <span class="sm:hidden stamp {stamp.tone}">{stamp.label}</span>
                    {#if item.position && item.total}
                      <span class="data text-xs text-ink-2">{formatClock(item.position)} / {formatClock(item.total)}</span>
                    {/if}
                  </span>
                </a>
              </li>
            {/each}
          </ol>
        </section>
      {/each}
    </div>

    {#if currentPage > 0 || hasMore}
      <nav aria-label="History pages" class="flex items-center justify-center gap-2">
        <button onclick={() => loadPage(currentPage - 1)} disabled={currentPage === 0 || loading} class="btn btn-secondary btn-sm">
          <ChevronLeft class="w-4 h-4" /> Newer
        </button>
        <span class="px-3 text-sm text-ink-2 data">Page {currentPage + 1}</span>
        <button onclick={() => loadPage(currentPage + 1)} disabled={!hasMore || loading} class="btn btn-secondary btn-sm">
          Older <ChevronRight class="w-4 h-4" />
        </button>
      </nav>
    {/if}
  {/if}
</div>
