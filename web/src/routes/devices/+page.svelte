<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getDevices, type Device } from '$lib/api';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  let devices = $state<Device[]>([]);
  let loading = $state(true);

  onMount(async () => {
    if (!auth.loggedIn || !auth.username) {
      goto('/login');
      return;
    }
    devices = await getDevices(auth.username);
    loading = false;
  });

  const typeIcons: Record<string, string> = {
    mobile: '📱',
    tablet: '📱',
    laptop: '💻',
    desktop: '🖥️',
    server: '🖧',
    other: '📻',
  };
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Devices</h1>

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if devices.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📱</span>
      <h2 class="text-lg font-medium mt-4">No devices registered</h2>
      <p class="text-text-dim mt-2">Connect a podcast app to register a device.</p>
    </div>
  {:else}
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      {#each devices as device}
        <div class="bg-surface border border-border rounded-xl p-5 hover:bg-surface-hover transition-colors">
          <div class="flex items-start gap-3">
            <span class="text-3xl">{typeIcons[device.type] || '📻'}</span>
            <div class="min-w-0 flex-1">
              <h3 class="font-semibold">{device.caption || device.id}</h3>
              <p class="text-sm text-text-dim mt-0.5">
                <code class="text-xs bg-bg px-1.5 py-0.5 rounded">{device.id}</code>
              </p>
              <div class="flex items-center gap-2 mt-2">
                <span class="text-xs px-2 py-0.5 bg-brand-dim text-brand rounded-full capitalize">
                  {device.type}
                </span>
                <span class="text-xs text-text-dim">
                  {device.subscriptions} subscription{device.subscriptions !== 1 ? 's' : ''}
                </span>
              </div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
