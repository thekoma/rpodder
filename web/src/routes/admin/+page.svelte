<script lang="ts">
  import { RefreshCw, UserPlus, X, KeyRound, Mail, ShieldCheck, ShieldOff, UserX, UserCheck, Trash2 } from '@lucide/svelte';
  import { auth } from '$lib/auth.svelte';
  import {
    getAdminUsers, getAdminStats, createAdminUser, deactivateUser, activateUser,
    setUserRole, deleteUser, forceUpdateFeeds, getBuildInfo,
    adminResetPassword, adminSetPassword,
    type AdminUser, type AdminStats, type BuildInfo
  } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { notify, notifyError } from '$lib/toast.svelte';

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
    }).catch(() => {}).finally(() => { loading = false; });
  }

  async function handleCreate(e: Event) {
    e.preventDefault();
    if (!newUsername.trim() || !newPassword.trim()) return;
    creating = true;
    const ok = await createAdminUser(newUsername.trim(), newPassword, newEmail.trim() || undefined);
    creating = false;
    if (ok) {
      notify(`Created ${newUsername.trim()}`);
      newUsername = ''; newPassword = ''; newEmail = '';
      showCreateForm = false;
      loadData();
    } else {
      notifyError("Couldn't create the user. The name may already be taken.");
    }
  }

  async function run(ok: Promise<boolean>, done: string, failed: string) {
    try {
      if (await ok) notify(done); else notifyError(failed);
    } catch {
      notifyError(failed);
    } finally {
      loadData();
    }
  }

  async function handleDeactivate(username: string) {
    if (!confirm(`Deactivate ${username}? They are signed out everywhere and can't log in until reactivated. Their data stays.`)) return;
    run(deactivateUser(username), `${username} deactivated`, `Couldn't deactivate ${username}.`);
  }

  function handleActivate(username: string) {
    run(activateUser(username), `${username} activated`, `Couldn't activate ${username}.`);
  }

  async function handleToggleAdmin(username: string, currentlyAdmin: boolean) {
    if (!confirm(currentlyAdmin ? `Remove administrator rights from ${username}?` : `Make ${username} an administrator?`)) return;
    run(setUserRole(username, !currentlyAdmin), currentlyAdmin ? `${username} is now a member` : `${username} is now an administrator`, `Couldn't change the role of ${username}.`);
  }

  async function handleDelete(username: string) {
    if (!confirm(`Delete ${username} and all their subscriptions, devices and history? This can't be undone.`)) return;
    run(deleteUser(username), `${username} deleted`, `Couldn't delete ${username}.`);
  }

  async function handleResetPassword(username: string) {
    const result = await adminResetPassword(username);
    if (result.ok && result.message !== 'SMTP not configured') notify(`Reset link sent to ${username}`);
    else notifyError(`Couldn't send a reset link: ${result.message || 'no email address or SMTP not configured'}.`);
  }

  async function confirmSetPassword(e: Event) {
    e.preventDefault();
    if (setPasswordValue.length < 4) {
      notifyError('The password needs at least 4 characters.');
      return;
    }
    const ok = await adminSetPassword(setPasswordUser, setPasswordValue);
    if (ok) notify(`Password set for ${setPasswordUser}. Their sessions were signed out.`);
    else notifyError("Couldn't set the password.");
    setPasswordUser = '';
    setPasswordValue = '';
  }

  async function handleForceUpdate() {
    feedUpdating = true;
    const ok = await forceUpdateFeeds();
    feedUpdating = false;
    if (ok) notify('Refreshing every feed in the background');
    else notifyError("Couldn't start the feed refresh.");
  }

  let stats = $derived(adminStats ? [
    { label: 'Users', value: adminStats.users },
    { label: 'Devices', value: adminStats.devices },
    { label: 'Subscriptions', value: adminStats.subscriptions },
    { label: 'Podcasts', value: adminStats.podcasts },
    { label: 'Episode actions', value: adminStats.episode_actions },
  ] : []);
</script>

<svelte:head><title>Admin · rpodder</title></svelte:head>

