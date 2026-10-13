<script lang="ts">
  import { Rss } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { looksLikeFeedUrl } from '$lib/format';
  import { notify } from '$lib/toast.svelte';
  import { subscribe } from '$lib/webDevice';

  // Subscribe to a feed by its address, then open it.
  let { value = $bindable(''), autofocus = false }: { value?: string; autofocus?: boolean } = $props();

  const uid = $props.id();
  let busy = $state(false);
  let error = $state('');

  async function submit(e: Event) {
    e.preventDefault();
    const username = auth.username;
    const url = value.trim();
    if (!username || busy) return;
    if (!looksLikeFeedUrl(url)) {
      error = 'Enter the full feed address, starting with https:// or http://';
      return;
    }
    busy = true;
    error = '';
    try {
      await subscribe(username, url);
      notify('Feed added to your library');
      value = '';
      goto(`/discover/podcast?url=${encodeURIComponent(url)}`);
    } catch (err) {
      if (isAuthError(err)) { sessionExpired(username); return; }
      error = "Couldn't add the feed, try again.";
    } finally {
      busy = false;
    }
  }
</script>

<form onsubmit={submit} class="space-y-1.5" novalidate>
  <label for="{uid}-url" class="label">Feed address</label>
  <div class="flex gap-2">
    <div class="relative flex-1 min-w-0">
      <Rss class="w-4 h-4 text-ink-3 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
      <!-- svelte-ignore a11y_autofocus -->
      <input id="{uid}-url" type="url" inputmode="url" autocomplete="off" spellcheck="false"
        bind:value placeholder="https://example.com/feed.xml" autofocus={autofocus}
        aria-invalid={!!error} aria-describedby="{uid}-help"
        class="field pl-9 data text-sm" />
    </div>
    <button type="submit" class="btn btn-primary" disabled={busy || !value.trim()}>
      {busy ? 'Adding…' : 'Add feed'}
    </button>
  </div>
  {#if error}
    <p id="{uid}-help" class="text-sm text-danger">{error}</p>
  {:else}
    <p id="{uid}-help" class="hint">RSS or Atom address. Private feeds with a token in the URL stay out of the public directory.</p>
  {/if}
</form>
