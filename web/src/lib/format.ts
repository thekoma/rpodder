// Display helpers shared by the pages. Pure functions, no Svelte state.

/** "1h 05m" / "42 min" for an episode length in seconds. */
export function formatDuration(secs: number | null | undefined): string {
  if (!secs || secs <= 0) return '';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${String(m).padStart(2, '0')}m`;
  return `${Math.max(1, m)} min`;
}

/** "1:02:03" / "4:05" clock time for positions. */
export function formatClock(secs: number | null | undefined): string {
  const s = Math.max(0, Math.floor(secs || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** "12 Mar 2026" in the browser's locale. */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

/** "14:05" in the browser's locale. */
export function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

/** Day heading for grouped lists: "Today", "Yesterday", or the date. */
export function dayLabel(iso: string, now: Date = new Date()): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(now) - day(d)) / 86_400_000);
  if (diff === 0) return 'Today';
  if (diff === 1) return 'Yesterday';
  return d.toLocaleDateString(undefined, {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric',
  });
}

/** Minutes left, rounded up, for "12 min left". */
export function minutesLeft(position: number, total: number): number {
  return Math.max(0, Math.ceil((total - position) / 60));
}

/** A readable name for a feed whose title is not known yet. */
export function feedFallbackTitle(url: string): string {
  try {
    const u = new URL(url);
    return u.host.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/** A readable name for an episode whose title is not known yet. */
export function episodeFallbackTitle(url: string): string {
  try {
    const seg = new URL(url).pathname.split('/').filter(Boolean).pop();
    return seg ? decodeURIComponent(seg) : url;
  } catch {
    return url;
  }
}

/** Whether the text typed in a search box is a feed address rather than a query. */
export function looksLikeFeedUrl(text: string): boolean {
  const t = text.trim();
  if (!/^https?:\/\//i.test(t) || /\s/.test(t)) return false;
  try {
    return !!new URL(t).host;
  } catch {
    return false;
  }
}

/** The letter a title is filed under in the catalog ("#" for digits and symbols). */
export function fileLetter(title: string): string {
  const c = title
    .trim()
    .replace(/^(the|a|an)\s+/i, '')
    .normalize('NFD')
    .charAt(0)
    .toUpperCase();
  return /[A-Z]/.test(c) ? c : '#';
}

/** Sort key matching fileLetter: leading articles ignored, case-insensitive. */
export function fileKey(title: string): string {
  return title.trim().replace(/^(the|a|an)\s+/i, '').toLocaleLowerCase();
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Feed descriptions often carry HTML; show only their text. DOMParser never runs scripts. */
export function plainText(html: string | null | undefined): string {
  if (!html) return '';
  if (!/[<&]/.test(html)) return html.trim();
  if (typeof DOMParser === 'undefined') return html.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
  const doc = new DOMParser().parseFromString(html.replace(/<br\s*\/?>/gi, ' '), 'text/html');
  return (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim();
}

/**
 * Catalog order: "#" first, then by filing letter, then by title. Sorting by
 * the letter first keeps every letter's titles contiguous, even for titles
 * `fileLetter` files under "#" (digits, non-Latin scripts, Ø, Ł) that a plain
 * locale sort would scatter.
 */
export function compareCatalog(a: string, b: string): number {
  const la = fileLetter(a);
  const lb = fileLetter(b);
  if (la !== lb) {
    if (la === '#') return -1;
    if (lb === '#') return 1;
    return la < lb ? -1 : 1;
  }
  return fileKey(a).localeCompare(fileKey(b));
}

/** Items in catalog order, grouped under their filing letter (each letter once). */
export function groupByLetter<T>(items: T[], title: (item: T) => string): { letter: string; rows: T[] }[] {
  const sorted = [...items].sort((x, y) => compareCatalog(title(x), title(y)));
  const out: { letter: string; rows: T[] }[] = [];
  for (const item of sorted) {
    const letter = fileLetter(title(item));
    const last = out[out.length - 1];
    if (last && last.letter === letter) last.rows.push(item);
    else out.push({ letter, rows: [item] });
  }
  return out;
}

/** First occurrence of each URL (normalized loosely: case-insensitive, no trailing slash). */
export function uniqueByUrl<T extends { url: string }>(items: T[]): T[] {
  const seen = new Set<string>();
  return items.filter(i => {
    const k = i.url.trim().replace(/\/+$/, '').toLowerCase();
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}
