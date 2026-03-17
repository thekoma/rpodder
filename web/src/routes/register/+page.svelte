<script lang="ts">
  import { createAdminUser } from '$lib/api';
  import { goto } from '$app/navigation';

  let username = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let email = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleRegister(e: Event) {
    e.preventDefault();
    error = '';

    if (password !== confirmPassword) {
      error = 'Passwords do not match';
      return;
    }
    if (username.length < 2) {
      error = 'Username must be at least 2 characters';
      return;
    }
    if (password.length < 4) {
      error = 'Password must be at least 4 characters';
      return;
    }

    loading = true;
    const ok = await createAdminUser(username, password, email || undefined);
    loading = false;

    if (ok) {
      goto('/login');
    } else {
      error = 'Username already exists or registration failed';
    }
  }
</script>

<div class="max-w-sm mx-auto mt-20">
  <div class="bg-surface border border-border rounded-xl p-8">
    <div class="text-center mb-8">
      <span class="text-4xl">🎧</span>
      <h1 class="text-2xl font-bold mt-2 text-brand">Create Account</h1>
      <p class="text-text-dim text-sm mt-1">Register to sync your podcasts</p>
    </div>

    <form onsubmit={handleRegister} class="space-y-4">
      <div>
        <label for="username" class="block text-sm text-text-dim mb-1">Username</label>
        <input id="username" type="text" bind:value={username} required autocomplete="username"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
      </div>

      <div>
        <label for="email" class="block text-sm text-text-dim mb-1">Email (optional)</label>
        <input id="email" type="email" bind:value={email}
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
      </div>

      <div>
        <label for="password" class="block text-sm text-text-dim mb-1">Password</label>
        <input id="password" type="password" bind:value={password} required autocomplete="new-password"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
      </div>

      <div>
        <label for="confirm" class="block text-sm text-text-dim mb-1">Confirm Password</label>
        <input id="confirm" type="password" bind:value={confirmPassword} required autocomplete="new-password"
          class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
      </div>

      {#if error}
        <p class="text-danger text-sm text-center">{error}</p>
      {/if}

      <button type="submit" disabled={loading}
        class="w-full py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer"
      >
        {loading ? 'Creating...' : 'Create Account'}
      </button>
    </form>

    <p class="text-center text-text-dim text-sm mt-4">
      Already have an account? <a href="/login" class="text-brand hover:underline">Sign in</a>
    </p>
  </div>
</div>
