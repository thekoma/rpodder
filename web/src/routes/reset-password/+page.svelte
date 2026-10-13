<script lang="ts">
  import { MailCheck, CircleCheck } from '@lucide/svelte';
  import { confirmPasswordReset, requestPasswordReset } from '#lib/api.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import AuthFrame from '#lib/components/AuthFrame.svelte';

  let token = $state('');
  let email = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let success = $state('');
  let loading = $state(false);
  let loaded = $state(false);

  // A token in the URL means the visitor came from the reset email.
  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    const params = new URLSearchParams(window.location.search);
    token = params.get('token') || '';
  });

  async function handleRequestReset(e: Event) {
    e.preventDefault();
    error = '';
    if (!email.trim()) { error = 'Enter the email address of your account.'; return; }
    loading = true;
    await requestPasswordReset(email.trim());
    loading = false;
    success = 'If an account uses that address, a reset link is on its way. It works for 24 hours.';
  }

  async function handleConfirmReset(e: Event) {
    e.preventDefault();
    error = '';
    if (newPassword.length < 4) { error = 'The new password needs at least 4 characters.'; return; }
    if (newPassword !== confirmPassword) { error = "The two passwords don't match."; return; }
    loading = true;
    const ok = await confirmPasswordReset(token, newPassword);
    loading = false;
    if (ok) {
      success = 'Password changed. Taking you to sign in…';
      setTimeout(() => goto('/login'), 2000);
    } else {
      error = 'This reset link is invalid or has expired. Ask for a new one.';
    }
  }
</script>

<svelte:head><title>Reset password · rpodder</title></svelte:head>

<AuthFrame title={token ? 'Choose a new password' : 'Reset your password'}
  subtitle={token ? 'Every browser and app signed in with a session will need to sign in again.' : "We'll email you a link to set a new one."}>
  {#if success}
    <p class="notice">
      {#if token}<CircleCheck class="w-4 h-4 text-success shrink-0 mt-0.5" />{:else}<MailCheck class="w-4 h-4 text-success shrink-0 mt-0.5" />{/if}
      {success}
    </p>
  {:else if token}
    <form onsubmit={handleConfirmReset} class="space-y-4">
      <div>
        <label for="password" class="label">New password</label>
        <input id="password" type="password" bind:value={newPassword} required minlength="4" autocomplete="new-password" class="field" />
      </div>
      <div>
        <label for="confirm" class="label">Repeat new password</label>
        <input id="confirm" type="password" bind:value={confirmPassword} required minlength="4" autocomplete="new-password" class="field" />
      </div>
      {#if error}<p class="text-sm text-danger" role="alert">{error}</p>{/if}
      <button type="submit" disabled={loading} class="btn btn-primary w-full h-11">
        {loading ? 'Saving…' : 'Set new password'}
      </button>
    </form>
  {:else}
    <form onsubmit={handleRequestReset} class="space-y-4">
      <div>
        <label for="email" class="label">Email</label>
        <input id="email" type="email" bind:value={email} required autocomplete="email" class="field" />
      </div>
      {#if error}<p class="text-sm text-danger" role="alert">{error}</p>{/if}
      <button type="submit" disabled={loading} class="btn btn-primary w-full h-11">
        {loading ? 'Sending…' : 'Send reset link'}
      </button>
    </form>
  {/if}

  {#snippet footer()}
    <p><a href="/login" class="font-semibold text-primary hover:underline">Back to sign in</a></p>
  {/snippet}
</AuthFrame>
