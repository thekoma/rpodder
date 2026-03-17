<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { getAdminUsers, createAdminUser, deactivateUser, forceUpdateFeeds, getHealth, type AdminUser, type HealthInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let users = $state<AdminUser[]>([]);
  let health = $state<HealthInfo | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let showCreateForm = $state(false);
  let newUsername = $state('');
  let newPassword = $state('');
  let newEmail = $state('');
  let creating = $state(false);
  let feedUpdating = $state(false);
  let message = $state('');

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username) { goto('/login'); return; }
    loadData();
  });

  function loadData() {
    Promise.all([getAdminUsers(), getHealth()]).then(([u, h]) => {
      users = u;
      health = h;
      loading = false;
    }).catch(() => { loading = false; });
  }

  async function handleCreate() {
    if (!newUsername.trim() || !newPassword.trim()) return;
    creating = true;
    const ok = await createAdminUser(newUsername, newPassword, newEmail || undefined);
    creating = false;
    if (ok) {
      message = `User "${newUsername}" created`;
      newUsername = ''; newPassword = ''; newEmail = '';
      showCreateForm = false;
      loadData();
    } else {
      message = 'Failed to create user (may already exist)';
    }
    setTimeout(() => { message = ''; }, 3000);
  }

  async function handleDeactivate(username: string) {
    if (!confirm(`Deactivate user "${username}"?`)) return;
    await deactivateUser(username);
    loadData();
  }

  async function handleForceUpdate() {
    feedUpdating = true;
    await forceUpdateFeeds();
    message = 'Feed update started in background';
    feedUpdating = false;
    setTimeout(() => { message = ''; }, 5000);
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
    <!-- Server Status -->
    {#if health}
      <div class="bg-surface border border-border rounded-xl p-5">
        <h2 class="text-lg font-semibold mb-3">Server</h2>
        <div class="flex items-center gap-6 text-sm">
          <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-success"></span> Online</span>
          <span>v{health.version}</span>
          <span class="capitalize">{health.database}</span>
          <button
            onclick={handleForceUpdate}
            disabled={feedUpdating}
            class="ml-auto px-4 py-1.5 bg-brand text-bg text-xs font-medium rounded-lg hover:opacity-90 disabled:opacity-50 cursor-pointer"
          >
            {feedUpdating ? 'Updating...' : 'Force Feed Update'}
          </button>
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
            </div>
            <div class="flex items-center gap-4 text-xs text-text-dim">
              <span>{user.devices} device{user.devices !== 1 ? 's' : ''}</span>
              <span>{user.subscriptions} sub{user.subscriptions !== 1 ? 's' : ''}</span>
              {#if user.active}
                <button onclick={() => handleDeactivate(user.username)} class="text-danger hover:text-red-400 cursor-pointer">Deactivate</button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
