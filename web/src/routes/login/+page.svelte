<script lang="ts">
  import { KeyRound, Clock } from '@lucide/svelte';
  import { auth } from '$lib/auth.svelte';
  import { login as apiLogin, getSsoInfo, getMe, type SsoInfo } from '$lib/api';
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import { page } from '$app/stores';
  import AuthFrame from '$lib/components/AuthFrame.svelte';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let ssoInfo = $state<SsoInfo | null>(null);
  let expired = $derived($page.url.searchParams.get('expired') === '1');

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
      goto('/subscriptions');
    } else {
      error = "That username and password don't match an account.";
    }
    loading = false;
  }
</script>

<svelte:head><title>Sign in · rpodder</title></svelte:head>

<AuthFrame title="Sign in" subtitle="Manage the podcasts your apps sync.">
  {#if expired}
    <p class="notice mb-5 bg-warning-soft border-warning/40"><Clock class="w-4 h-4 text-warning shrink-0 mt-0.5" /> Your session expired. Sign in again to continue.</p>
  {/if}

  {#if ssoInfo?.enabled}
    <a href="/auth/sso/login" rel="external" class="btn btn-secondary w-full h-11">
      <KeyRound class="w-4 h-4" /> Continue with {ssoInfo.provider_name}
    </a>
    <div class="flex items-center gap-3 my-5" aria-hidden="true">
      <span class="flex-1 h-px bg-rule"></span><span class="text-xs text-ink-3">or with a password</span><span class="flex-1 h-px bg-rule"></span>
    </div>
  {/if}

  <form onsubmit={handleLogin} class="space-y-4">
    <div>
      <label for="username" class="label">Username</label>
      <input id="username" type="text" bind:value={username} required autocomplete="username" autocapitalize="none" spellcheck="false" class="field" />
    </div>
    <div>
      <div class="flex items-baseline justify-between">
        <label for="password" class="label">Password</label>
        <a href="/reset-password" class="text-xs font-semibold text-primary hover:underline">Forgot it?</a>
      </div>
      <input id="password" type="password" bind:value={password} required autocomplete="current-password" class="field" />
    </div>

    {#if error}<p class="text-sm text-danger" role="alert">{error}</p>{/if}

    <button type="submit" disabled={loading} class="btn btn-primary w-full h-11">
      {loading ? 'Signing in…' : 'Sign in'}
    </button>
  </form>

  {#snippet footer()}
    {#if ssoInfo && ssoInfo.registration !== 'closed'}
      <p>New here? <a href="/register" class="font-semibold text-primary hover:underline">Create an account</a></p>
    {/if}
  {/snippet}
</AuthFrame>
