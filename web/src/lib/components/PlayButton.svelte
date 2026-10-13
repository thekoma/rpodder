<script lang="ts">
  import { Play, Pause } from '@lucide/svelte';
  import { auth } from '$lib/auth.svelte';
  import { player, type PlayerEpisode } from '$lib/player.svelte';
  import { normalizeUrl } from '$lib/playerLogic';

  let { episode, playable, size = 'md' }: {
    episode: PlayerEpisode;
    playable: boolean;
    size?: 'md' | 'lg';
  } = $props();

  let loaded = $derived(!!player.episode && normalizeUrl(player.episode.url) === normalizeUrl(episode.url));
  let playing = $derived(loaded && player.playing);

  function onclick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (!playable || !auth.username) return;
    player.play(auth.username, episode);
  }

  let label = $derived(playable ? (playing ? 'Pause' : `Play ${episode.title}`) : 'No audio file');
</script>

{#if auth.loggedIn}
  <button {onclick} disabled={!playable} title={playable ? (playing ? 'Pause' : 'Play') : 'No audio file'} aria-label={label}
    class="{size === 'lg' ? 'w-11 h-11' : 'w-9 h-9'} shrink-0 rounded-full flex items-center justify-center transition-colors
      {playable
        ? (loaded
          ? 'bg-primary text-primary-ink hover:bg-primary-hover cursor-pointer'
          : 'border border-primary/60 text-primary hover:bg-primary hover:text-primary-ink cursor-pointer')
        : 'border border-rule text-ink-3/60 cursor-not-allowed'}">
    {#if playing}
      <Pause class="w-4 h-4" fill="currentColor" />
    {:else}
      <Play class="w-4 h-4 ml-0.5" fill="currentColor" />
    {/if}
  </button>
{/if}
