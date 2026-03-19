<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { changeMyPassword, getMe, type MeInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let me = $state<MeInfo | null>(null);
  let oldPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let success = $state('');
  let loading = $state(false);
  let loaded = $state(false);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    if (!auth.username) { goto('/login'); return; }
    getMe().then(m => { me = m; });
  });

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
