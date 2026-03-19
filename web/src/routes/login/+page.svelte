<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { login as apiLogin, getSsoInfo, getMe, type SsoInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let ssoInfo = $state<SsoInfo | null>(null);

  $effect(() => {
    if (!browser) return;
    getSsoInfo().then(info => { ssoInfo = info; });
  });

  async function handleLogin(e: Event) {
    e.preventDefault();
    error = '';
    loading = true;

    const result = await apiLogin(username, password);

    if (result.ok) {
      auth.login(username);
      const me = await getMe();
      if (me) auth.setAdmin(me.is_admin);
      goto('/');
    } else {
      error = 'Invalid username or password';
    }
    loading = false;
  }
</script>

<div class="max-w-sm mx-auto mt-20">
  <div class="bg-surface border border-border rounded-xl p-8">
    <div class="text-center mb-8">
      <span class="text-4xl">🎧</span>
      <h1 class="text-2xl font-bold mt-2 text-brand">rpodder</h1>
      <p class="text-text-dim text-sm mt-1">Sign in to sync your podcasts</p>
    </div>

    {#if ssoInfo?.enabled}
      <a
        href="/auth/sso/login"
        class="w-full flex items-center justify-center gap-2 py-2.5 bg-surface border border-border text-text font-medium rounded-lg hover:bg-surface-hover transition-colors no-underline mb-4"
      >
        🔐 Sign in with {ssoInfo.provider_name}
      </a>
      <div class="flex items-center gap-3 mb-4">
        <div class="flex-1 border-t border-border"></div>
        <span class="text-xs text-text-dim">or</span>
        <div class="flex-1 border-t border-border"></div>
      </div>
    {/if}

    <form onsubmit={handleLogin} class="space-y-4">
      <div>
        <label for="username" class="block text-sm text-text-dim mb-1">Username</label>
        <input
          id="username"
          type="text"
          bind:value={username}
          required
          autocomplete="username"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors"
        />
      </div>

      <div>
        <label for="password" class="block text-sm text-text-dim mb-1">Password</label>
        <input
          id="password"
          type="password"
          bind:value={password}
          required
          autocomplete="current-password"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors"
        />
      </div>

      {#if error}
        <p class="text-danger text-sm text-center">{error}</p>
      {/if}

      <button
        type="submit"
        disabled={loading}
        class="w-full py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer"
      >
        {loading ? 'Signing in...' : 'Sign in'}
      </button>
    </form>

    <p class="text-center text-text-dim text-sm mt-3">
      <a href="/reset-password" class="text-brand hover:underline">Forgot password?</a>
    </p>

    {#if ssoInfo?.registration !== 'closed'}
      <p class="text-center text-text-dim text-sm mt-2">
        Don't have an account? <a href="/register" class="text-brand hover:underline">Register</a>
      </p>
    {/if}
  </div>
</div>
