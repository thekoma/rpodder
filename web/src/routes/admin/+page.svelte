<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import {
    getAdminUsers, getAdminStats, createAdminUser, deactivateUser, activateUser,
    setUserRole, deleteUser, forceUpdateFeeds, getBuildInfo,
    adminResetPassword, adminSetPassword,
    type AdminUser, type AdminStats, type BuildInfo
  } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let users = $state<AdminUser[]>([]);
  let adminStats = $state<AdminStats | null>(null);
  let health = $state<BuildInfo | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let showCreateForm = $state(false);
  let newUsername = $state('');
  let newPassword = $state('');
  let newEmail = $state('');
  let creating = $state(false);
  let feedUpdating = $state(false);
  let message = $state('');
  let setPasswordUser = $state('');
  let setPasswordValue = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username || !auth.isAdmin) { goto('/'); return; }
    loadData();
  });

  function loadData() {
    Promise.all([getAdminUsers(), getAdminStats(), getBuildInfo()]).then(([u, s, h]) => {
      users = u;
      adminStats = s;
      health = h;
      loading = false;
    }).catch(() => { loading = false; });
  }

  function showMessage(msg: string, timeout = 3000) {
    message = msg;
    setTimeout(() => { message = ''; }, timeout);
  }

  async function handleCreate() {
    if (!newUsername.trim() || !newPassword.trim()) return;
    creating = true;
    const ok = await createAdminUser(newUsername, newPassword, newEmail || undefined);
    creating = false;
    if (ok) {
      showMessage(`User "${newUsername}" created`);
      newUsername = ''; newPassword = ''; newEmail = '';
      showCreateForm = false;
      loadData();
    } else {
      showMessage('Failed to create user (may already exist)');
    }
  }

  async function handleDeactivate(username: string) {
    if (!confirm(`Deactivate user "${username}"?`)) return;
    await deactivateUser(username);
    loadData();
  }

  async function handleActivate(username: string) {
    await activateUser(username);
    loadData();
  }

  async function handleToggleAdmin(username: string, currentlyAdmin: boolean) {
    const action = currentlyAdmin ? 'Remove admin from' : 'Make admin';
    if (!confirm(`${action} "${username}"?`)) return;
    await setUserRole(username, !currentlyAdmin);
    loadData();
  }

  async function handleDelete(username: string) {
    if (!confirm(`Permanently delete user "${username}"? This cannot be undone.`)) return;
    await deleteUser(username);
    loadData();
  }

  async function handleResetPassword(username: string) {
    const result = await adminResetPassword(username);
    showMessage(result.ok ? `Reset email sent to ${username}` : `Failed: ${result.message || 'no email or SMTP not configured'}`, 5000);
  }

  async function handleSetPassword(username: string) {
    setPasswordUser = username;
    setPasswordValue = '';
  }

  async function confirmSetPassword() {
    if (!setPasswordValue.trim() || setPasswordValue.length < 4) {
      showMessage('Password must be at least 4 characters');
      return;
    }
    const ok = await adminSetPassword(setPasswordUser, setPasswordValue);
    showMessage(ok ? `Password set for "${setPasswordUser}"` : 'Failed to set password');
    setPasswordUser = '';
    setPasswordValue = '';
  }

  async function handleForceUpdate() {
    feedUpdating = true;
    await forceUpdateFeeds();
    showMessage('Feed update started in background', 5000);
    feedUpdating = false;
  }
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Admin Panel</h1>

  {#if message}
    <div class="bg-brand-dim text-brand px-4 py-2 rounded-lg text-sm text-center">{message}</div>
  {/if}

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else}
    <!-- Server Status + Stats -->
    <div class="bg-surface border border-border rounded-xl p-5">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">Server</h2>
        {#if health}
          <div class="flex items-center gap-4 text-sm">
            <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-success"></span> Online</span>
            <span>v{health.version}</span>
            <span class="capitalize">{health.database}</span>
          </div>
        {/if}
      </div>

      {#if adminStats}
        <div class="grid grid-cols-5 gap-3 mb-4">
          <div class="bg-bg border border-border rounded-lg p-3 text-center">
            <div class="text-xl font-bold text-brand">{adminStats.users}</div>
            <div class="text-xs text-text-dim">Users</div>
          </div>
          <div class="bg-bg border border-border rounded-lg p-3 text-center">
            <div class="text-xl font-bold text-brand">{adminStats.devices}</div>
            <div class="text-xs text-text-dim">Devices</div>
          </div>
          <div class="bg-bg border border-border rounded-lg p-3 text-center">
            <div class="text-xl font-bold text-brand">{adminStats.subscriptions}</div>
            <div class="text-xs text-text-dim">Subscriptions</div>
          </div>
          <div class="bg-bg border border-border rounded-lg p-3 text-center">
            <div class="text-xl font-bold text-brand">{adminStats.podcasts}</div>
            <div class="text-xs text-text-dim">Podcasts</div>
          </div>
          <div class="bg-bg border border-border rounded-lg p-3 text-center">
            <div class="text-xl font-bold text-brand">{adminStats.episode_actions}</div>
            <div class="text-xs text-text-dim">Episode Actions</div>
          </div>
        </div>
      {/if}

      <button
        onclick={handleForceUpdate}
        disabled={feedUpdating}
        class="px-4 py-1.5 bg-brand text-bg text-xs font-medium rounded-lg hover:opacity-90 disabled:opacity-50 cursor-pointer"
      >
        {feedUpdating ? 'Updating...' : 'Force Feed Update'}
      </button>
    </div>

    <!-- Set Password Modal -->
    {#if setPasswordUser}
      <div class="bg-surface border border-border rounded-xl p-5">
        <h3 class="text-sm font-semibold mb-3">Set password for "{setPasswordUser}"</h3>
        <div class="flex gap-2">
          <input bind:value={setPasswordValue} type="password" placeholder="New password (min 4 chars)"
            class="flex-1 px-3 py-2 bg-bg border border-border rounded text-text text-sm" />
          <button onclick={confirmSetPassword} class="px-4 py-2 bg-brand text-bg text-xs rounded-md cursor-pointer">Set</button>
          <button onclick={() => { setPasswordUser = ''; }} class="px-4 py-2 bg-surface border border-border text-text-dim text-xs rounded-md cursor-pointer">Cancel</button>
        </div>
      </div>
    {/if}

    <!-- Users -->
    <div class="bg-surface border border-border rounded-xl p-5">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold">Users ({users.length})</h2>
        <button
          onclick={() => { showCreateForm = !showCreateForm; }}
          class="px-3 py-1 bg-brand text-bg text-xs font-medium rounded-md hover:opacity-90 cursor-pointer"
        >
          + Create User
        </button>
      </div>

      {#if showCreateForm}
        <div class="bg-bg border border-border rounded-lg p-4 mb-4 space-y-3">
          <input bind:value={newUsername} placeholder="Username" class="w-full px-3 py-2 bg-surface border border-border rounded text-text text-sm" />
          <input bind:value={newPassword} type="password" placeholder="Password" class="w-full px-3 py-2 bg-surface border border-border rounded text-text text-sm" />
          <input bind:value={newEmail} type="email" placeholder="Email (optional)" class="w-full px-3 py-2 bg-surface border border-border rounded text-text text-sm" />
          <div class="flex gap-2">
            <button onclick={handleCreate} disabled={creating} class="px-4 py-1.5 bg-brand text-bg text-xs rounded-md cursor-pointer disabled:opacity-50">{creating ? 'Creating...' : 'Create'}</button>
            <button onclick={() => { showCreateForm = false; }} class="px-4 py-1.5 bg-surface border border-border text-text-dim text-xs rounded-md cursor-pointer">Cancel</button>
          </div>
        </div>
      {/if}

      <div class="divide-y divide-border">
        {#each users as user}
          <div class="flex items-center justify-between py-3">
            <div>
              <span class="font-medium text-sm">{user.username}</span>
              {#if user.email}<span class="text-xs text-text-dim ml-2">{user.email}</span>{/if}
              <span class="ml-2 text-xs px-1.5 py-0.5 rounded-full {user.active ? 'bg-green-900/30 text-success' : 'bg-red-900/30 text-danger'}">{user.active ? 'active' : 'inactive'}</span>
              {#if user.is_admin}
                <span class="ml-1 text-xs px-1.5 py-0.5 rounded-full bg-brand-dim text-brand">admin</span>
              {/if}
            </div>
            <div class="flex items-center gap-3 text-xs text-text-dim">
              <span>{user.devices} device{user.devices !== 1 ? 's' : ''}</span>
              <span>{user.subscriptions} sub{user.subscriptions !== 1 ? 's' : ''}</span>
              {#if user.username !== auth.username}
                <button onclick={() => handleToggleAdmin(user.username, user.is_admin)} class="text-brand hover:text-blue-400 cursor-pointer">
                  {user.is_admin ? 'Remove admin' : 'Make admin'}
                </button>
                <button onclick={() => handleSetPassword(user.username)} class="text-brand hover:text-blue-400 cursor-pointer">Set password</button>
                {#if user.email}
                  <button onclick={() => handleResetPassword(user.username)} class="text-brand hover:text-blue-400 cursor-pointer">Reset password</button>
                {/if}
                {#if user.active}
                  <button onclick={() => handleDeactivate(user.username)} class="text-warning hover:text-yellow-400 cursor-pointer">Deactivate</button>
                {:else}
                  <button onclick={() => handleActivate(user.username)} class="text-success hover:text-green-400 cursor-pointer">Activate</button>
                {/if}
                <button onclick={() => handleDelete(user.username)} class="text-danger hover:text-red-400 cursor-pointer">Delete</button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
