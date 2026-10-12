<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { player } from '$lib/player.svelte';
  import { isSafeMediaUrl } from '$lib/playerLogic';

  const RATES = [1, 1.25, 1.5, 2];

  let dragging = $state(false);
  let dragValue = $state(0);
  let shown = $derived(dragging ? dragValue : player.position);
  let remaining = $derived(Math.max(0, player.duration - shown));

  function fmt(secs: number): string {
    const s = Math.max(0, Math.floor(secs || 0));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const ss = String(s % 60).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
  }

  function onInput(e: Event) {
    dragging = true;
    dragValue = Number((e.currentTarget as HTMLInputElement).value);
  }

  function onChange(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    dragging = false;
    player.seek(v);
  }

  function nextRate() {
    const i = RATES.indexOf(player.rate);
    player.setRate(RATES[(i + 1) % RATES.length]);
  }

  function rateLabel(r: number): string {
    return `${r}×`;
  }
</script>

{#if auth.loggedIn && player.episode}
  {@const ep = player.episode}
  <div class="fixed inset-x-0 bottom-0 z-40 bg-surface border-t border-border">
    <div class="max-w-6xl mx-auto px-4 py-2 flex flex-col md:flex-row md:items-center gap-2 md:gap-4">
      <!-- Episode info -->
      <div class="flex items-center gap-3 min-w-0 md:w-72 md:shrink-0">
        {#if ep.imageUrl}
          <img src={ep.imageUrl} alt="" class="w-10 h-10 rounded object-cover shrink-0" />
        {:else}
          <div class="w-10 h-10 rounded bg-brand-dim flex items-center justify-center text-sm shrink-0">🎙</div>
        {/if}
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium truncate" title={ep.title}>{ep.title}</p>
          <a href="/discover/podcast?url={encodeURIComponent(ep.podcastUrl)}"
            class="block text-xs text-text-dim hover:text-text truncate transition-colors">{ep.podcastTitle}</a>
        </div>
        <button onclick={() => player.close()} aria-label="Close player" title="Close player"
          class="md:hidden p-1 text-text-dim hover:text-text transition-colors cursor-pointer">
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
        </button>
      </div>

      {#if player.error}
        <div class="flex-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm min-w-0">
          <span class="text-danger">
            {player.error === 'secure-version-failed'
              ? "Couldn't load a secure (https) version of this episode; it may only be available over plain HTTP."
              : "Can't play this episode."}
          </span>
          {#if isSafeMediaUrl(ep.url)}
            <a href={ep.url} target="_blank" rel="noopener noreferrer" class="text-brand hover:underline shrink-0">Open episode</a>
          {/if}
        </div>
      {:else}
        <!-- Controls -->
        <div class="flex items-center gap-2 flex-1 min-w-0">
          <button onclick={() => player.skip(-15)} aria-label="Back 15 seconds" title="Back 15 seconds"
            class="p-1 text-text-dim hover:text-text transition-colors cursor-pointer shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M3 12a9 9 0 1 0 3-6.7M3 4v5h5" /><text x="12" y="15.5" text-anchor="middle" font-size="7" fill="currentColor" stroke="none">15</text></svg>
          </button>
          <button onclick={() => player.toggle()} aria-label={player.playing ? 'Pause' : 'Play'} title={player.playing ? 'Pause' : 'Play'}
            class="w-9 h-9 rounded-full bg-brand text-bg flex items-center justify-center hover:opacity-90 transition-opacity cursor-pointer shrink-0">
            {#if player.playing}
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>
            {:else}
              <svg class="w-4 h-4 ml-0.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.14v13.72a1 1 0 0 0 1.52.85l10.6-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z" /></svg>
            {/if}
          </button>
          <button onclick={() => player.skip(30)} aria-label="Forward 30 seconds" title="Forward 30 seconds"
            class="p-1 text-text-dim hover:text-text transition-colors cursor-pointer shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 12a9 9 0 1 1-3-6.7M21 4v5h-5" /><text x="12" y="15.5" text-anchor="middle" font-size="7" fill="currentColor" stroke="none">30</text></svg>
          </button>

          <span class="text-xs text-text-dim tabular-nums shrink-0">{fmt(shown)}</span>
          <input type="range" min="0" max={Math.max(1, Math.floor(player.duration))} step="1"
            value={Math.floor(shown)} oninput={onInput} onchange={onChange}
            aria-label="Seek" disabled={player.duration <= 0}
            class="flex-1 min-w-0 accent-brand cursor-pointer disabled:cursor-default" />
          <span class="text-xs text-text-dim tabular-nums shrink-0">-{fmt(remaining)}</span>

          <button onclick={nextRate} title="Playback speed"
            class="px-2 py-0.5 text-xs text-text-dim border border-border rounded-md hover:text-text hover:bg-surface-hover transition-colors cursor-pointer shrink-0 tabular-nums">
            {rateLabel(player.rate)}
          </button>
        </div>
      {/if}

      <button onclick={() => player.close()} aria-label="Close player" title="Close player"
        class="hidden md:block p-1 text-text-dim hover:text-text transition-colors cursor-pointer shrink-0">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
      </button>
    </div>
  </div>
{/if}
