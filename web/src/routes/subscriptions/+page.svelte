<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getAllSubscriptions, getDevices, type Device } from '$lib/api';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  let subscriptions = $state<string[]>([]);
  let devices = $state<Device[]>([]);
  let loading = $state(true);

  onMount(async () => {
    if (!auth.loggedIn || !auth.username) {
      goto('/login');
      return;
    }
    const [subs, devs] = await Promise.all([
      getAllSubscriptions(auth.username),
      getDevices(auth.username),
    ]);
    subscriptions = subs;
    devices = devs;
    loading = false;
  });
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">My Subscriptions</h1>
    <a
      href="/discover"
      class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity"
    >
      + Discover
    </a>
  </div>

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if subscriptions.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📭</span>
      <h2 class="text-lg font-medium mt-4">No subscriptions yet</h2>
      <p class="text-text-dim mt-2">Search for podcasts and subscribe to start syncing.</p>
      <a
        href="/discover"
        class="inline-block mt-4 px-6 py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity"
      >
        Discover Podcasts
      </a>
    </div>
  {:else}
    <div class="bg-surface border border-border rounded-xl divide-y divide-border">
      {#each subscriptions as url, i}
        <div class="px-4 py-3 flex items-center gap-3 hover:bg-surface-hover transition-colors">
          <div class="w-8 h-8 rounded bg-brand-dim flex items-center justify-center text-sm shrink-0">
            🎙️
          </div>
          <div class="min-w-0 flex-1">
            <p class="text-sm truncate text-brand">{url}</p>
          </div>
        </div>
      {/each}
    </div>
    <p class="text-sm text-text-dim">{subscriptions.length} subscription{subscriptions.length !== 1 ? 's' : ''}</p>
  {/if}
</div>
