// Auth state using Svelte 5 runes
import { browser } from '$app/environment';

let _username = $state<string | null>(
  browser ? localStorage.getItem('rpodder_user') : null
);

export const auth = {
  get username() { return _username; },
  get loggedIn() { return !!_username; },

  login(username: string) {
    _username = username;
    if (browser) localStorage.setItem('rpodder_user', username);
  },

  logout() {
    _username = null;
    if (browser) localStorage.removeItem('rpodder_user');
  }
};
