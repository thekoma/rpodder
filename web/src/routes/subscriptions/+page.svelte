<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getAllSubscriptions, getPodcastInfo, getDevices, uploadSubscriptionChanges, getSubscriptionUpgrades, type PodcastInfo, type UpgradeableSub } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  interface SubInfo {
    url: string;
    info: PodcastInfo | null;
  }

  let subs = $state<SubInfo[]>([]);
  let upgrades = $state<UpgradeableSub[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let deviceId = $state('');
  let upgradeMsg = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;

    const username = auth.username;
    if (!username) { goto('/login'); return; }

    Promise.all([getAllSubscriptions(username), getDevices(username), getSubscriptionUpgrades()]).then(([urls, devs, upg]) => {
      subs = urls.map(url => ({ url, info: null }));
      deviceId = devs.length > 0 ? devs[0].id : 'web';
      upgrades = upg;
      loading = false;

      // Enrich with metadata in background
      for (let i = 0; i < subs.length; i++) {
        getPodcastInfo(subs[i].url).then(info => {
          if (info) subs[i] = { ...subs[i], info };
        });
      }
    }).catch(() => { loading = false; });
  });

  async function unsubscribe(url: string) {
    if (!auth.username) return;
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, [], [url]);
    if (ok) {
      subs = subs.filter(s => s.url !== url);
    }
  }

  async function upgradeToHttps(upgrade: UpgradeableSub) {
    if (!auth.username) return;
    // Unsubscribe from HTTP, subscribe to HTTPS
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, [upgrade.https_url], [upgrade.http_url]);
    if (ok) {
      subs = subs.map(s => s.url === upgrade.http_url ? { ...s, url: upgrade.https_url } : s);
      upgrades = upgrades.filter(u => u.http_url !== upgrade.http_url);
      upgradeMsg = `Upgraded "${upgrade.title}" to HTTPS`;
      setTimeout(() => { upgradeMsg = ''; }, 3000);
    }
  }

  async function upgradeAll() {
    if (!auth.username) return;
    const add = upgrades.map(u => u.https_url);
    const remove = upgrades.map(u => u.http_url);
    const ok = await uploadSubscriptionChanges(auth.username, deviceId, add, remove);
    if (ok) {
      subs = subs.map(s => {
        const upg = upgrades.find(u => u.http_url === s.url);
        return upg ? { ...s, url: upg.https_url } : s;
      });
      const count = upgrades.length;
      upgrades = [];
      upgradeMsg = `Upgraded ${count} subscription${count > 1 ? 's' : ''} to HTTPS`;
      setTimeout(() => { upgradeMsg = ''; }, 3000);
    }
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">My Subscriptions</h1>
    <a href="/discover" class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity no-underline">+ Discover</a>
  </div>

  {#if upgradeMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{upgradeMsg}</div>
  {/if}

  <!-- HTTPS upgrade banner -->
  {#if upgrades.length > 0}
    <div class="bg-surface border border-yellow-600/30 rounded-xl p-4">
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-2">
          <span class="text-yellow-500">🔒</span>
          <h3 class="text-sm font-semibold">HTTPS upgrades available</h3>
        </div>
        <button onclick={upgradeAll}
          class="px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md hover:opacity-90 cursor-pointer">
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
            <button onclick={() => upgradeToHttps(upgrade)}
              class="text-xs text-brand hover:text-blue-400 cursor-pointer ml-2 shrink-0">
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
            onclick={(e) => { e.preventDefault(); e.stopPropagation(); unsubscribe(sub.url); }}
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
