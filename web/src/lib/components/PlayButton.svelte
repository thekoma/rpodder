<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { player, type PlayerEpisode } from '$lib/player.svelte';
  import { normalizeUrl, pickResume, type KnownPosition } from '$lib/playerLogic';

  let { episode, playable, progress = null }: {
    episode: PlayerEpisode;
    playable: boolean;
    progress?: KnownPosition | null;
  } = $props();

  let key = $derived(normalizeUrl(episode.url));
  let loaded = $derived(!!player.episode && normalizeUrl(player.episode.url) === key);
  let known = $derived(loaded ? null : pickResume(progress, player.recent[key]));
  let shownPos = $derived(loaded ? player.position : (known?.position ?? 0));
  let shownTotal = $derived(loaded
    ? (player.duration || episode.duration || 0)
    : (known?.total ?? episode.duration ?? 0));
  let showProgress = $derived(shownTotal > 0 && shownPos > 0);
  let percent = $derived(showProgress ? Math.min(100, (shownPos / shownTotal) * 100) : 0);
  let minutesLeft = $derived(Math.max(0, Math.ceil((shownTotal - shownPos) / 60)));

  function onclick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (!playable || !auth.username) return;
    player.play(auth.username, episode);
  }
</script>

{#if auth.loggedIn}
  <div class="flex items-center gap-2 shrink-0">
    <button {onclick} disabled={!playable}
      title={playable ? (loaded && player.playing ? 'Pause' : 'Play') : 'No audio file'}
      aria-label={playable ? (loaded && player.playing ? 'Pause' : 'Play') : 'No audio file'}
      class="w-8 h-8 rounded-full flex items-center justify-center transition-colors shrink-0 {playable
        ? 'bg-brand-dim text-brand hover:bg-brand hover:text-bg cursor-pointer'
        : 'bg-surface border border-border text-text-dim/50 cursor-not-allowed'}">
      {#if loaded && player.playing}
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>
      {:else}
        <svg class="w-4 h-4 ml-0.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.14v13.72a1 1 0 0 0 1.52.85l10.6-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z" /></svg>
      {/if}
    </button>
    {#if showProgress}
      <div class="w-20 min-w-0">
        <div class="h-0.5 w-full bg-border rounded-full overflow-hidden">
          <div class="h-full bg-brand" style="width: {percent}%"></div>
        </div>
        <span class="block text-[11px] text-text-dim mt-1 whitespace-nowrap">{minutesLeft} min left</span>
      </div>
    {/if}
  </div>
{/if}
