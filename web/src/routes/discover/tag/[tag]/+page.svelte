<script lang="ts">
  import { ArrowLeft, Plus, Check } from '@lucide/svelte';
  import { getPodcastsForTag, type PodcastInfo } from '#lib/api.js';
  import { auth, isAuthError, sessionExpired } from '#lib/auth.svelte.js';
  import { subscribe as subscribeEverywhere } from '#lib/webDevice.js';
  import { browser } from '$app/env';
  import { page } from '$app/state';
  import Cover from '#lib/components/Cover.svelte';
  import { plainText } from '#lib/format.js';
  import { notify, notifyError } from '#lib/toast.svelte.js';

  let podcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);
  let tag = $state('');
  let subscribing = $state<string | null>(null);
  let added = $state<Set<string>>(new Set());

  // React to URL changes (fixes SPA navigation)
  $effect(() => {
    if (!browser) return;
    const newTag = page.params.tag || '';
    if (newTag === tag) return;
    tag = newTag;
    loading = true;
    getPodcastsForTag(decodeURIComponent(tag), 50).then(p => {
      if (newTag === tag) podcasts = p;
    }).catch(() => {}).finally(() => { if (newTag === tag) loading = false; });
  });

  async function subscribe(podcast: PodcastInfo) {
    const username = auth.username;
    if (!username) return;
    subscribing = podcast.url;
    try {
      await subscribeEverywhere(username, podcast.url);
      added = new Set([...added, podcast.url]);
      notify(`Subscribed to ${podcast.title}`);
    } catch (e) {
      if (isAuthError(e)) { subscribing = null; sessionExpired(username); return; }
      notifyError(`Couldn't subscribe to ${podcast.title}, try again.`);
    }
    subscribing = null;
  }
</script>

<svelte:head><title>{decodeURIComponent(tag)} · rpodder</title></svelte:head>

<div class="space-y-4">
  <div class="flex items-center gap-3">
    <a href="/discover" class="btn btn-quiet btn-sm btn-icon" aria-label="Back to Browse"><ArrowLeft class="w-4 h-4" /></a>
    <h2 class="text-xl font-bold capitalize">{decodeURIComponent(tag)}</h2>
  </div>

  {#if loading}
    <div class="panel divide-y divide-rule" aria-busy="true">
      {#each [0, 1, 2, 3] as i (i)}
        <div class="flex items-center gap-4 px-4 py-3"><div class="skeleton w-14 h-14"></div><div class="flex-1 space-y-2"><div class="skeleton h-4 w-1/2"></div><div class="skeleton h-3 w-1/3"></div></div></div>
      {/each}
    </div>
  {:else if podcasts.length === 0}
    <p class="panel p-6 text-ink-2">No podcasts in this category yet.</p>
  {:else}
    <ul class="panel divide-y divide-rule">
      {#each podcasts as podcast (podcast.url)}
        <li class="flex items-center gap-4 pl-4 pr-3 py-3 hover:bg-panel-2 transition-colors">
          <a href="/discover/podcast?url={encodeURIComponent(podcast.url)}" class="group flex gap-4 flex-1 min-w-0">
            <Cover src={podcast.logo_url} title={podcast.title} size="md" />
            <span class="min-w-0">
              <span class="block font-semibold leading-snug line-clamp-1 group-hover:underline">{podcast.title}</span>
              {#if podcast.author}<span class="block text-sm text-ink-2 truncate">{podcast.author}</span>{/if}
              {#if podcast.description}<span class="block text-sm text-ink-3 mt-0.5 line-clamp-1">{plainText(podcast.description)}</span>{/if}
            </span>
          </a>
          {#if auth.loggedIn}
            {#if added.has(podcast.url)}
              <span class="btn btn-sm text-success shrink-0"><Check class="w-4 h-4" /> Added</span>
            {:else}
              <button onclick={() => subscribe(podcast)} disabled={subscribing === podcast.url} class="btn btn-secondary btn-sm shrink-0">
                <Plus class="w-4 h-4" /> {subscribing === podcast.url ? 'Adding…' : 'Subscribe'}
              </button>
            {/if}
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
