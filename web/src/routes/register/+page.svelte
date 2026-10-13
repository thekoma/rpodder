<script lang="ts">
  import { register } from '$lib/api';
  import { goto } from '$app/navigation';
  import AuthFrame from '$lib/components/AuthFrame.svelte';
  import { notify } from '$lib/toast.svelte';

  let username = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let email = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleRegister(e: Event) {
    e.preventDefault();
    error = '';

    if (username.trim().length < 2) { error = 'The username needs at least 2 characters.'; return; }
    if (password.length < 4) { error = 'The password needs at least 4 characters.'; return; }
    if (password !== confirmPassword) { error = "The two passwords don't match."; return; }

    loading = true;
    const ok = await register(username.trim(), password, email.trim() || undefined);
    loading = false;

    if (ok) {
      notify('Account created. Check your email if this server asks you to activate it.');
      goto('/login');
    } else {
      error = 'That username is taken, or registration is closed on this server.';
    }
  }
</script>

<svelte:head><title>Create account · rpodder</title></svelte:head>

<AuthFrame title="Create an account" subtitle="One account for all your podcast apps.">
  <form onsubmit={handleRegister} class="space-y-4">
    <div>
      <label for="username" class="label">Username</label>
      <input id="username" type="text" bind:value={username} required autocomplete="username" autocapitalize="none" spellcheck="false" class="field" />
      <p class="hint mt-1">You'll type this into your podcast apps too.</p>
    </div>
    <div>
      <label for="email" class="label">Email <span class="font-normal text-ink-3">(optional, for password resets)</span></label>
      <input id="email" type="email" bind:value={email} autocomplete="email" class="field" />
    </div>
    <div>
      <label for="password" class="label">Password</label>
      <input id="password" type="password" bind:value={password} required minlength="4" autocomplete="new-password" class="field" />
    </div>
    <div>
      <label for="confirm" class="label">Repeat password</label>
      <input id="confirm" type="password" bind:value={confirmPassword} required minlength="4" autocomplete="new-password" class="field" />
    </div>

    {#if error}<p class="text-sm text-danger" role="alert">{error}</p>{/if}

    <button type="submit" disabled={loading} class="btn btn-primary w-full h-11">
      {loading ? 'Creating…' : 'Create account'}
    </button>
  </form>

  {#snippet footer()}
    <p>Already registered? <a href="/login" class="font-semibold text-primary hover:underline">Sign in</a></p>
  {/snippet}
</AuthFrame>
