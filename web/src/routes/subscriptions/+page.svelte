<script lang="ts">
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { getAllSubscriptions, getPodcastInfo, getSubscriptionUpgrades, type PodcastInfo, type UpgradeableSub } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import ContinueListening from '$lib/components/ContinueListening.svelte';
  import { normalizeUrl } from '$lib/playerLogic';
  import { devicesWith, replaceSubscriptions, unsubscribe as unsubscribeFrom } from '$lib/webDevice';

  interface SubInfo {
    url: string;
    info: PodcastInfo | null;
  }

  let subs = $state<SubInfo[]>([]);
  let upgrades = $state<UpgradeableSub[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let busy = $state(false);
  let upgradeMsg = $state('');
  let errorMsg = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    const username = auth.username;
    if (!username) { goto('/login'); return; }

    reload(username).catch(() => {}).finally(() => { loading = false; });
  });

  // Re-read subscriptions and upgrades, keeping already fetched metadata.
  async function reload(username: string) {
    const known = new Map(subs.map(s => [s.url, s.info]));
    const [urls, upg] = await Promise.all([getAllSubscriptions(username), getSubscriptionUpgrades()]);
    subs = urls.map(url => ({ url, info: known.get(url) ?? null }));
    upgrades = upg;

    // Enrich with metadata in background
    for (const sub of subs) {
      if (sub.info) continue;
      const url = sub.url;
      getPodcastInfo(url).then(info => {
        if (!info) return;
        const i = subs.findIndex(s => s.url === url);
        if (i >= 0) subs[i] = { ...subs[i], info };
      });
    }
  }

  function flash(msg: string) {
    upgradeMsg = msg;
    setTimeout(() => { upgradeMsg = ''; }, 3000);
  }

  function fail(e: unknown, username: string, msg: string): boolean {
    if (isAuthError(e)) { sessionExpired(username); return true; }
    errorMsg = msg;
    return false;
  }

  async function unsubscribe(sub: SubInfo) {
    const username = auth.username;
    if (!username || busy) return;
    busy = true;
    errorMsg = '';
    try {
      const owners = (await devicesWith(username, [sub.url])).get(normalizeUrl(sub.url)) ?? [];
      if (owners.length === 0) {
        errorMsg = 'Not subscribed on any device.';
      } else {
        const title = sub.info?.title || sub.url;
        const names = owners.map(d => d.caption || d.id).join(', ');
        if (confirm(`Unsubscribe from ${title} on: ${names}?`)) {
          await unsubscribeFrom(username, owners, sub.url);
        }
      }
      await reload(username);
    } catch (e) {
      if (fail(e, username, "Couldn't unsubscribe, try again.")) return;
      await reload(username).catch(() => {});
    } finally {
      busy = false;
    }
  }

  async function upgradeToHttps(upgrade: UpgradeableSub) {
    const username = auth.username;
    if (!username || busy) return;
    busy = true;
    errorMsg = '';
    try {
      await replaceSubscriptions(username, [{ from: upgrade.http_url, to: upgrade.https_url }]);
      flash(`Upgraded "${upgrade.title}" to HTTPS`);
      await reload(username);
    } catch (e) {
      if (fail(e, username, "Couldn't upgrade, try again.")) return;
      await reload(username).catch(() => {});
    } finally {
      busy = false;
    }
  }

  async function upgradeAll() {
    const username = auth.username;
    if (!username || busy || upgrades.length === 0) return;
    busy = true;
    errorMsg = '';
    const count = upgrades.length;
    try {
      await replaceSubscriptions(username, upgrades.map(u => ({ from: u.http_url, to: u.https_url })));
      flash(`Upgraded ${count} subscription${count > 1 ? 's' : ''} to HTTPS`);
      await reload(username);
    } catch (e) {
      if (fail(e, username, "Couldn't upgrade, try again.")) return;
      await reload(username).catch(() => {});
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">My Subscriptions</h1>
    <a href="/discover" class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity no-underline">+ Discover</a>
  </div>

  <ContinueListening />

  {#if upgradeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{upgradeMsg}</div>
  {/if}
  {#if errorMsg}
    <div class="bg-surface border border-danger/40 text-danger px-4 py-2 rounded-lg text-sm text-center">{errorMsg}</div>
  {/if}

  <!-- HTTPS upgrade banner -->
  {#if upgrades.length > 0}
    <div class="bg-surface border border-yellow-600/30 rounded-xl p-4">
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-2">
          <span class="text-yellow-500">🔒</span>
          <h3 class="text-sm font-semibold">HTTPS upgrades available</h3>
        </div>
        <button onclick={upgradeAll} disabled={busy}
          class="px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md hover:opacity-90 disabled:opacity-50 cursor-pointer">
          Upgrade all ({upgrades.length})
        </button>
      </div>
      <p class="text-xs text-text-dim mb-3">
        {upgrades.length === 1 ? 'This subscription has' : 'These subscriptions have'} a secure HTTPS feed available.
      </p>
      <div class="space-y-1">
        {#each upgrades as upgrade}
          <div class="flex items-center justify-between py-1">
            <span class="text-sm truncate flex-1">{upgrade.title || upgrade.http_url}</span>
            <button onclick={() => upgradeToHttps(upgrade)} disabled={busy}
              class="disabled:opacity-50 text-xs text-brand hover:text-blue-400 cursor-pointer ml-2 shrink-0">
              Upgrade
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if subs.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📭</span>
      <h2 class="text-lg font-medium mt-4">No subscriptions yet</h2>
      <p class="text-text-dim mt-2">Search for podcasts and subscribe to start syncing.</p>
      <a href="/discover" class="inline-block mt-4 px-6 py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity no-underline">Discover Podcasts</a>
    </div>
  {:else}
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      {#each subs as sub}
        <div class="bg-surface border border-border rounded-xl p-4 hover:bg-surface-hover transition-colors group relative">
          <a href="/discover/podcast?url={encodeURIComponent(sub.url)}" class="flex gap-3 no-underline text-text">
            {#if sub.info?.logo_url}
              <img src={sub.info.logo_url} alt="" class="w-16 h-16 rounded-lg object-cover shrink-0" loading="lazy" />
            {:else}
              <div class="w-16 h-16 rounded-lg bg-brand-dim flex items-center justify-center text-2xl shrink-0">🎙</div>
            {/if}
            <div class="min-w-0 flex-1">
              <h3 class="font-medium text-sm line-clamp-2">{sub.info?.title || sub.url.split('/').pop() || sub.url}</h3>
              {#if sub.info?.author}
                <p class="text-xs text-text-dim mt-1 truncate">{sub.info.author}</p>
              {/if}
            </div>
          </a>
          <button
            onclick={(e) => { e.preventDefault(); e.stopPropagation(); unsubscribe(sub); }}
            disabled={busy}
            class="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-danger hover:text-red-400 bg-bg/80 px-2 py-1 rounded cursor-pointer"
          >
            Unsubscribe
          </button>
        </div>
      {/each}
    </div>
    <p class="text-sm text-text-dim">{subs.length} subscription{subs.length !== 1 ? 's' : ''}</p>
  {/if}
</div>
