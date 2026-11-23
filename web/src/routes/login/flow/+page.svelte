<script lang="ts">
  import { untrack } from 'svelte';
  import { ShieldAlert, CircleCheck, CircleX } from '@lucide/svelte';
  import { auth, sessionExpired } from '#lib/auth.svelte.js';
  import { answerLoginFlow, logout as apiLogout, lookupLoginFlow, type FlowInfo } from '#lib/api.js';
  import { rememberNext, takeNext } from '#lib/navigation.js';
  import { formatDate, formatTime } from '#lib/format.js';
  import { goto } from '$app/navigation';
  import { browser } from '$app/env';
  import AuthFrame from '#lib/components/AuthFrame.svelte';

  type View = 'loading' | 'incomplete' | 'gone' | 'forbidden' | 'error' | 'pending' | 'granted' | 'denied' | 'taken';

  let view = $state<View>('loading');
  let flow = $state<FlowInfo | null>(null);
  let token = $state('');
  let busy = $state(false);

  // The login token is in the fragment, which browsers don't send to the server.
  $effect(() => {
    if (!browser) return;
    untrack(() => load());
  });

  async function load() {
    token = location.hash.slice(1);
    if (!token) { view = 'incomplete'; return; }
    // Set before anything can redirect to /login, so the login comes back here.
    rememberNext('/login/flow#' + token);
    if (!auth.loggedIn) { goto('/login', { replaceState: true }); return; }

    const result = await lookupLoginFlow(token);
    if (result === 'unauthenticated') { sessionExpired(auth.username); return; }
    if (result === 'forbidden') { view = 'forbidden'; return; }
    takeNext(location.origin);
    if (result === 'gone' || result === 'error') { view = result; return; }
    flow = result;
    view = result.approved ? 'granted' : 'pending';
  }

  async function respond(approve: boolean) {
    busy = true;
    const result = await answerLoginFlow(token, approve);
    busy = false;
    if (result === 'unauthenticated') { rememberNext('/login/flow#' + token); sessionExpired(auth.username); return; }
    if (result === 'ok') view = approve ? 'granted' : 'denied';
    else view = result;
  }

  async function signOut() {
    const username = auth.username;
    if (username) await apiLogout(username);
    auth.logout();
    goto('/login');
  }
</script>

<svelte:head><title>Grant access · rpodder</title></svelte:head>

<AuthFrame title="Grant access" subtitle="An app is asking to sync with your account.">
  {#if view === 'loading'}
    <p class="text-ink-2">Loading…</p>
  {:else if view === 'incomplete'}
    <p>This link is incomplete. Start the login again in your app.</p>
  {:else if view === 'gone'}
    <p>This link has expired or was already used. Start the login again in your app.</p>
  {:else if view === 'error'}
    <p>Something went wrong. Try again.</p>
  {:else if view === 'forbidden'}
    <p class="mb-5">You're signed in with an app password. Sign out and sign in with your account password or SSO to grant access.</p>
    <button type="button" onclick={signOut} class="btn btn-secondary w-full h-11">Sign out</button>
  {:else if view === 'granted'}
    <p class="flex items-start gap-2"><CircleCheck class="w-5 h-5 text-success shrink-0" /> Access granted. You can go back to the app.</p>
  {:else if view === 'denied'}
    <p class="flex items-start gap-2"><CircleX class="w-5 h-5 text-danger shrink-0" /> Access denied.</p>
  {:else if view === 'taken'}
    <p>This request was already answered.</p>
  {:else if view === 'pending' && flow}
    <div class="space-y-4">
      <p>An app wants to sync with your rpodder account <strong>{flow.username}</strong>.</p>
      <p class="text-ink-2">
        It calls itself <strong class="text-ink">{flow.client_name}</strong> (reported by the app),
        requested {formatDate(flow.created_at)} at {formatTime(flow.created_at)}.
      </p>
      <p class="notice bg-warning-soft border-warning/40 text-sm">
        <ShieldAlert class="w-4 h-4 text-warning shrink-0 mt-0.5" />
        Only continue if you started signing in to an app just now. If you didn't, someone may be trying to get access to your account. Deny it.
      </p>
      <div class="flex gap-3">
        <button type="button" disabled={busy} onclick={() => respond(false)} class="btn btn-secondary flex-1 h-11">Deny</button>
        <button type="button" disabled={busy} onclick={() => respond(true)} class="btn btn-primary flex-1 h-11">Grant access</button>
      </div>
    </div>
  {/if}
</AuthFrame>
