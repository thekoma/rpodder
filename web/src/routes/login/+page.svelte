<script lang="ts">
  import { auth } from '$lib/auth.svelte';
  import { login as apiLogin } from '$lib/api';
  import { goto } from '$app/navigation';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleLogin(e: Event) {
    e.preventDefault();
    error = '';
    loading = true;

    const result = await apiLogin(username, password);

    if (result.ok) {
      auth.login(username);
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
  </div>
</div>
