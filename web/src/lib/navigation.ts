// Where to go after a login. The target lives in sessionStorage, never in a URL,
// because the login-flow page's target carries a token in its fragment.

const NEXT_KEY = 'rpodder_next';
const NEXT_TTL_MS = 15 * 60 * 1000;

interface KeyValueStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function sessionStore(): KeyValueStorage | null {
  try {
    return typeof sessionStorage === 'undefined' ? null : sessionStorage;
  } catch {
    return null;
  }
}

function pathIsLocal(path: string): boolean {
  return path.startsWith('/') && !path.startsWith('//') && !/[\\\u0000-\u001f\u007f]/.test(path);
}

/** `value` if it is a path on `origin`, else null. Checked as given and percent-decoded. */
export function safeNext(value: string | null, origin: string): string | null {
  if (!value) return null;
  let decoded: string;
  try {
    decoded = decodeURIComponent(value);
  } catch {
    return null;
  }
  if (!pathIsLocal(value) || !pathIsLocal(decoded)) return null;
  try {
    if (new URL(value, origin).origin !== origin) return null;
  } catch {
    return null;
  }
  return value;
}

/** Remember where to return after the next login. */
export function rememberNext(next: string, store: KeyValueStorage | null = sessionStore(), now = Date.now()): void {
  try {
    store?.setItem(NEXT_KEY, JSON.stringify({ next, at: now }));
  } catch {
    // With storage full or blocked the login lands on the default page.
  }
}

/** Read and forget the return target. Null when missing, stale or not a local path. */
export function takeNext(origin: string, store: KeyValueStorage | null = sessionStore(), now = Date.now()): string | null {
  let raw: string | null = null;
  try {
    raw = store?.getItem(NEXT_KEY) ?? null;
    store?.removeItem(NEXT_KEY);
  } catch {
    return null;
  }
  if (!raw) return null;
  try {
    const { next, at } = JSON.parse(raw);
    if (typeof next !== 'string' || typeof at !== 'number' || now - at > NEXT_TTL_MS || at > now) return null;
    return safeNext(next, origin);
  } catch {
    return null;
  }
}
