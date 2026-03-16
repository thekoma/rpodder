<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getDevices, updateDevice, type Device } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let devices = $state<Device[]>([]);
  let loading = $state(true);
  let loaded = $state(false);
  let editing = $state<string | null>(null);
  let editCaption = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    const username = auth.username;
    if (!username) { goto('/login'); return; }
    getDevices(username).then(devs => { devices = devs; loading = false; }).catch(() => { loading = false; });
  });

  function startEdit(device: Device) {
    editing = device.id;
    editCaption = device.caption;
  }

  async function saveEdit(device: Device) {
    if (!auth.username) return;
    await updateDevice(auth.username, device.id, editCaption, device.type);
    devices = devices.map(d => d.id === device.id ? { ...d, caption: editCaption } : d);
    editing = null;
  }

  const typeIcons: Record<string, string> = {
    mobile: '📱', tablet: '📱', laptop: '💻',
    desktop: '🖥️', server: '🖧', other: '📻',
  };
  const typeOptions = ['mobile', 'tablet', 'laptop', 'desktop', 'server', 'other'];
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
          {#if editing === device.id}
            <!-- Edit mode -->
            <div class="space-y-3">
              <input
                bind:value={editCaption}
                class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand"
                placeholder="Device name"
              />
              <div class="flex gap-2">
                <button onclick={() => saveEdit(device)} class="px-3 py-1 bg-brand text-bg text-xs rounded-md cursor-pointer">Save</button>
                <button onclick={() => { editing = null; }} class="px-3 py-1 bg-surface border border-border text-text-dim text-xs rounded-md cursor-pointer">Cancel</button>
              </div>
            </div>
          {:else}
            <!-- View mode -->
            <div class="flex items-start gap-3">
              <span class="text-3xl">{typeIcons[device.type] || '📻'}</span>
              <div class="min-w-0 flex-1">
                <h3 class="font-semibold">{device.caption || device.id}</h3>
                <p class="text-sm text-text-dim mt-0.5"><code class="text-xs bg-bg px-1.5 py-0.5 rounded">{device.id}</code></p>
                <div class="flex items-center gap-2 mt-2">
                  <span class="text-xs px-2 py-0.5 bg-brand-dim text-brand rounded-full capitalize">{device.type}</span>
                  <span class="text-xs text-text-dim">{device.subscriptions} sub{device.subscriptions !== 1 ? 's' : ''}</span>
                </div>
                <button
                  onclick={() => startEdit(device)}
                  class="mt-2 text-xs text-text-dim hover:text-brand transition-colors cursor-pointer"
                >
                  Rename
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
