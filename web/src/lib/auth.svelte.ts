// Auth state using Svelte 5 runes

let _username = $state<string | null>(
  typeof localStorage !== 'undefined' ? localStorage.getItem('rpodder_user') : null
);
let _loggedIn = $state<boolean>(!!_username);

export const auth = {
  get username() { return _username; },
  get loggedIn() { return _loggedIn; },

  login(username: string) {
    _username = username;
    _loggedIn = true;
    localStorage.setItem('rpodder_user', username);
  },

  logout() {
    _username = null;
    _loggedIn = false;
    localStorage.removeItem('rpodder_user');
  }
};
