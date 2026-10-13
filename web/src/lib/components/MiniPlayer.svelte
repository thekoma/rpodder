<script lang="ts">
  import {
    Play, Pause, RotateCcw, RotateCw, X, ChevronDown, Download, CircleAlert, Volume1, Volume2, VolumeX,
  } from '@lucide/svelte';
  import { auth } from '#lib/auth.svelte.js';
  import { player } from '#lib/player.svelte.js';
  import { isSafeMediaUrl } from '#lib/playerLogic.js';
  import { formatClock } from '#lib/format.js';
  import Cover from './Cover.svelte';

  const RATES = [1, 1.25, 1.5, 2];

  let dragging = $state(false);
  let dragValue = $state(0);
  let expanded = $state(false);
  let shown = $derived(dragging ? dragValue : player.position);
  let remaining = $derived(Math.max(0, player.duration - shown));
  let percent = $derived(player.duration > 0 ? Math.min(100, (shown / player.duration) * 100) : 0);

  // Collapse the sheet whenever the player goes away.
  $effect(() => {
    if (!player.episode) expanded = false;
  });

  function onInput(e: Event) {
    dragging = true;
    dragValue = Number((e.currentTarget as HTMLInputElement).value);
  }

  function onChange(e: Event) {
    const v = Number((e.currentTarget as HTMLInputElement).value);
    dragging = false;
    player.seek(v);
  }

  let silent = $derived(player.muted || player.volume === 0);
  let volumeLabel = $derived(silent ? 'Unmute' : 'Mute');

  function onVolume(e: Event) {
    player.setVolume(Number((e.currentTarget as HTMLInputElement).value));
  }

  function nextRate() {
    const i = RATES.indexOf(player.rate);
    player.setRate(RATES[(i + 1) % RATES.length]);
  }

  function onKey(e: KeyboardEvent) {
    if (expanded && e.key === 'Escape') expanded = false;
  }

  function errorText(code: string): string {
    return code === 'secure-version-failed'
      ? "Couldn't load a secure (https) copy of this episode; it may only exist over plain HTTP."
      : "This episode can't be played here.";
  }
</script>

<svelte:window onkeydown={onKey} />