<div class="space-y-10">
  <header class="flex flex-wrap items-end justify-between gap-4">
    <div>
      <h1 class="text-[1.75rem] font-bold tracking-[-0.02em] leading-tight">Admin</h1>
      {#if health}
        <p class="text-sm text-ink-2 mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
          <span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-success"></span> Running</span>
          <span class="data">v{health.version}</span>
          <span class="capitalize">{health.database}</span>
        </p>
      {/if}
    </div>
    <button onclick={handleForceUpdate} disabled={feedUpdating} class="btn btn-secondary">
      <RefreshCw class="w-4 h-4 {feedUpdating ? 'animate-spin' : ''}" /> Refresh all feeds
    </button>
  </header>

  {#if loading}
    <div class="skeleton h-16 w-full"></div>
  {:else}
    {#if stats.length > 0}
      <dl aria-label="Server totals" class="-mt-6 flex flex-wrap gap-x-5 gap-y-1 text-sm">
        {#each stats as s (s.label)}
          <div class="flex items-baseline gap-1.5">
            <dt class="order-2 text-ink-2">{s.label.toLowerCase()}</dt>
            <dd class="order-1 data font-semibold text-brass">{s.value.toLocaleString()}</dd>
          </div>
        {/each}
      </dl>
    {/if}

    <section aria-labelledby="users-heading" class="space-y-3">
      <div class="flex items-center justify-between gap-3">
        <h2 id="users-heading" class="text-base font-bold">Users <span class="text-brass font-semibold data text-sm ml-1">{users.length}</span></h2>
        <button onclick={() => { showCreateForm = !showCreateForm; }} class="btn {showCreateForm ? 'btn-quiet' : 'btn-primary'} btn-sm" aria-expanded={showCreateForm}>
          {#if showCreateForm}<X class="w-4 h-4" /> Cancel{:else}<UserPlus class="w-4 h-4" /> New user{/if}
        </button>
      </div>

      {#if showCreateForm}
        <form onsubmit={handleCreate} class="catalog-card p-4 sm:p-5 grid gap-4 sm:grid-cols-3 items-end">
          <div><label for="nu-name" class="label">Username</label><input id="nu-name" bind:value={newUsername} required autocomplete="off" class="field" /></div>
          <div><label for="nu-pass" class="label">Password</label><input id="nu-pass" bind:value={newPassword} type="password" required autocomplete="new-password" class="field" /></div>
          <div><label for="nu-mail" class="label">Email <span class="font-normal text-ink-3">(optional)</span></label><input id="nu-mail" bind:value={newEmail} type="email" class="field" /></div>
          <div class="sm:col-span-3"><button type="submit" disabled={creating} class="btn btn-primary">{creating ? 'Creating…' : 'Create user'}</button></div>
        </form>
      {/if}

      {#if setPasswordUser}
        <form onsubmit={confirmSetPassword} class="catalog-card p-4 sm:p-5 space-y-3">
          <label for="sp-pass" class="label">New password for {setPasswordUser}</label>
          <div class="flex gap-2 max-w-md">
            <!-- svelte-ignore a11y_autofocus -->
            <input id="sp-pass" bind:value={setPasswordValue} type="password" minlength="4" autocomplete="new-password" autofocus class="field flex-1" />
            <button type="submit" class="btn btn-primary">Set</button>
            <button type="button" onclick={() => { setPasswordUser = ''; }} class="btn btn-quiet">Cancel</button>
          </div>
          <p class="hint">Signs {setPasswordUser} out of every browser and app that uses a session.</p>
        </form>
      {/if}

      <ul class="panel divide-y divide-rule">
        {#each users as user (user.username)}
          {@const self = user.username === auth.username}
          <li class="flex flex-col lg:flex-row lg:items-center gap-3 px-4 py-3.5">
            <div class="min-w-0 flex-1">
              <p class="flex flex-wrap items-center gap-2">
                <span class="font-semibold">{user.username}</span>
                {#if self}<span class="text-xs text-ink-3">(you)</span>{/if}
                {#if user.is_admin}<span class="tag bg-brass-soft text-brass">Admin</span>{/if}
                {#if !user.active}<span class="stamp text-danger">Inactive</span>{/if}
              </p>
              <p class="text-xs text-ink-3 mt-0.5 flex flex-wrap gap-x-3">
                {#if user.email}<span>{user.email}</span>{/if}
                <span class="data">{user.devices} dev · {user.subscriptions} subs</span>
              </p>
            </div>
            {#if !self}
              <div class="flex flex-wrap items-center gap-1 -ml-2 lg:ml-0">
                <button onclick={() => handleToggleAdmin(user.username, user.is_admin)} class="btn btn-quiet btn-sm">
                  {#if user.is_admin}<ShieldOff class="w-4 h-4" /> Remove admin{:else}<ShieldCheck class="w-4 h-4" /> Make admin{/if}
                </button>
                <button onclick={() => { setPasswordUser = user.username; setPasswordValue = ''; }} class="btn btn-quiet btn-sm">
                  <KeyRound class="w-4 h-4" /> Set password
                </button>
                {#if user.email}
                  <button onclick={() => handleResetPassword(user.username)} class="btn btn-quiet btn-sm">
                    <Mail class="w-4 h-4" /> Send reset link
                  </button>
                {/if}
                {#if user.active}
                  <button onclick={() => handleDeactivate(user.username)} class="btn btn-quiet btn-sm hover:!text-warning">
                    <UserX class="w-4 h-4" /> Deactivate
                  </button>
                {:else}
                  <button onclick={() => handleActivate(user.username)} class="btn btn-quiet btn-sm hover:!text-success">
                    <UserCheck class="w-4 h-4" /> Activate
                  </button>
                {/if}
                <button onclick={() => handleDelete(user.username)} class="btn btn-quiet btn-sm text-danger hover:!bg-danger-soft">
                  <Trash2 class="w-4 h-4" /> Delete
                </button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
