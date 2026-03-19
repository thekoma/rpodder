// Auth state using Svelte 5 runes
import { browser } from '$app/environment';

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
    _username = null;
    _isAdmin = false;
    if (browser) {
      localStorage.removeItem('rpodder_user');
      localStorage.removeItem('rpodder_admin');
    }
  }
};
