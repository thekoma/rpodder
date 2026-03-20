<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getDevices, updateDevice, deleteDevice, getSyncStatus, updateSyncStatus, type Device, type SyncStatus } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let devices = $state<Device[]>([]);
  let syncStatus = $state<SyncStatus | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let editing = $state<string | null>(null);
  let editCaption = $state('');
  let deleting = $state<string | null>(null);
  let savingSync = $state(false);
  let syncMsg = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    const username = auth.username;
    if (!username) { goto('/login'); return; }
    Promise.all([getDevices(username), getSyncStatus(username)]).then(([devs, sync]) => {
      devices = devs;
      syncStatus = sync;
      loading = false;
    }).catch(() => { loading = false; });
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

  async function confirmDelete(device: Device) {
    if (!auth.username) return;
    if (!confirm(`Delete device "${device.caption || device.id}"? This will also remove its subscriptions.`)) return;
    deleting = device.id;
    const ok = await deleteDevice(auth.username, device.id);
    if (ok) {
      devices = devices.filter(d => d.id !== device.id);
      // Refresh sync status after delete
      syncStatus = await getSyncStatus(auth.username);
    }
    deleting = null;
  }

  // --- Sync group helpers ---

  function getGroupForDevice(deviceId: string): number {
    if (!syncStatus) return -1;
    for (let i = 0; i < syncStatus.synchronized.length; i++) {
      if (syncStatus.synchronized[i].includes(deviceId)) return i;
    }
    return -1;
  }

  function isSynced(deviceId: string): boolean {
    return getGroupForDevice(deviceId) >= 0;
  }

  async function addToGroup(deviceId: string, groupIndex: number) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const newGroups = syncStatus.synchronized.map((g, i) =>
      i === groupIndex ? [...g, deviceId] : g
    );
    const ok = await updateSyncStatus(auth.username, newGroups);
    if (ok) {
      syncStatus = await getSyncStatus(auth.username);
      syncMsg = 'Sync group updated';
    } else {
      syncMsg = 'Failed to update sync group';
    }
    savingSync = false;
    setTimeout(() => { syncMsg = ''; }, 3000);
  }

  async function removeFromGroup(deviceId: string) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const ok = await updateSyncStatus(auth.username, syncStatus.synchronized, [deviceId]);
    if (ok) {
      syncStatus = await getSyncStatus(auth.username);
      syncMsg = 'Device removed from sync group';
    } else {
      syncMsg = 'Failed to update sync group';
    }
    savingSync = false;
    setTimeout(() => { syncMsg = ''; }, 3000);
  }

  async function createNewGroup(deviceIds: string[]) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const newGroups = [...syncStatus.synchronized, deviceIds];
    const ok = await updateSyncStatus(auth.username, newGroups);
    if (ok) {
      syncStatus = await getSyncStatus(auth.username);
      syncMsg = 'Sync group created';
    } else {
      syncMsg = 'Failed to create sync group';
    }
    savingSync = false;
    setTimeout(() => { syncMsg = ''; }, 3000);
  }

  // State for the "add to group" UI
  let groupingDevice = $state<string | null>(null);
  let selectedPartner = $state<string>('');

  function startGrouping(deviceId: string) {
    groupingDevice = deviceId;
    selectedPartner = '';
  }

  function cancelGrouping() {
    groupingDevice = null;
    selectedPartner = '';
  }

  async function confirmGrouping() {
    if (!groupingDevice || !selectedPartner) return;
    const existingGroup = getGroupForDevice(selectedPartner);
    if (existingGroup >= 0) {
      // Add to existing group
      await addToGroup(groupingDevice, existingGroup);
    } else {
      // Create new group with both devices
      await createNewGroup([groupingDevice, selectedPartner]);
    }
    groupingDevice = null;
    selectedPartner = '';
  }

  const typeIcons: Record<string, string> = {
    mobile: '📱', tablet: '📱', laptop: '💻',
    desktop: '🖥️', server: '🖧', other: '📻',
  };

  function getDeviceCaption(deviceId: string): string {
    const d = devices.find(dev => dev.id === deviceId);
    return d ? (d.caption || d.id) : deviceId;
  }
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Devices</h1>

  {#if syncMsg}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{syncMsg}</div>
  {/if}

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if devices.length === 0}
    <div class="text-center py-16 bg-surface border border-border rounded-xl">
      <span class="text-5xl">📱</span>
      <h2 class="text-lg font-medium mt-4">No devices registered</h2>
      <p class="text-text-dim mt-2">Connect a podcast app to register a device.</p>
    </div>
  {:else}
    <!-- Sync Groups -->
    {#if syncStatus && (syncStatus.synchronized.length > 0 || devices.length > 1)}
      <div class="space-y-3">
        <h2 class="text-lg font-semibold flex items-center gap-2">
          <svg class="w-5 h-5 text-brand" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5" />
          </svg>
          Sync Groups
        </h2>
        <p class="text-xs text-text-dim">Devices in the same sync group share subscriptions automatically.</p>

        {#if syncStatus.synchronized.length > 0}
          {#each syncStatus.synchronized as group, i}
            <div class="bg-surface border border-brand/30 rounded-xl p-4">
              <div class="flex items-center gap-2 mb-3">
                <span class="text-xs font-medium px-2 py-0.5 bg-brand-dim text-brand rounded-full">Group {i + 1}</span>
                <span class="text-xs text-text-dim">{group.length} devices synced</span>
              </div>
              <div class="flex flex-wrap gap-2">
                {#each group as deviceId}
                  <div class="flex items-center gap-1.5 bg-bg px-3 py-1.5 rounded-lg text-sm">
                    <span>{typeIcons[devices.find(d => d.id === deviceId)?.type || 'other'] || '📻'}</span>
                    <span>{getDeviceCaption(deviceId)}</span>
                    <button
                      onclick={() => removeFromGroup(deviceId)}
                      disabled={savingSync}
                      class="ml-1 text-text-dim hover:text-danger transition-colors cursor-pointer disabled:opacity-50"
                      title="Remove from group"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}

        {#if syncStatus['not-synchronized'].length > 1 || (syncStatus['not-synchronized'].length > 0 && syncStatus.synchronized.length > 0)}
          <div class="bg-surface border border-border rounded-xl p-4">
            <p class="text-xs text-text-dim mb-3">Not synced:</p>
            <div class="flex flex-wrap gap-2">
              {#each syncStatus['not-synchronized'] as deviceId}
                <div class="flex items-center gap-1.5 bg-bg px-3 py-1.5 rounded-lg text-sm">
                  <span>{typeIcons[devices.find(d => d.id === deviceId)?.type || 'other'] || '📻'}</span>
                  <span>{getDeviceCaption(deviceId)}</span>
                  {#if groupingDevice === deviceId}
                    <!-- Grouping mode for this device -->
                  {:else}
                    <button
                      onclick={() => startGrouping(deviceId)}
                      disabled={savingSync}
                      class="ml-1 text-text-dim hover:text-brand transition-colors cursor-pointer disabled:opacity-50"
                      title="Add to sync group"
                    >
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" /></svg>
                    </button>
                  {/if}
                </div>
              {/each}
            </div>

            {#if groupingDevice}
              <div class="mt-3 p-3 bg-bg border border-border rounded-lg">
                <p class="text-xs text-text-dim mb-2">Sync <strong>{getDeviceCaption(groupingDevice)}</strong> with:</p>
                <div class="flex flex-wrap gap-2">
                  {#each devices.filter(d => d.id !== groupingDevice) as d}
                    <button
                      onclick={() => { selectedPartner = d.id; }}
                      class="px-3 py-1.5 text-xs rounded-lg border cursor-pointer transition-colors
                        {selectedPartner === d.id
                          ? 'bg-brand text-bg border-brand'
                          : 'bg-surface border-border text-text-dim hover:text-text hover:border-brand'}"
                    >
                      {typeIcons[d.type] || '📻'} {d.caption || d.id}
                      {#if isSynced(d.id)}
                        <span class="opacity-60">(Group {getGroupForDevice(d.id) + 1})</span>
                      {/if}
                    </button>
                  {/each}
                </div>
                <div class="flex gap-2 mt-3">
                  <button
                    onclick={confirmGrouping}
                    disabled={!selectedPartner || savingSync}
                    class="px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md cursor-pointer disabled:opacity-50"
                  >
                    {savingSync ? 'Saving...' : 'Sync'}
                  </button>
                  <button
                    onclick={cancelGrouping}
                    class="px-3 py-1 bg-surface border border-border text-text-dim text-xs rounded-md cursor-pointer"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <!-- Device cards -->
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      {#each devices as device}
        <div class="bg-surface border border-border rounded-xl p-5 hover:bg-surface-hover transition-colors group">
          {#if editing === device.id}
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
            <div class="flex items-start gap-3">
              <span class="text-3xl">{typeIcons[device.type] || '📻'}</span>
              <div class="min-w-0 flex-1">
                <h3 class="font-semibold">{device.caption || device.id}</h3>
                <p class="text-sm text-text-dim mt-0.5"><code class="text-xs bg-bg px-1.5 py-0.5 rounded">{device.id}</code></p>
                <div class="flex items-center gap-2 mt-2">
                  <span class="text-xs px-2 py-0.5 bg-brand-dim text-brand rounded-full capitalize">{device.type}</span>
                  <span class="text-xs text-text-dim">{device.subscriptions} sub{device.subscriptions !== 1 ? 's' : ''}</span>
                  {#if isSynced(device.id)}
                    <span class="text-xs px-2 py-0.5 bg-green-900/30 text-green-400 rounded-full">
                      synced
                    </span>
                  {/if}
                </div>
                <div class="flex gap-3 mt-3 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onclick={() => startEdit(device)}
                    class="text-xs text-text-dim hover:text-brand transition-colors cursor-pointer"
                  >
                    Rename
                  </button>
                  <button
                    onclick={() => confirmDelete(device)}
                    disabled={deleting === device.id}
                    class="text-xs text-danger hover:text-red-400 transition-colors cursor-pointer disabled:opacity-50"
                  >
                    {deleting === device.id ? 'Deleting...' : 'Delete'}
                  </button>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
