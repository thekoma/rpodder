<script lang="ts">
  import { browser } from '$app/environment';
  import { getInProgress, type PlayItem } from '$lib/api';
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { localInProgress } from '$lib/positions';
  import { isPlayable } from '$lib/playerLogic';
  import { getWebSettingsCached } from '$lib/webDevice';
  import PlayButton from './PlayButton.svelte';

  const LIMIT = 12;

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

  function episodeTitle(item: PlayItem): string {
    if (item.episode_title) return item.episode_title;
    try {
      const seg = new URL(item.episode_url).pathname.split('/').filter(Boolean).pop();
      return seg ? decodeURIComponent(seg) : item.episode_url;
    } catch {
      return item.episode_url;
    }
  }

  function podcastTitle(item: PlayItem): string {
    if (item.podcast_title) return item.podcast_title;
    try {
      return new URL(item.podcast_url).host;
    } catch {
      return item.podcast_url;
    }
  }
</script>

{#if items.length > 0}
  <section>
    <h2 class="text-lg font-semibold mb-3">Continue listening</h2>
    <div class="grid gap-3 grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
      {#each items as item}
        {@const title = episodeTitle(item)}
        {@const pTitle = podcastTitle(item)}
        <div class="bg-surface border border-border rounded-xl p-3 flex items-center gap-3">
          {#if item.podcast_logo_url}
            <img src={item.podcast_logo_url} alt="" class="w-12 h-12 rounded-lg object-cover shrink-0" loading="lazy" />
          {:else}
            <div class="w-12 h-12 rounded-lg bg-brand-dim flex items-center justify-center text-xl shrink-0">🎙</div>
          {/if}
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium line-clamp-1" title={title}>{title}</p>
            <a href="/discover/podcast?url={encodeURIComponent(item.podcast_url)}"
              class="block text-xs text-text-dim hover:text-text truncate transition-colors">{pTitle}</a>
          </div>
          <PlayButton
            episode={{ url: item.episode_url, title, podcastUrl: item.podcast_url, podcastTitle: pTitle, imageUrl: item.podcast_logo_url, duration: item.total }}
            playable={isPlayable({ url: item.episode_url, mimetype: item.mimetype })}
            progress={item} />
        </div>
      {/each}
    </div>
  </section>
{/if}
