// Light / dark / follow-the-system, remembered per browser.
import { browser } from '$app/environment';

export type ThemeChoice = 'system' | 'light' | 'dark';

const KEY = 'rpodder_theme';

function read(): ThemeChoice {
  if (!browser) return 'system';
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

let _choice = $state<ThemeChoice>(read());

function apply(choice: ThemeChoice) {
  if (!browser) return;
  const root = document.documentElement;
  if (choice === 'system') delete root.dataset.theme;
  else root.dataset.theme = choice;
}

export const theme = {
  get choice() {
    return _choice;
  },
  set(choice: ThemeChoice) {
    _choice = choice;
    apply(choice);
    if (!browser) return;
    try {
      if (choice === 'system') localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, choice);
    } catch {
      // Private mode or blocked storage: the choice lasts for this page only.
    }
  },
};