{#if auth.loggedIn && player.episode}
  {@const ep = player.episode}

  <!-- Docked bar: above the phone tab bar, at the bottom on desktop -->
  <section aria-label="Player"
    class="fixed inset-x-0 z-40 bottom-[calc(3.5rem+env(safe-area-inset-bottom))] md:bottom-0 bg-panel border-t border-rule shadow-[var(--shadow)]">
    <!-- Progress rule -->
    <div class="h-[3px] bg-rule" aria-hidden="true">
      <div class="h-full bg-red transition-[width] duration-200 ease-out" style="width: {percent}%"></div>
    </div>

    <div class="max-w-6xl mx-auto px-3 md:px-4 h-16 md:h-[4.5rem] flex items-center gap-3 md:gap-4">
      <!-- Episode: tapping opens the full sheet on phones -->
      <button onclick={() => { expanded = true; }} aria-label="Open player"
        class="md:hidden flex items-center gap-3 min-w-0 flex-1 text-left cursor-pointer">
        <Cover src={ep.imageUrl} title={ep.podcastTitle} size="xs" />
        <span class="min-w-0">
          <span class="block text-sm font-semibold truncate">{ep.title}</span>
          <span class="block text-xs text-ink-2 truncate">{ep.podcastTitle}</span>
        </span>
      </button>
      <div class="hidden md:flex items-center gap-3 min-w-0 w-72 shrink-0">
        <Cover src={ep.imageUrl} title={ep.podcastTitle} size="sm" />
        <div class="min-w-0">
          <p class="text-sm font-semibold truncate" title={ep.title}>{ep.title}</p>
          <a href="/discover/podcast?url={encodeURIComponent(ep.podcastUrl)}"
            class="block text-xs text-ink-2 hover:text-ink hover:underline truncate">{ep.podcastTitle}</a>
        </div>
      </div>

      {#if player.error}
        <div class="hidden md:flex flex-1 items-center gap-2 min-w-0 text-sm">
          <CircleAlert class="w-4 h-4 text-danger shrink-0" />
          <span class="text-danger truncate">{errorText(player.error)}</span>
          {#if isSafeMediaUrl(ep.url)}
            <a href={ep.url} target="_blank" rel="noopener noreferrer" class="btn btn-secondary btn-sm shrink-0">Open file</a>
          {/if}
        </div>
        <span class="md:hidden"><CircleAlert class="w-5 h-5 text-danger" /></span>
      {:else}
        <div class="flex items-center gap-1 md:gap-2 shrink-0">
          <button onclick={() => player.skip(-15)} aria-label="Back 15 seconds" title="Back 15 seconds"
            class="hidden md:inline-flex btn btn-quiet btn-icon relative">
            <RotateCcw class="w-5 h-5" /><span class="absolute text-[0.5rem] font-bold data mt-[1px]">15</span>
          </button>
          <button onclick={() => player.toggle()} aria-label={player.playing ? 'Pause' : 'Play'}
            class="w-10 h-10 rounded-full bg-primary text-primary-ink flex items-center justify-center hover:bg-primary-hover transition-colors cursor-pointer">
            {#if player.playing}<Pause class="w-5 h-5" fill="currentColor" />{:else}<Play class="w-5 h-5 ml-0.5" fill="currentColor" />{/if}
          </button>
          <button onclick={() => player.skip(30)} aria-label="Forward 30 seconds" title="Forward 30 seconds"
            class="hidden md:inline-flex btn btn-quiet btn-icon relative">
            <RotateCw class="w-5 h-5" /><span class="absolute text-[0.5rem] font-bold data mt-[1px]">30</span>
          </button>
        </div>

        <div class="hidden md:flex items-center gap-3 flex-1 min-w-0">
          <span class="text-xs text-ink-2 data w-14 text-right">{formatClock(shown)}</span>
          <input type="range" min="0" max={Math.max(1, Math.floor(player.duration))} step="1"
            value={Math.floor(shown)} oninput={onInput} onchange={onChange}
            aria-label="Seek" disabled={player.duration <= 0}
            class="flex-1 min-w-0 cursor-pointer disabled:cursor-default" />
          <span class="text-xs text-ink-2 data w-16">-{formatClock(remaining)}</span>
          <button onclick={nextRate} title="Playback speed" aria-label="Playback speed {player.rate}×"
            class="btn btn-secondary btn-sm data w-14">{player.rate}×</button>
          <div class="flex items-center gap-1">
            <button onclick={() => player.toggleMute()} aria-label={volumeLabel} title={volumeLabel}
              class="btn btn-quiet btn-icon">
              {#if silent}<VolumeX class="w-5 h-5" />{:else if player.volume < 0.5}<Volume1 class="w-5 h-5" />{:else}<Volume2 class="w-5 h-5" />{/if}
            </button>
            {#if player.volumeAdjustable}
              <input type="range" min="0" max="1" step="0.05" value={silent ? 0 : player.volume} oninput={onVolume}
                aria-label="Volume" class="w-24 cursor-pointer" />
            {/if}
          </div>
        </div>
      {/if}

      {#if isSafeMediaUrl(ep.url)}
        <a href={ep.url} download target="_blank" rel="noopener noreferrer" title="Download episode" aria-label="Download episode"
          class="hidden md:inline-flex btn btn-quiet btn-icon"><Download class="w-5 h-5" /></a>
      {/if}
      <button onclick={() => player.close()} aria-label="Close player" title="Close player"
        class="hidden md:inline-flex btn btn-quiet btn-icon"><X class="w-5 h-5" /></button>
    </div>
  </section>

  <!-- Phone: full now-playing sheet -->
  {#if expanded}
    <div role="dialog" aria-modal="true" aria-label="Now playing"
      class="md:hidden fixed inset-0 z-[60] bg-panel flex flex-col pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] sheet-in">
      <div class="h-14 px-2 flex items-center justify-between border-b border-rule">
        <button onclick={() => { expanded = false; }} aria-label="Close sheet" class="btn btn-quiet btn-icon">
          <ChevronDown class="w-6 h-6" />
        </button>
        <span class="text-sm font-semibold text-ink-2">Now playing</span>
        <button onclick={() => { expanded = false; player.close(); }} class="btn btn-quiet btn-sm">Stop</button>
      </div>

      <div class="flex-1 overflow-y-auto px-6 py-6 flex flex-col items-center gap-6">
        <Cover src={ep.imageUrl} title={ep.podcastTitle} size="fill" class="max-w-[18rem] shadow-[var(--shadow-lift)]" />
        <div class="w-full text-center">
          <h2 class="text-lg font-bold leading-snug">{ep.title}</h2>
          <a href="/discover/podcast?url={encodeURIComponent(ep.podcastUrl)}" onclick={() => { expanded = false; }}
            class="inline-block mt-1 text-sm text-ink-2 underline">{ep.podcastTitle}</a>
        </div>

        {#if player.error}
          <div class="notice w-full border-danger/40">
            <CircleAlert class="w-5 h-5 text-danger shrink-0" />
            <div class="space-y-2">
              <p>{errorText(player.error)}</p>
              {#if isSafeMediaUrl(ep.url)}
                <a href={ep.url} target="_blank" rel="noopener noreferrer" class="btn btn-secondary btn-sm">Open file</a>
              {/if}
            </div>
          </div>
        {:else}
          <div class="w-full">
            <input type="range" min="0" max={Math.max(1, Math.floor(player.duration))} step="1"
              value={Math.floor(shown)} oninput={onInput} onchange={onChange}
              aria-label="Seek" disabled={player.duration <= 0} class="w-full cursor-pointer" />
            <div class="flex justify-between text-xs text-ink-2 data mt-1">
              <span>{formatClock(shown)}</span><span>-{formatClock(remaining)}</span>
            </div>
          </div>

          <div class="flex items-center justify-center gap-6">
            <button onclick={() => player.skip(-15)} aria-label="Back 15 seconds" class="btn btn-quiet w-14 h-14 rounded-full relative">
              <RotateCcw class="w-7 h-7" /><span class="absolute text-[0.625rem] font-bold data mt-[2px]">15</span>
            </button>
            <button onclick={() => player.toggle()} aria-label={player.playing ? 'Pause' : 'Play'}
              class="w-[4.5rem] h-[4.5rem] rounded-full bg-primary text-primary-ink flex items-center justify-center hover:bg-primary-hover transition-colors cursor-pointer shadow-[var(--shadow)]">
              {#if player.playing}<Pause class="w-8 h-8" fill="currentColor" />{:else}<Play class="w-8 h-8 ml-1" fill="currentColor" />{/if}
            </button>
            <button onclick={() => player.skip(30)} aria-label="Forward 30 seconds" class="btn btn-quiet w-14 h-14 rounded-full relative">
              <RotateCw class="w-7 h-7" /><span class="absolute text-[0.625rem] font-bold data mt-[2px]">30</span>
            </button>
          </div>

          <div class="flex items-center gap-3 w-full max-w-xs">
            <button onclick={() => player.toggleMute()} aria-label={volumeLabel} class="btn btn-quiet btn-icon">
              {#if silent}<VolumeX class="w-6 h-6" />{:else if player.volume < 0.5}<Volume1 class="w-6 h-6" />{:else}<Volume2 class="w-6 h-6" />{/if}
            </button>
            {#if player.volumeAdjustable}
              <input type="range" min="0" max="1" step="0.05" value={silent ? 0 : player.volume} oninput={onVolume}
                aria-label="Volume" class="flex-1 min-w-0 cursor-pointer" />
            {/if}
          </div>

          <div class="flex items-center gap-2">
            <button onclick={nextRate} aria-label="Playback speed {player.rate}×" class="btn btn-secondary data">{player.rate}×</button>
            {#if isSafeMediaUrl(ep.url)}
              <a href={ep.url} download target="_blank" rel="noopener noreferrer" class="btn btn-secondary">
                <Download class="w-4 h-4" /> Download
              </a>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
  .sheet-in {
    animation: sheet-in 220ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes sheet-in {
    from {
      transform: translateY(24px);
      opacity: 0.6;
    }
  }
</style>
