<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getHistory, type HistoryItem } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let items = $state<HistoryItem[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let currentPage = $state(0);
  let hasMore = $state(true);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username) { goto('/login'); return; }
    loadPage(0);
  });

  function loadPage(page: number) {
    loading = true;
    currentPage = page;
    getHistory(auth.username!, page).then(data => {
      items = data;
      hasMore = data.length >= 50;
      loading = false;
    }).catch(() => { loading = false; });
  }

  function formatAction(action: string): string {
    const icons: Record<string, string> = { play: '▶️', download: '⬇️', delete: '🗑️', new: '🆕' };
    return icons[action] || action;
  }

  function formatTime(secs: number | undefined): string {
    if (!secs) return '';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  function formatDate(iso: string): string {
    try {
      return new Date(iso).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
    } catch { return iso; }
  }
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Listening History</h1>

  {#if loading && items.length === 0}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if items.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📻</span>
      <h2 class="text-lg font-medium mt-4">No episode actions yet</h2>
      <p class="text-text-dim mt-2">Start listening to podcasts and your history will appear here.</p>
    </div>
  {:else}
    <div class="bg-surface border border-border rounded-xl divide-y divide-border">
      {#each items as item}
        <a href="/discover/podcast?url={encodeURIComponent(item.podcast_url)}" class="px-5 py-3 hover:bg-surface-hover transition-colors flex items-center gap-4 no-underline text-text">
          <span class="text-lg w-8 text-center shrink-0">{formatAction(item.action)}</span>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium truncate">{item.episode_title}</p>
            <p class="text-xs text-text-dim truncate">{item.podcast_title}</p>
          </div>
          <div class="text-right shrink-0">
            {#if item.position && item.total}
              <p class="text-xs text-brand">{formatTime(item.position)} / {formatTime(item.total)}</p>
            {/if}
            <p class="text-xs text-text-dim">{formatDate(item.timestamp)}</p>
          </div>
        </a>
      {/each}
    </div>

    <div class="flex justify-center gap-3">
      {#if currentPage > 0}
        <button onclick={() => loadPage(currentPage - 1)} class="px-4 py-2 bg-surface border border-border text-sm rounded-lg hover:bg-surface-hover cursor-pointer">← Previous</button>
      {/if}
      <span class="px-4 py-2 text-sm text-text-dim">Page {currentPage + 1}</span>
      {#if hasMore}
        <button onclick={() => loadPage(currentPage + 1)} class="px-4 py-2 bg-surface border border-border text-sm rounded-lg hover:bg-surface-hover cursor-pointer">Next →</button>
      {/if}
    </div>
  {/if}
</div>
