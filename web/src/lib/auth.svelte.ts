// Auth state using Svelte 5 runes
import { browser } from '$app/env';
import { goto } from '$app/navigation';
import { ApiError } from './api';
import { sameUser } from './playerLogic';

const sessionEndListeners: (() => void)[] = [];

/** Register a callback run whenever the session ends (logout or expiry). */
export function onSessionEnd(cb: () => void): void {
  sessionEndListeners.push(cb);
}

function fireSessionEnd() {
  for (const cb of sessionEndListeners) {
    try { cb(); } catch (e) { console.warn('session-end listener failed', e); }
  }
}

let _username = $state<string | null>(
  browser ? localStorage.getItem('rpodder_user') : null
);

let _isAdmin = $state<boolean>(
  browser ? localStorage.getItem('rpodder_admin') === 'true' : false
);

export const auth = {
  get username() { return _username; },
  get loggedIn() { return !!_username; },
  get isAdmin() { return _isAdmin; },

  login(username: string, isAdmin: boolean = false) {
    // Logging in as someone else without logging out first ends the old session.
    if (_username !== null && _username.toLowerCase() !== username.toLowerCase()) fireSessionEnd();
    _username = username;
    _isAdmin = isAdmin;
    if (browser) {
      localStorage.setItem('rpodder_user', username);
      localStorage.setItem('rpodder_admin', String(isAdmin));
    }
  },

  setAdmin(isAdmin: boolean) {
    _isAdmin = isAdmin;
    if (browser) localStorage.setItem('rpodder_admin', String(isAdmin));
  },

  logout() {
    const mine = _username;
    fireSessionEnd();
    _username = null;
    _isAdmin = false;
    // Only clear storage that is still ours: another tab may have logged in as someone else.
    if (browser && sameUser(localStorage.getItem('rpodder_user'), mine)) {
      localStorage.removeItem('rpodder_user');
      localStorage.removeItem('rpodder_admin');
    }
  }
};

// Another tab logged in as a different user (sharing our cookie) or logged out.
if (browser) {
  window.addEventListener('storage', (e: StorageEvent) => {
    if (e.storageArea !== localStorage) return;
    if (e.key === 'rpodder_admin' && sameUser(localStorage.getItem('rpodder_user'), _username)) {
      _isAdmin = e.newValue === 'true';
      return;
    }
    if (e.key !== 'rpodder_user' && e.key !== null) return; // null key: storage cleared
    const stored = localStorage.getItem('rpodder_user');
    if (sameUser(stored, _username)) return;
    fireSessionEnd();
    _username = stored;
    _isAdmin = stored !== null && localStorage.getItem('rpodder_admin') === 'true';
    // Pages that loaded once still hold the previous user's data: start over.
    location.reload();
  });
}

/**
 * The server rejected our session (401/403 on the user's own resources). Pass the username the
 * failing request was for: a late failure for a user this tab no longer has is ignored.
 */
export function sessionExpired(forUser?: string | null): void {
  if (forUser !== undefined && !sameUser(auth.username, forUser)) return;
  auth.logout();
  goto('/login?expired=1');
}

export function isAuthError(e: unknown): boolean {
  return e instanceof ApiError && (e.status === 401 || e.status === 403);
}
