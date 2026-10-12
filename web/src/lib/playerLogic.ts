// Pure player logic (no imports, erasable TypeScript only so `node --test` can run it).

// Mirror of rpodder_core::url::normalize_url.
export function normalizeUrl(raw: string): string {
  const trimmed = raw.trim();
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return trimmed;
  }
  // Setting an empty hash drops the fragment; a no-op when there is none.
  if (url.hash === '') url.hash = '';
  const path = url.pathname;
  if (path.length > 1 && path.endsWith('/')) {
    url.pathname = path.replace(/\/+$/, '');
  }
  return url.toString();
}

export function isSafeMediaUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  try {
    const { protocol } = new URL(url);
    return protocol === 'http:' || protocol === 'https:';
  } catch {
    return false;
  }
}

export function isPlayable(item: { url?: string | null; media?: boolean | null; mimetype?: string | null }): boolean {
  if (!isSafeMediaUrl(item.url)) return false;
  if (item.media === false) return false;
  if (item.mimetype && /^(text\/|image\/)/.test(item.mimetype)) return false;
  return true;
}

export function chooseSource(pageProtocol: string, url: string): { src: string; upgraded: boolean } {
  if (pageProtocol === 'https:') {
    try {
      const u = new URL(url);
      if (u.protocol === 'http:') {
        u.protocol = 'https:';
        return { src: u.toString(), upgraded: true };
      }
    } catch {
      // not a URL: leave untouched
    }
  }
  return { src: url, upgraded: false };
}

export interface KnownPosition {
  position: number;
  total: number | null;
  timestamp: string;
  device: string | null;
}

// Newest timestamp wins; on a tie the local entry is kept.
export function pickResume(
  remote: KnownPosition | null | undefined,
  local: KnownPosition | null | undefined,
): KnownPosition | null {
  if (!remote) return local ?? null;
  if (!local) return remote;
  return remote.timestamp > local.timestamp ? remote : local;
}

export function resumeAt(pos: KnownPosition | null, duration: number | null): number {
  if (!pos) return 0;
  const total = pos.total ?? duration;
  if (total !== null && total > 0 && pos.position >= total - 30) return 0;
  return Math.max(0, pos.position);
}

export function evict<T extends { timestamp: string }>(entries: Record<string, T>, cap: number): Record<string, T> {
  const keys = Object.keys(entries);
  if (keys.length <= cap) return entries;
  keys.sort((a, b) => {
    const ta = entries[a].timestamp;
    const tb = entries[b].timestamp;
    return ta < tb ? 1 : ta > tb ? -1 : 0;
  });
  const out: Record<string, T> = {};
  for (const k of keys.slice(0, Math.max(0, cap))) out[k] = entries[k];
  return out;
}

export function isoNow(d?: Date): string {
  return (d ?? new Date()).toISOString().slice(0, 19);
}

// Timestamp for a local save: unchanged when the position is, so re-saving a
// paused episode (e.g. on pagehide) never makes a stale position look newer
// than one another device recorded since.
export function localTimestamp(prev: { position: number; timestamp: string } | null | undefined, position: number, now: string): string {
  return prev && prev.position === position ? prev.timestamp : now;
}

export function effectiveTotal(audioDuration: number, feedDuration: number | null | undefined): number | null {
  if (Number.isFinite(audioDuration) && audioDuration > 0) return Math.floor(audioDuration);
  if (feedDuration != null && feedDuration > 0) return feedDuration;
  return null;
}

export interface UploadGuard {
  sync: boolean;
  remoteKnown: boolean;
  position: number;
  total: number | null;
  lastUploaded: number | null;
}

export function shouldUpload(g: UploadGuard): boolean {
  return g.sync && g.remoteKnown && g.total !== null && g.position >= 1 && g.position !== g.lastUploaded;
}

export interface PlayActionInput {
  podcastUrl: string;
  episodeUrl: string;
  device: string;
  started: number;
  position: number;
  total: number;
  now: Date;
}

export function buildPlayAction(i: PlayActionInput): {
  podcast: string;
  episode: string;
  device: string;
  action: 'play';
  started: number;
  position: number;
  total: number;
  timestamp: string;
} {
  return {
    podcast: i.podcastUrl,
    episode: i.episodeUrl,
    device: i.device,
    action: 'play',
    started: Math.min(i.started, i.position),
    position: i.position,
    total: i.total,
    timestamp: isoNow(i.now),
  };
}

// Usernames are case-insensitive on the server.
export function sameUser(a: string | null | undefined, b: string | null | undefined): boolean {
  if (a == null || b == null) return a == null && b == null;
  return a.toLowerCase() === b.toLowerCase();
}

// A remote read that arrived after playback already resumed from `resumedFrom` (at `startedAt`)
// may enable uploads only if it is not newer; otherwise uploading would overwrite another
// device's newer position. A remote entry resolving to the same start point is harmless.
export function remoteAccepted(
  remote: KnownPosition | null | undefined,
  resumedFrom: KnownPosition | null,
  startedAt: number,
  duration: number | null,
): boolean {
  if (!remote) return true;
  if (pickResume(remote, resumedFrom) !== remote) return true;
  return resumeAt(remote, duration) === Math.floor(startedAt);
}
