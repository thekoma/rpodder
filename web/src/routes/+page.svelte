<script lang="ts">
  import { auth } from '#lib/auth.svelte.js';
  import { getToplist, type PodcastInfo } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import Cover from '#lib/components/Cover.svelte';

  let topPodcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);
  let loaded = $state(false);

  $effect(() => {
    if (!browser) return;
    // Signed in, the library is home.
    if (auth.loggedIn) { goto('/subscriptions', { replaceState: true }); return; }
    if (loaded) return;
    loaded = true;
    getToplist(12).then((top) => { topPodcasts = top; }).catch(() => {}).finally(() => { loading = false; });
  });

  // An illustrative catalog card (not real data): how rpodder records one feed.
  const example = {
    title: 'A podcast you follow',
    feed: 'https://feeds.example.com/show.xml',
    devices: [
      { name: 'Phone', held: true },
      { name: 'Tablet', held: true },
      { name: 'Laptop', held: false },
      { name: 'Web player', held: true },
    ],
  };
</script>

<svelte:head><title>rpodder</title></svelte:head>

{#if !auth.loggedIn}
  <div class="space-y-14">
    <section class="grid gap-8 lg:grid-cols-[1.1fr_1fr] items-center pt-4 sm:pt-10">
      <div class="space-y-5">
        <h1 class="text-[2.25rem] sm:text-[2.75rem] font-bold tracking-[-0.03em] leading-[1.08]">
          Your podcasts, in step on every device.
        </h1>
        <p class="text-lg text-ink-2 max-w-[38rem]">
          rpodder is a self-hosted sync server for the gpodder API. Point your podcast apps at it and manage the whole library from here.
        </p>
        <div class="flex flex-wrap gap-3">
          <a href="/login" class="btn btn-primary h-11 px-5 text-[0.9375rem]">Sign in</a>
          <a href="/discover" class="btn btn-secondary h-11 px-5 text-[0.9375rem]">Browse the directory</a>
        </div>
      </div>
      <figure class="catalog-card shadow-[var(--shadow)] p-5 sm:p-6 space-y-4" aria-label="Example of a catalog card">
        <div class="flex items-start justify-between gap-3">
          <p class="text-lg font-bold">{example.title}</p>
          <span class="stamp text-ink-3">Example</span>
        </div>
        <dl class="grid grid-cols-[6.5rem_1fr] gap-x-4 gap-y-2.5 text-sm border-t border-card-rule pt-4">
          <dt class="text-ink-3 font-semibold">Feed</dt>
          <dd class="data text-[0.8125rem] text-ink-2 truncate">{example.feed}</dd>
          <dt class="text-ink-3 font-semibold">On devices</dt>
          <dd class="flex flex-wrap gap-1.5">
            {#each example.devices as d (d.name)}
              <span class="tag border {d.held ? 'border-rule-strong bg-panel text-ink' : 'border-dashed border-rule text-ink-3'}">
                <span class="w-2 h-2 rounded-[2px] border" style="border-color: var(--group-1); background: {d.held ? 'var(--group-1)' : 'transparent'}"></span>
                {d.name}
              </span>
            {/each}
          </dd>
          <dt class="text-ink-3 font-semibold">Last episode</dt>
          <dd class="flex items-center gap-2"><span class="stamp text-red">12 min left</span><span class="text-ink-3">on Phone</span></dd>
        </dl>
        <figcaption class="text-sm text-ink-2 border-t border-card-rule pt-4">
          Every feed you follow gets a card like this: where it is subscribed, and where you stopped listening. Export them all as OPML whenever you like.
        </figcaption>
      </figure>
    </section>

    <section aria-labelledby="popular-heading" class="space-y-4">
      <div class="flex items-baseline justify-between gap-4">
        <h2 id="popular-heading" class="text-lg font-bold">Popular on this server</h2>
        <a href="/discover/toplist" class="text-sm font-semibold text-primary hover:underline">Top podcasts</a>
      </div>
      {#if loading}
        <div class="grid grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 gap-4" aria-busy="true">
          {#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton aspect-square"></div>{/each}
        </div>
      {:else if topPodcasts.length > 0}
        <ul class="grid grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 gap-x-4 gap-y-5">
          {#each topPodcasts as podcast (podcast.url)}
            <li>
              <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group block space-y-2">
                <Cover src={podcast.logo_url} title={podcast.title} size="fill" />
                <span class="block text-sm font-semibold leading-snug line-clamp-2 group-hover:underline">{podcast.title}</span>
              </a>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="panel p-6 text-ink-2">Nothing indexed yet. Podcasts show up here once people on this server subscribe to them.</p>
      {/if}
    </section>
  </div>
{/if}
