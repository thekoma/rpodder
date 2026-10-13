<script lang="ts">
  import {
    Smartphone, Tablet, Laptop, Monitor, Server, Radio, Globe, FileDown, Pencil, Trash2, X, Plus, Link2, Unlink,
  } from '@lucide/svelte';
  import { auth } from '$lib/auth.svelte';
  import { getDevices, updateDevice, deleteDevice, getSyncStatus, updateSyncStatus, renameSyncGroup, type Device, type SyncStatus } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { WEB_DEVICE_ID } from '$lib/webDevice';
  import { plural } from '$lib/format';
  import { notify, notifyError } from '$lib/toast.svelte';

  let devices = $state<Device[]>([]);
  let syncStatus = $state<SyncStatus | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let editing = $state<string | null>(null);
  let editCaption = $state('');
  let deleting = $state<string | null>(null);
  let savingSync = $state(false);

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
    }).catch(() => {}).finally(() => { loading = false; });
  });

  function startEdit(device: Device) { editing = device.id; editCaption = device.caption; }

  async function saveEdit(device: Device) {
    if (!auth.username) return;
    const caption = editCaption.trim();
    const ok = await updateDevice(auth.username, device.id, caption, device.type);
    if (ok) {
      devices = devices.map(d => d.id === device.id ? { ...d, caption } : d);
      notify('Device renamed');
    } else {
      notifyError("Couldn't rename the device.");
    }
    editing = null;
  }

  async function confirmDelete(device: Device) {
    if (!auth.username) return;
    if (!confirm(`Remove "${device.caption || device.id}"? Its subscription list on the server is deleted too. The app re-registers the next time it syncs.`)) return;
    deleting = device.id;
    const ok = await deleteDevice(auth.username, device.id);
    if (ok) {
      devices = devices.filter(d => d.id !== device.id);
      syncStatus = await getSyncStatus(auth.username);
      notify('Device removed');
    } else {
      notifyError("Couldn't remove the device.");
    }
    deleting = null;
  }

  // --- Sync helpers ---

  function getGroupIndex(deviceId: string): number {
    if (!syncStatus) return -1;
    return syncStatus.groups.findIndex(g => g.devices.includes(deviceId));
  }

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

  function groupColor(i: number): string {
    return `var(--group-${(i % 4) + 1})`;
  }

  async function removeFromGroup(deviceId: string) {
    if (!auth.username || !syncStatus) return;
    savingSync = true;
    const newGroups = syncStatus.groups
      .map(g => ({ devices: g.devices.filter(id => id !== deviceId), name: g.name }))
      .filter(g => g.devices.length > 1);
    const ok = await updateSyncStatus(auth.username, newGroups, [deviceId]);
    if (ok) { syncStatus = await getSyncStatus(auth.username); notify('Device left the sync group'); }
    else notifyError("Couldn't update the sync group.");
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
    if (ok) { syncStatus = await getSyncStatus(auth.username); notify('Device joined the sync group'); }
    else notifyError("Couldn't update the sync group.");
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
    if (ok) { syncStatus = await getSyncStatus(auth.username); notify('Sync group created'); }
    else notifyError("Couldn't create the sync group.");
    savingSync = false;
    selectingForGroup = false;
    selectedDevices = new Set();
    newGroupName = '';
  }

  async function dissolveGroup(groupIndex: number) {
    if (!auth.username || !syncStatus) return;
    const group = syncStatus.groups[groupIndex];
    if (!confirm(`Dissolve "${groupLabel(group, groupIndex)}"? Each device keeps its current subscriptions but stops sharing new ones.`)) return;
    savingSync = true;
    const newGroups = syncStatus.groups.filter((_, i) => i !== groupIndex).map(g => ({ devices: g.devices, name: g.name }));
    const ok = await updateSyncStatus(auth.username, newGroups, group.devices);
    if (ok) { syncStatus = await getSyncStatus(auth.username); notify('Sync group dissolved'); }
    else notifyError("Couldn't dissolve the sync group.");
    savingSync = false;
  }

  function startRename(groupId: string, currentName: string) {
    renamingGroup = groupId;
    renameValue = currentName;
  }

  async function saveRename() {
    if (!renamingGroup || !auth.username) return;
    const ok = await renameSyncGroup(renamingGroup, renameValue.trim());
    if (ok) { syncStatus = await getSyncStatus(auth.username); notify('Group renamed'); }
    else notifyError("Couldn't rename the group.");
    renamingGroup = null;
    renameValue = '';
  }

  const typeIcons: Record<string, typeof Smartphone> = {
    mobile: Smartphone, tablet: Tablet, laptop: Laptop, desktop: Monitor, server: Server, other: Radio,
  };

  function iconFor(deviceId: string, type: string) {
    return deviceId === WEB_DEVICE_ID ? Globe : (typeIcons[type] ?? Radio);
  }

  let ungrouped = $derived(syncStatus?.['not-synchronized'] ?? []);
