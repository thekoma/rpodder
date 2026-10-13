<script lang="ts">
  import { browser } from '$app/env';
  import { getInProgress, type PlayItem } from '#lib/api.js';
  import { auth, isAuthError, sessionExpired } from '#lib/auth.svelte.js';
  import { localInProgress } from '#lib/positions.js';
  import { isPlayable } from '#lib/playerLogic.js';
  import { getWebSettingsCached } from '#lib/webDevice.js';
  import { shownProgress } from '#lib/progress.svelte.js';
  import { episodeFallbackTitle, feedFallbackTitle, minutesLeft } from '#lib/format.js';
  import PlayButton from './PlayButton.svelte';
  import Cover from './Cover.svelte';

  const LIMIT = 6;

  let items = $state<PlayItem[]>([]);
  let loaded = false;

  $effect(() => {
    if (!browser || loaded) return;
    const username = auth.username;
    if (!username) return;
    loaded = true;
    load(username);
  });

  async function load(username: string) {
    let sync = false;
    try {
      sync = (await getWebSettingsCached(username)).syncPositions;
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return; }
    }
    if (!sync) {
      items = localInProgress(username, LIMIT);
      return;
    }
    try {
      items = await getInProgress(LIMIT);
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return; }
      items = localInProgress(username, LIMIT);
    }
  }

  let rows = $derived(items.map(item => {
    const p = shownProgress(item.episode_url, item.total, item);
    return { item, ...p, title: item.episode_title || episodeFallbackTitle(item.episode_url), podcast: item.podcast_title || feedFallbackTitle(item.podcast_url) };
  }));
  // Bars share one scale: a two-hour episode is drawn twice as long as a one-hour one.
  let longest = $derived(Math.max(1, ...rows.map(r => r.total || 0)));
</script>

{#if items.length > 0}
  <section aria-labelledby="continue-heading" class="space-y-3">
    <h2 id="continue-heading" class="text-base font-bold">Continue listening</h2>
    <ul class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
      {#each rows as r (r.item.episode_url)}
        <li class="panel p-3 flex items-center gap-3 min-w-0">
          <Cover src={r.item.podcast_logo_url} title={r.podcast} size="sm" />
          <div class="min-w-0 flex-1">
            <p class="text-sm font-semibold leading-snug line-clamp-1" title={r.title}>{r.title}</p>
            <a href="/discover/podcast?url={encodeURIComponent(r.item.podcast_url)}"
              class="block text-xs text-ink-2 hover:text-ink hover:underline truncate">{r.podcast}</a>
            {#if r.total > 0}
              <div class="mt-2 flex items-center gap-2">
                <div class="h-1 rounded-full bg-rule overflow-hidden" style="width: {Math.max(18, (r.total / longest) * 100)}%">
                  <div class="h-full bg-red" style="width: {Math.min(100, (r.position / r.total) * 100)}%"></div>
                </div>
                <span class="text-[0.6875rem] text-ink-3 data whitespace-nowrap">{minutesLeft(r.position, r.total)} min left</span>
              </div>
            {/if}
          </div>
          <PlayButton
            episode={{ url: r.item.episode_url, title: r.title, podcastUrl: r.item.podcast_url, podcastTitle: r.podcast, imageUrl: r.item.podcast_logo_url, duration: r.item.total }}
            playable={isPlayable({ url: r.item.episode_url, mimetype: r.item.mimetype })} />
        </li>
      {/each}
    </ul>
  </section>
{/if}
