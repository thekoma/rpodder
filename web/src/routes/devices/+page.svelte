<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getDevices, updateDevice, deleteDevice, getSyncStatus, updateSyncStatus, renameSyncGroup, type Device, type SyncStatus } from '$lib/api';
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

  // Sync group creation
  let selectingForGroup = $state(false);
  let selectedDevices = $state<Set<string>>(new Set());
  let newGroupName = $state('');

  // Group rename
  let renamingGroup = $state<string | null>(null);
  let renameValue = $state('');

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

  function startEdit(device: Device) { editing = device.id; editCaption = device.caption; }

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
      syncStatus = await getSyncStatus(auth.username);
    }
    deleting = null;
  }

  // --- Sync helpers ---

  function getGroupIndex(deviceId: string): number {
    if (!syncStatus) return -1;
    return syncStatus.groups.findIndex(g => g.devices.includes(deviceId));
  }

  function isSynced(deviceId: string): boolean { return getGroupIndex(deviceId) >= 0; }

  function getDeviceCaption(deviceId: string): string {
    const d = devices.find(dev => dev.id === deviceId);
    return d ? (d.caption || d.id) : deviceId;
  }

  function getDeviceType(deviceId: string): string {
    return devices.find(d => d.id === deviceId)?.type || 'other';
  }

  function groupLabel(g: { name: string }, i: number): string {
    return g.name || `Group ${i + 1}`;
  }

  async function showMsg(msg: string) {
    syncMsg = msg;
    setTimeout(() => { syncMsg = ''; }, 3000);
  }

  async function removeFromGroup(deviceId: string) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const newGroups = syncStatus.groups
      .map(g => ({ devices: g.devices.filter(id => id !== deviceId), name: g.name }))
      .filter(g => g.devices.length > 1);
    const ok = await updateSyncStatus(auth.username, newGroups, [deviceId]);
    if (ok) { syncStatus = await getSyncStatus(auth.username); showMsg('Device removed from sync group'); }
    else showMsg('Failed to update');
    savingSync = false;
  }

  async function addToExistingGroup(deviceId: string, groupIndex: number) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const newGroups = syncStatus.groups.map((g, i) => ({
      devices: i === groupIndex ? [...g.devices, deviceId] : g.devices,
      name: g.name,
    }));
    const ok = await updateSyncStatus(auth.username, newGroups);
    if (ok) { syncStatus = await getSyncStatus(auth.username); showMsg('Device added to sync group'); }
    else showMsg('Failed to update');
    savingSync = false;
  }

  function toggleSelect(deviceId: string) {
    const next = new Set(selectedDevices);
    if (next.has(deviceId)) next.delete(deviceId); else next.add(deviceId);
    selectedDevices = next;
  }

  function startCreateGroup() { selectingForGroup = true; selectedDevices = new Set(); newGroupName = ''; }
  function cancelCreateGroup() { selectingForGroup = false; selectedDevices = new Set(); newGroupName = ''; }

  async function confirmCreateGroup() {
    if (!auth.username || !syncStatus || selectedDevices.size < 2) return;
    savingSync = true;
    const existingGroups = syncStatus.groups.map(g => ({ devices: g.devices, name: g.name }));
    const newGroup = { devices: [...selectedDevices], name: newGroupName.trim() || undefined };
    const ok = await updateSyncStatus(auth.username, [...existingGroups, newGroup]);
    if (ok) { syncStatus = await getSyncStatus(auth.username); showMsg('Sync group created'); }
    else showMsg('Failed to create group');
    savingSync = false;
    selectingForGroup = false;
    selectedDevices = new Set();
    newGroupName = '';
  }

  async function dissolveGroup(groupIndex: number) {
    if (!auth.username || !syncStatus) return;
    const group = syncStatus.groups[groupIndex];
    if (!confirm(`Remove sync group "${groupLabel(group, groupIndex)}"? Devices will keep their subscriptions but stop syncing.`)) return;
    savingSync = true;
    const newGroups = syncStatus.groups.filter((_, i) => i !== groupIndex).map(g => ({ devices: g.devices, name: g.name }));
    const ok = await updateSyncStatus(auth.username, newGroups, group.devices);
    if (ok) { syncStatus = await getSyncStatus(auth.username); showMsg('Sync group dissolved'); }
    else showMsg('Failed to update');
    savingSync = false;
  }

  function startRename(groupId: string, currentName: string) {
    renamingGroup = groupId;
    renameValue = currentName;
  }

  async function saveRename() {
    if (!renamingGroup || !auth.username) return;
    const ok = await renameSyncGroup(renamingGroup, renameValue.trim());
    if (ok) { syncStatus = await getSyncStatus(auth.username); showMsg('Group renamed'); }
    else showMsg('Failed to rename');
    renamingGroup = null;
    renameValue = '';
  }

  const typeIcons: Record<string, string> = {
    mobile: '📱', tablet: '📱', laptop: '💻', desktop: '🖥️', server: '🖧', other: '📻',
  };

  const groupColors = [
    'border-blue-500/40 bg-blue-500/5',
    'border-purple-500/40 bg-purple-500/5',
    'border-emerald-500/40 bg-emerald-500/5',
    'border-amber-500/40 bg-amber-500/5',
  ];

  const groupDotColors = ['bg-blue-500', 'bg-purple-500', 'bg-emerald-500', 'bg-amber-500'];
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
    <!-- Device cards -->
    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
      {#each devices as device}
        {@const gi = getGroupIndex(device.id)}
        <div class="bg-surface border border-border rounded-xl p-5 hover:bg-surface-hover transition-colors group relative">
          {#if gi >= 0 && syncStatus}
            <div class="absolute top-3 right-3 flex items-center gap-1.5" title={groupLabel(syncStatus.groups[gi], gi)}>
              <span class="w-2.5 h-2.5 rounded-full {groupDotColors[gi % groupDotColors.length]}"></span>
              <span class="text-xs text-text-dim">{groupLabel(syncStatus.groups[gi], gi)}</span>
            </div>
          {/if}

          {#if editing === device.id}
            <div class="space-y-3">
              <input bind:value={editCaption}
                class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand"
                placeholder="Device name" />
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
                </div>
                <div class="flex gap-3 mt-3 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button onclick={() => startEdit(device)} class="text-xs text-text-dim hover:text-brand transition-colors cursor-pointer">Rename</button>
                  <button onclick={() => confirmDelete(device)} disabled={deleting === device.id}
                    class="text-xs text-danger hover:text-red-400 transition-colors cursor-pointer disabled:opacity-50"
                  >{deleting === device.id ? 'Deleting...' : 'Delete'}</button>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <!-- Sync Groups -->
    {#if devices.length > 1}
      <div class="space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-lg font-semibold flex items-center gap-2">
              <svg class="w-5 h-5 text-brand" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5" />
              </svg>
              Sync Groups
            </h2>
            <p class="text-xs text-text-dim mt-0.5">Devices in the same group share subscriptions automatically.</p>
          </div>
          {#if !selectingForGroup}
            <button onclick={startCreateGroup} disabled={savingSync || (syncStatus?.['not-synchronized']?.length ?? 0) < 2}
              class="px-3 py-1.5 bg-brand text-bg text-xs font-medium rounded-lg cursor-pointer hover:opacity-90 disabled:opacity-50"
              title={((syncStatus?.['not-synchronized']?.length ?? 0) < 2) ? 'Need at least 2 ungrouped devices' : 'Create a new sync group'}
            >New group</button>
          {/if}
        </div>

        <!-- Create group -->
        {#if selectingForGroup && syncStatus}
          <div class="bg-surface border-2 border-dashed border-brand/40 rounded-xl p-4 space-y-3">
            <p class="text-sm text-text-dim">Select devices and give the group a name:</p>
            <input bind:value={newGroupName} placeholder="Group name (optional)"
              class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand" />
            <div class="flex flex-wrap gap-2">
              {#each syncStatus['not-synchronized'] as deviceId}
                <button onclick={() => toggleSelect(deviceId)}
                  class="flex items-center gap-2 px-3 py-2 rounded-lg border text-sm cursor-pointer transition-all
                    {selectedDevices.has(deviceId)
                      ? 'bg-brand/10 border-brand text-text ring-1 ring-brand'
                      : 'bg-bg border-border text-text-dim hover:border-brand/50'}">
                  <span>{typeIcons[getDeviceType(deviceId)] || '📻'}</span>
                  <span>{getDeviceCaption(deviceId)}</span>
                  {#if selectedDevices.has(deviceId)}
                    <svg class="w-4 h-4 text-brand" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
                  {/if}
                </button>
              {/each}
            </div>
            <div class="flex items-center gap-2">
              <button onclick={confirmCreateGroup} disabled={selectedDevices.size < 2 || savingSync}
                class="px-4 py-1.5 bg-brand text-bg text-xs font-medium rounded-lg cursor-pointer disabled:opacity-50"
              >{savingSync ? 'Creating...' : `Create group (${selectedDevices.size} devices)`}</button>
              <button onclick={cancelCreateGroup} class="px-3 py-1.5 text-xs text-text-dim hover:text-text cursor-pointer">Cancel</button>
            </div>
          </div>
        {/if}

        <!-- Existing groups -->
        {#if syncStatus && syncStatus.groups.length > 0}
          {#each syncStatus.groups as group, i}
            <div class="rounded-xl p-4 border {groupColors[i % groupColors.length]}">
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-2">
                  <span class="w-2.5 h-2.5 rounded-full {groupDotColors[i % groupDotColors.length]}"></span>
                  {#if renamingGroup === group.id}
                    <input bind:value={renameValue} placeholder="Group name"
                      onkeydown={(e) => { if (e.key === 'Enter') saveRename(); if (e.key === 'Escape') { renamingGroup = null; } }}
                      class="px-2 py-0.5 bg-bg border border-border rounded text-sm text-text focus:outline-none focus:border-brand w-40" />
                    <button onclick={saveRename} class="text-xs text-brand hover:underline cursor-pointer">Save</button>
                    <button onclick={() => { renamingGroup = null; }} class="text-xs text-text-dim hover:text-text cursor-pointer">Cancel</button>
                  {:else}
                    <span class="text-sm font-medium">{groupLabel(group, i)}</span>
                    <button onclick={() => startRename(group.id, group.name)}
                      class="text-text-dim hover:text-brand transition-colors cursor-pointer" title="Rename group">
                      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931z" /></svg>
                    </button>
                    <span class="text-xs text-text-dim">{group.devices.length} devices</span>
                  {/if}
                </div>
                <button onclick={() => dissolveGroup(i)} disabled={savingSync}
                  class="text-xs text-text-dim hover:text-danger transition-colors cursor-pointer disabled:opacity-50">Dissolve</button>
              </div>
              <div class="flex flex-wrap gap-2">
                {#each group.devices as deviceId}
                  <div class="flex items-center gap-2 bg-bg/80 px-3 py-2 rounded-lg text-sm">
                    <span>{typeIcons[getDeviceType(deviceId)] || '📻'}</span>
                    <span class="font-medium">{getDeviceCaption(deviceId)}</span>
                    {#if group.devices.length > 2}
                      <button onclick={() => removeFromGroup(deviceId)} disabled={savingSync}
                        class="text-text-dim hover:text-danger transition-colors cursor-pointer disabled:opacity-50" title="Remove from group">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
                      </button>
                    {/if}
                  </div>
                {/each}
                {#if syncStatus['not-synchronized'].length > 0}
                  <select onchange={(e) => { const t = e.target as HTMLSelectElement; if (t.value) { addToExistingGroup(t.value, i); t.value = ''; } }}
                    disabled={savingSync}
                    class="appearance-none bg-bg/50 border border-dashed border-border text-text-dim px-3 py-2 rounded-lg text-sm cursor-pointer hover:border-brand/50 transition-colors disabled:opacity-50">
                    <option value="">+ Add device</option>
                    {#each syncStatus['not-synchronized'] as deviceId}
                      <option value={deviceId}>{getDeviceCaption(deviceId)}</option>
                    {/each}
                  </select>
                {/if}
              </div>
            </div>
          {/each}
        {:else if !selectingForGroup}
          <div class="text-center py-6 bg-surface border border-border rounded-xl">
            <p class="text-sm text-text-dim">No sync groups configured. Create one to keep subscriptions in sync across devices.</p>
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</div>