</script>

<svelte:head><title>Devices · rpodder</title></svelte:head>

<div class="space-y-10">
  <header>
    <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">Devices</h1>
    <p class="text-sm text-ink-2 mt-1">Each app registers itself the first time it syncs. Devices in a sync group share their subscriptions.</p>
  </header>

  {#if loading}
    <div class="panel divide-y divide-rule" aria-busy="true">
      {#each [0, 1, 2] as i (i)}
        <div class="flex items-center gap-4 px-4 py-4"><div class="skeleton w-9 h-9"></div><div class="flex-1 space-y-2"><div class="skeleton h-4 w-40"></div><div class="skeleton h-3 w-24"></div></div></div>
      {/each}
    </div>
  {:else if devices.length === 0}
    <div class="catalog-card p-6 sm:p-8 max-w-2xl space-y-2">
      <h2 class="text-lg font-bold">No devices yet</h2>
      <p class="text-ink-2">Add this server to AntennaPod, gPodder or Kasts with your username and password. The app appears here after its first sync.</p>
      <a href="https://thekoma.github.io/rpodder/" target="_blank" rel="noopener noreferrer" class="inline-block text-sm font-semibold text-primary hover:underline">How to connect an app</a>
    </div>
  {:else}
    <!-- Devices -->
    <section aria-labelledby="devices-heading" class="space-y-3">
      <h2 id="devices-heading" class="text-base font-bold">Your devices <span class="text-brass font-semibold data text-sm ml-1">{devices.length}</span></h2>
      <ul class="panel divide-y divide-rule">
        {#each devices as device (device.id)}
          {@const gi = getGroupIndex(device.id)}
          {@const Icon = iconFor(device.id, device.type)}
          <li class="flex flex-wrap sm:flex-nowrap items-center gap-x-4 gap-y-3 px-4 py-3.5">
            <span class="w-9 h-9 rounded-[6px] bg-panel-2 border border-rule flex items-center justify-center shrink-0">
              <Icon class="w-[1.125rem] h-[1.125rem] text-ink-2" />
            </span>

            {#if editing === device.id}
              <form class="flex flex-1 min-w-0 gap-2" onsubmit={(e) => { e.preventDefault(); saveEdit(device); }}>
                <!-- svelte-ignore a11y_autofocus -->
                <input bind:value={editCaption} class="field h-9 text-sm flex-1 min-w-0" placeholder="Device name" aria-label="Device name" autofocus
                  onkeydown={(e) => { if (e.key === 'Escape') editing = null; }} />
                <button type="submit" class="btn btn-primary btn-sm h-9">Save</button>
                <button type="button" onclick={() => { editing = null; }} class="btn btn-quiet btn-sm h-9">Cancel</button>
              </form>
            {:else}
              <div class="min-w-0 flex-1 basis-[calc(100%-3.25rem)] sm:basis-auto">
                <p class="font-semibold leading-snug flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span class="truncate">{device.caption || device.id}</span>
                  {#if device.id === WEB_DEVICE_ID}<span class="tag bg-primary-soft text-primary">Plays in the browser</span>{/if}
                </p>
                <p class="text-xs text-ink-3 mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5">
                  <span class="data">{device.id}</span>
                  <span class="capitalize">{device.type}</span>
                  <span>{plural(device.subscriptions, 'subscription')}</span>
                </p>
              </div>

              <div class="flex items-center gap-1 ml-[3.25rem] sm:ml-auto">
                {#if gi >= 0 && syncStatus}
                  <span class="tag border border-rule bg-panel mr-2" title="Sync group">
                    <span class="w-2 h-2 rounded-[2px]" style="background: {groupColor(gi)}"></span>
                    {groupLabel(syncStatus.groups[gi], gi)}
                  </span>
                {/if}
                {#if auth.username}
                  <a href="/subscriptions/{encodeURIComponent(auth.username)}/{encodeURIComponent(device.id)}.opml"
                    download="rpodder-{device.id}.opml" rel="external"
                    class="btn btn-quiet btn-sm btn-icon" aria-label="Export {device.caption || device.id} subscriptions as OPML" title="Export OPML">
                    <FileDown class="w-4 h-4" />
                  </a>
                {/if}
                <button onclick={() => startEdit(device)} class="btn btn-quiet btn-sm btn-icon" aria-label="Rename {device.caption || device.id}" title="Rename">
                  <Pencil class="w-4 h-4" />
                </button>
                <button onclick={() => confirmDelete(device)} disabled={deleting === device.id}
                  class="btn btn-quiet btn-sm btn-icon hover:!text-danger" aria-label="Remove {device.caption || device.id}" title="Remove">
                  <Trash2 class="w-4 h-4" />
                </button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <!-- Sync groups -->
    <section aria-labelledby="groups-heading" class="space-y-3">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 id="groups-heading" class="text-base font-bold">Sync groups</h2>
          <p class="text-sm text-ink-2 mt-0.5">Subscribe on one device in a group and every other device in it gets the podcast too.</p>
        </div>
        {#if !selectingForGroup}
          <button onclick={startCreateGroup} disabled={savingSync || ungrouped.length < 2} class="btn btn-secondary btn-sm"
            title={ungrouped.length < 2 ? 'Needs at least two devices that are not in a group' : undefined}>
            <Plus class="w-4 h-4" /> New group
          </button>
        {/if}
      </div>

      {#if selectingForGroup && syncStatus}
        <form class="catalog-card p-4 sm:p-5 space-y-4" onsubmit={(e) => { e.preventDefault(); confirmCreateGroup(); }}>
          <div>
            <label for="group-name" class="label">Group name</label>
            <input id="group-name" bind:value={newGroupName} placeholder="For example: My phones" class="field max-w-sm" />
          </div>
          <fieldset class="space-y-2">
            <legend class="label">Devices to sync <span class="font-normal text-ink-3">(at least two)</span></legend>
            <div class="flex flex-wrap gap-2">
              {#each ungrouped as deviceId (deviceId)}
                {@const Icon = iconFor(deviceId, getDeviceType(deviceId))}
                {@const on = selectedDevices.has(deviceId)}
                <label class="flex items-center gap-2 h-10 px-3 rounded-[6px] border text-sm cursor-pointer transition-colors
                  {on ? 'border-primary bg-primary-soft text-ink' : 'border-rule-strong bg-panel text-ink-2 hover:border-primary'}">
                  <input type="checkbox" checked={on} onchange={() => toggleSelect(deviceId)} />
                  <Icon class="w-4 h-4" />
                  {getDeviceCaption(deviceId)}
                </label>
              {/each}
            </div>
          </fieldset>
          <div class="flex items-center gap-2">
            <button type="submit" disabled={selectedDevices.size < 2 || savingSync} class="btn btn-primary btn-sm">
              {savingSync ? 'Creating…' : `Create group${selectedDevices.size ? ` with ${selectedDevices.size}` : ''}`}
            </button>
            <button type="button" onclick={cancelCreateGroup} class="btn btn-quiet btn-sm">Cancel</button>
          </div>
        </form>
      {/if}

      {#if syncStatus && syncStatus.groups.length > 0}
        <div class="grid gap-3 md:grid-cols-2">
          {#each syncStatus.groups as group, i (group.id)}
            <div class="panel overflow-hidden rounded-t-none" style="border-top: 3px solid {groupColor(i)}">
              <div class="flex items-center justify-between gap-2 px-4 pt-3 pb-2">
                {#if renamingGroup === group.id}
                  <form class="flex items-center gap-2 flex-1 min-w-0" onsubmit={(e) => { e.preventDefault(); saveRename(); }}>
                    <!-- svelte-ignore a11y_autofocus -->
                    <input bind:value={renameValue} placeholder="Group name" aria-label="Group name" autofocus
                      onkeydown={(e) => { if (e.key === 'Escape') renamingGroup = null; }}
                      class="field h-8 text-sm flex-1 min-w-0" />
                    <button type="submit" class="btn btn-primary btn-sm">Save</button>
                    <button type="button" onclick={() => { renamingGroup = null; }} class="btn btn-quiet btn-sm">Cancel</button>
                  </form>
                {:else}
                  <div class="flex items-center gap-2 min-w-0">
                    <Link2 class="w-4 h-4 shrink-0" style="color: {groupColor(i)}" />
                    <span class="font-bold truncate">{groupLabel(group, i)}</span>
                    <button onclick={() => startRename(group.id, group.name)} class="btn btn-quiet btn-sm btn-icon" aria-label="Rename group" title="Rename group">
                      <Pencil class="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <button onclick={() => dissolveGroup(i)} disabled={savingSync} class="btn btn-quiet btn-sm shrink-0 hover:!text-danger">
                    <Unlink class="w-4 h-4" /> Dissolve
                  </button>
                {/if}
              </div>
              <ul class="px-4 pb-3 flex flex-wrap gap-2">
                {#each group.devices as deviceId (deviceId)}
                  {@const Icon = iconFor(deviceId, getDeviceType(deviceId))}
                  <li class="flex items-center gap-2 h-9 pl-2.5 pr-1 rounded-[6px] bg-panel-2 border border-rule text-sm">
                    <Icon class="w-4 h-4 text-ink-2" />
                    <span class="font-semibold">{getDeviceCaption(deviceId)}</span>
                    {#if group.devices.length > 2}
                      <button onclick={() => removeFromGroup(deviceId)} disabled={savingSync}
                        class="btn btn-quiet btn-sm btn-icon w-7 h-7" aria-label="Remove {getDeviceCaption(deviceId)} from group" title="Remove from group">
                        <X class="w-3.5 h-3.5" />
                      </button>
                    {:else}
                      <span class="w-1"></span>
                    {/if}
                  </li>
                {/each}
                {#if ungrouped.length > 0}
                  <li>
                    <select aria-label="Add a device to {groupLabel(group, i)}" disabled={savingSync}
                      onchange={(e) => { const t = e.target as HTMLSelectElement; if (t.value) { addToExistingGroup(t.value, i); t.value = ''; } }}
                      class="field h-9 text-sm w-auto border-dashed bg-transparent">
                      <option value="">Add device…</option>
                      {#each ungrouped as deviceId (deviceId)}
                        <option value={deviceId}>{getDeviceCaption(deviceId)}</option>
                      {/each}
                    </select>
                  </li>
                {/if}
              </ul>
            </div>
          {/each}
        </div>
      {:else if !selectingForGroup}
        <p class="panel px-4 py-5 text-sm text-ink-2">
          {devices.length < 2
            ? 'Sync groups need at least two devices. Connect another app to create one.'
            : 'No sync groups yet. Each device keeps its own subscriptions until you group them.'}
        </p>
      {/if}
      {#if devices.some(d => d.id === WEB_DEVICE_ID)}
        <p class="hint">The web player's sync group and position sync are set in <a href="/settings" class="underline hover:text-ink">Settings</a>.</p>
      {/if}
    </section>
  {/if}
</div>
