<script lang="ts">
  import { confirmPasswordReset, requestPasswordReset } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';

  let token = $state('');
  let email = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let success = $state('');
  let loading = $state(false);
  let loaded = $state(false);

  // Check if we have a token in the URL (reset confirm) or not (request reset)
  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    const params = new URLSearchParams(window.location.search);
    token = params.get('token') || '';
  });

  async function handleRequestReset(e: Event) {
    e.preventDefault();
    error = '';
    if (!email.trim()) { error = 'Email is required'; return; }
    loading = true;
    await requestPasswordReset(email);
    loading = false;
    success = 'If an account with that email exists, a reset link has been sent.';
  }

  async function handleConfirmReset(e: Event) {
    e.preventDefault();
    error = '';
    if (newPassword.length < 4) { error = 'Password must be at least 4 characters'; return; }
    if (newPassword !== confirmPassword) { error = 'Passwords do not match'; return; }
    loading = true;
    const ok = await confirmPasswordReset(token, newPassword);
    loading = false;
    if (ok) {
      success = 'Password has been reset. You can now log in.';
      setTimeout(() => goto('/login'), 2000);
    } else {
      error = 'Invalid or expired reset link.';
    }
  }
</script>

<div class="max-w-sm mx-auto mt-20">
  <div class="bg-surface border border-border rounded-xl p-8">
    <div class="text-center mb-8">
      <h1 class="text-2xl font-bold text-brand">{token ? 'Set New Password' : 'Reset Password'}</h1>
      <p class="text-text-dim text-sm mt-1">
        {token ? 'Enter your new password below.' : 'Enter your email to receive a reset link.'}
      </p>
    </div>

    {#if success}
      <p class="text-success text-sm text-center mb-4">{success}</p>
    {:else if token}
      <!-- Confirm reset form -->
      <form onsubmit={handleConfirmReset} class="space-y-4">
        <div>
          <label for="password" class="block text-sm text-text-dim mb-1">New Password</label>
          <input id="password" type="password" bind:value={newPassword} required autocomplete="new-password"
            class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
        </div>
        <div>
          <label for="confirm" class="block text-sm text-text-dim mb-1">Confirm Password</label>
          <input id="confirm" type="password" bind:value={confirmPassword} required autocomplete="new-password"
            class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
        </div>
        {#if error}<p class="text-danger text-sm text-center">{error}</p>{/if}
        <button type="submit" disabled={loading}
          class="w-full py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer">
          {loading ? 'Resetting...' : 'Reset Password'}
        </button>
      </form>
    {:else}
      <!-- Request reset form -->
      <form onsubmit={handleRequestReset} class="space-y-4">
        <div>
          <label for="email" class="block text-sm text-text-dim mb-1">Email</label>
          <input id="email" type="email" bind:value={email} required
            class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-text focus:outline-none focus:border-brand transition-colors" />
        </div>
        {#if error}<p class="text-danger text-sm text-center">{error}</p>{/if}
        <button type="submit" disabled={loading}
          class="w-full py-2.5 bg-brand text-bg font-medium rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50 cursor-pointer">
          {loading ? 'Sending...' : 'Send Reset Link'}
        </button>
      </form>
    {/if}

    <p class="text-center text-text-dim text-sm mt-4">
      <a href="/login" class="text-brand hover:underline">Back to login</a>
    </p>
  </div>
</div>
