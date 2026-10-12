<script lang="ts">
  import { auth, isAuthError, sessionExpired } from '$lib/auth.svelte';
  import { changeMyPassword, getMe, type MeInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { player } from '$lib/player.svelte';
  import { WEB_DEVICE_ID, getWebSettings, groupLabel, joinSyncGroup, setSyncPositions, type WebSettings } from '$lib/webDevice';

  let me = $state<MeInfo | null>(null);
  let oldPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let success = $state('');
  let loading = $state(false);
  let loaded = $state(false);

  // Web player
  let web = $state<WebSettings | null>(null);
  let webLoadError = $state('');
  let webError = $state('');
  let syncOn = $state(true);
  let selectedGroupId = $state('');
  let savingSync = $state(false);
  let savingGroup = $state(false);

  let currentGroup = $derived(web?.group ?? null);
  let selectedGroup = $derived(web?.groups.find(g => g.id === selectedGroupId) ?? null);
  let groupChanged = $derived(selectedGroupId !== (currentGroup?.id ?? ''));
  let dissolvedPeer = $derived(
    groupChanged && currentGroup && currentGroup.devices.length === 2
      ? currentGroup.devices.find(d => d !== WEB_DEVICE_ID) ?? null
      : null
  );

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username) { goto('/login'); return; }
    getMe().then(m => { me = m; });
    loadWebSettings(auth.username);
  });

  async function loadWebSettings(username: string) {
    try {
      const s = await getWebSettings(username);
      web = s;
      syncOn = s.syncPositions;
      selectedGroupId = s.group?.id ?? '';
      webLoadError = '';
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return; }
      if (!web) webLoadError = "Couldn't load web player settings.";
    }
  }

  async function onSyncChange(e: Event) {
    const username = auth.username;
    if (!username) return;
    const on = (e.currentTarget as HTMLInputElement).checked;
    const previous = !on;
    savingSync = true;
    webError = '';
    try {
      await setSyncPositions(username, on);
      await player.setSync(username, on);
    } catch (err) {
      if (isAuthError(err)) { savingSync = false; sessionExpired(username); return; }
      syncOn = previous;
      webError = "Couldn't save, try again.";
    }
    await loadWebSettings(username);
    savingSync = false;
  }

  async function saveGroup() {
    const username = auth.username;
    if (!username || !groupChanged) return;
    savingGroup = true;
    webError = '';
    try {
      await joinSyncGroup(username, selectedGroup);
    } catch (err) {
      if (isAuthError(err)) { savingGroup = false; sessionExpired(username); return; }
      selectedGroupId = currentGroup?.id ?? '';
      webError = "Couldn't save, try again.";
    }
    await loadWebSettings(username);
    savingGroup = false;
  }

  async function handleChangePassword(e: Event) {
    e.preventDefault();
    error = ''; success = '';
    if (newPassword.length < 4) { error = 'Password must be at least 4 characters'; return; }
    if (newPassword !== confirmPassword) { error = 'Passwords do not match'; return; }

    loading = true;
    const result = await changeMyPassword(oldPassword || null, newPassword);
    loading = false;

    if (result.ok) {
      success = 'Password changed successfully';
      oldPassword = ''; newPassword = ''; confirmPassword = '';
    } else {
      error = result.error || 'Failed to change password';
    }
  }
</script>

<div class="max-w-lg mx-auto space-y-6">
  <h1 class="text-2xl font-bold">Account Settings</h1>

  {#if me}
    <div class="bg-surface border border-border rounded-xl p-5">
      <h2 class="text-lg font-semibold mb-3">Profile</h2>
      <div class="space-y-2 text-sm">
        <div><span class="text-text-dim">Username:</span> <span class="font-medium">{me.username}</span></div>
        <div><span class="text-text-dim">Email:</span> <span class="font-medium">{me.email || 'not set'}</span></div>
        <div><span class="text-text-dim">Role:</span> <span class="font-medium">{me.is_admin ? 'Admin' : 'User'}</span></div>
      </div>
    </div>
  {/if}

  <div class="bg-surface border border-border rounded-xl p-5">
    <h2 class="text-lg font-semibold mb-3">Web player</h2>

    {#if webLoadError}
      <p class="text-danger text-sm">{webLoadError}</p>
    {:else if !web}
      <p class="text-text-dim text-sm">Loading...</p>
    {:else}
      <div class="space-y-5">
        <div>
          <label class="flex items-start gap-3 cursor-pointer">
            <input type="checkbox" bind:checked={syncOn} onchange={onSyncChange} disabled={savingSync}
              class="mt-0.5 accent-brand cursor-pointer" />
            <span>
              <span class="block text-sm">Sync listening positions with my other devices</span>
              <span class="block text-xs text-text-dim mt-1">When off, positions stay in this browser only.</span>
            </span>
          </label>
        </div>

        <div>
          <label for="web-group" class="block text-sm text-text-dim mb-1">Sync group</label>
          <div class="flex gap-2">
            <select id="web-group" bind:value={selectedGroupId} disabled={savingGroup}
              class="flex-1 min-w-0 px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand transition-colors">
              <option value="">None</option>
              {#each web.groups as g (g.id)}
                <option value={g.id}>{groupLabel(g)}</option>
              {/each}
            </select>
            <button onclick={saveGroup} disabled={!groupChanged || savingGroup}
              class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer shrink-0">
              {savingGroup ? 'Saving...' : 'Save'}
            </button>
          </div>
          <p class="text-xs text-text-dim mt-1">The web player shares subscriptions with the devices in this group.</p>
          {#if groupChanged && selectedGroup}
            <p class="text-xs text-yellow-500 mt-2">
              The web player's current subscriptions will be added to every device in "{groupLabel(selectedGroup)}", and theirs to the web player. This can't be undone.
            </p>
          {/if}
          {#if currentGroup && dissolvedPeer}
            <p class="text-xs text-yellow-500 mt-2">
              "{groupLabel(currentGroup)}" will be dissolved: {dissolvedPeer} will no longer sync with anything.
            </p>
          {/if}
        </div>

        {#if webError}<p class="text-danger text-sm">{webError}</p>{/if}
      </div>
    {/if}
  </div>

  <div class="bg-surface border border-border rounded-xl p-5">
    <h2 class="text-lg font-semibold mb-3">Change Password</h2>

    <form onsubmit={handleChangePassword} class="space-y-4">
      <div>
        <label for="old-password" class="block text-sm text-text-dim mb-1">Current Password</label>
        <input id="old-password" type="password" bind:value={oldPassword} autocomplete="current-password"
          placeholder="Leave blank if you logged in via SSO"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand transition-colors" />
        <p class="text-xs text-text-dim mt-1">SSO users who never set a password can leave this blank.</p>
      </div>

      <div>
        <label for="new-password" class="block text-sm text-text-dim mb-1">New Password</label>
        <input id="new-password" type="password" bind:value={newPassword} required autocomplete="new-password"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand transition-colors" />
      </div>

      <div>
        <label for="confirm-password" class="block text-sm text-text-dim mb-1">Confirm New Password</label>
        <input id="confirm-password" type="password" bind:value={confirmPassword} required autocomplete="new-password"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text text-sm focus:outline-none focus:border-brand transition-colors" />
      </div>

      {#if error}<p class="text-danger text-sm">{error}</p>{/if}
      {#if success}<p class="text-success text-sm">{success}</p>{/if}

      <button type="submit" disabled={loading}
        class="px-4 py-2 bg-brand text-bg text-sm font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer">
        {loading ? 'Changing...' : 'Change Password'}
      </button>
    </form>
  </div>
</div>
