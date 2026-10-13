<script lang="ts">
  import { FileDown, FileText, Braces, TriangleAlert } from '@lucide/svelte';
  import { auth, isAuthError, sessionExpired } from '#lib/auth.svelte.js';
  import { changeMyPassword, getMe, type MeInfo } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import { player } from '#lib/player.svelte.js';
  import ThemeSwitch from '#lib/components/ThemeSwitch.svelte';
  import { notify } from '#lib/toast.svelte.js';
  import { WEB_DEVICE_ID, getWebSettings, groupLabel, joinSyncGroup, setSyncPositions, type WebSettings } from '#lib/webDevice.js';

  let me = $state<MeInfo | null>(null);
  let oldPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
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
      if (!web) webLoadError = "Couldn't load the web player settings.";
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
      notify(selectedGroup ? 'Web player joined the sync group' : 'Web player left its sync group');
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
    error = '';
    if (newPassword.length < 4) { error = 'The new password needs at least 4 characters.'; return; }
    if (newPassword !== confirmPassword) { error = "The two new passwords don't match."; return; }

    loading = true;
    const result = await changeMyPassword(oldPassword || null, newPassword);
    loading = false;

    if (result.ok) {
      notify('Password changed. Other browsers and apps using a session were signed out.');
      oldPassword = ''; newPassword = ''; confirmPassword = '';
    } else {
      error = result.error === 'current password required'
        ? 'Enter your current password.'
        : result.error === 'incorrect old password'
          ? 'The current password is wrong.'
          : (result.error || "Couldn't change the password.");
    }
  }

  let user = $derived(auth.username ?? '');
  let exportBase = $derived(`/subscriptions/${encodeURIComponent(user)}`);
</script>

<svelte:head><title>Settings · rpodder</title></svelte:head>

{#snippet section(title: string, description: string)}
  <div>
    <h2 class="text-base font-bold">{title}</h2>
    <p class="text-sm text-ink-2 mt-1 max-w-[34ch]">{description}</p>
  </div>
{/snippet}

<div class="space-y-8 max-w-4xl">
  <header>
    <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">Settings</h1>
  </header>

  <div class="divide-y divide-rule border-y border-rule">
    <!-- Profile -->
    <section class="grid gap-4 md:grid-cols-[16rem_1fr] py-7">
      {@render section('Account', 'How this server knows you.')}
      <dl class="panel grid grid-cols-[7rem_1fr] gap-x-4 gap-y-2.5 p-4 text-sm">
        <dt class="text-ink-3 font-semibold">Username</dt><dd class="font-semibold">{me?.username ?? user}</dd>
        <dt class="text-ink-3 font-semibold">Email</dt><dd>{me ? (me.email || 'Not set') : '…'}</dd>
        <dt class="text-ink-3 font-semibold">Role</dt><dd>{me ? (me.is_admin ? 'Administrator' : 'Member') : '…'}</dd>
      </dl>
    </section>

    <!-- Appearance -->
    <section class="grid gap-4 md:grid-cols-[16rem_1fr] py-7">
      {@render section('Appearance', 'Saved in this browser only.')}
      <div><ThemeSwitch /></div>
    </section>

    <!-- Web player -->
    <section class="grid gap-4 md:grid-cols-[16rem_1fr] py-7">
      {@render section('Web player', 'The browser player counts as one more device of yours.')}
      <div class="space-y-5">
        {#if webLoadError}
          <p class="text-sm text-danger">{webLoadError}</p>
        {:else if !web}
          <div class="space-y-3" aria-busy="true"><div class="skeleton h-5 w-2/3"></div><div class="skeleton h-9 w-full max-w-md"></div></div>
        {:else}
          <label class="flex items-start gap-3 cursor-pointer">
            <input type="checkbox" bind:checked={syncOn} onchange={onSyncChange} disabled={savingSync} class="mt-1 w-4 h-4 cursor-pointer" />
            <span>
              <span class="block font-semibold">Sync listening positions with my other devices</span>
              <span class="block text-sm text-ink-2 mt-0.5">Resume in the browser where your phone stopped, and the other way round. When off, positions stay in this browser.</span>
            </span>
          </label>

          <div class="space-y-1.5">
            <label for="web-group" class="label">Sync group</label>
            <div class="flex gap-2 max-w-lg">
              <select id="web-group" bind:value={selectedGroupId} disabled={savingGroup} class="field flex-1 min-w-0">
                <option value="">None — subscribe on every device</option>
                {#each web.groups as g (g.id)}
                  <option value={g.id}>{groupLabel(g)}</option>
                {/each}
              </select>
              <button onclick={saveGroup} disabled={!groupChanged || savingGroup} class="btn btn-primary shrink-0">
                {savingGroup ? 'Saving…' : 'Save'}
              </button>
            </div>
            <p class="hint">Podcasts you subscribe to here go to the devices of this group. With no group, they go to all your devices.</p>
            {#if groupChanged && selectedGroup}
              <p class="notice text-sm mt-2 bg-warning-soft border-warning/40">
                <TriangleAlert class="w-4 h-4 text-warning shrink-0 mt-0.5" />
                <span>The web player's subscriptions will be added to every device in “{groupLabel(selectedGroup)}”, and theirs to the web player. This can't be undone.</span>
              </p>
            {/if}
            {#if currentGroup && dissolvedPeer}
              <p class="notice text-sm mt-2 bg-warning-soft border-warning/40">
                <TriangleAlert class="w-4 h-4 text-warning shrink-0 mt-0.5" />
                <span>“{groupLabel(currentGroup)}” will be dissolved: {dissolvedPeer} will no longer sync with anything.</span>
              </p>
            {/if}
          </div>

          {#if webError}<p class="text-sm text-danger">{webError}</p>{/if}
        {/if}
      </div>
    </section>

    <!-- Export -->
    <section class="grid gap-4 md:grid-cols-[16rem_1fr] py-7">
      {@render section('Your data', 'Take your subscriptions to any podcast app.')}
      <div class="flex flex-wrap gap-2 content-start">
        <a href="{exportBase}.opml" download="rpodder-{user}.opml" rel="external" class="btn btn-secondary">
          <FileDown class="w-4 h-4" /> OPML
        </a>
        <a href="{exportBase}.txt" download="rpodder-{user}.txt" rel="external" class="btn btn-secondary">
          <FileText class="w-4 h-4" /> Plain list
        </a>
        <a href="{exportBase}.json" download="rpodder-{user}.json" rel="external" class="btn btn-secondary">
          <Braces class="w-4 h-4" /> JSON
        </a>
        <p class="hint basis-full mt-1">All subscriptions across your devices. Single devices can be exported from <a href="/devices" class="underline hover:text-ink">Devices</a>.</p>
      </div>
    </section>

    <!-- Password -->
    <section class="grid gap-4 md:grid-cols-[16rem_1fr] py-7">
      {@render section('Password', 'Changing it signs out every other browser and app that uses a session.')}
      <form onsubmit={handleChangePassword} class="space-y-4 max-w-sm">
        <div>
          <label for="old-password" class="label">Current password</label>
          <input id="old-password" type="password" bind:value={oldPassword} autocomplete="current-password" class="field" />
          <p class="hint mt-1">Signed in with single sign-on and never set one? Leave it empty.</p>
        </div>
        <div>
          <label for="new-password" class="label">New password</label>
          <input id="new-password" type="password" bind:value={newPassword} required minlength="4" autocomplete="new-password" class="field" />
        </div>
        <div>
          <label for="confirm-password" class="label">Repeat new password</label>
          <input id="confirm-password" type="password" bind:value={confirmPassword} required minlength="4" autocomplete="new-password" class="field" />
        </div>
        {#if error}<p class="text-sm text-danger" role="alert">{error}</p>{/if}
        <button type="submit" disabled={loading} class="btn btn-primary">
          {loading ? 'Changing…' : 'Change password'}
        </button>
      </form>
    </section>
  </div>
</div>
