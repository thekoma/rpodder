// Listening positions: localStorage copy plus the server's latest plays.
import { getPositions, type PlayItem } from './api';
import { isAuthError, sessionExpired } from './auth.svelte';
import { evict, normalizeUrl, pickResume, type KnownPosition } from './playerLogic';

export interface LocalEntry extends KnownPosition {
  podcastUrl: string;
  podcastTitle: string;
  episodeTitle: string;
  imageUrl: string | null;
}

const LOCAL_CAP = 200;

function storageKey(username: string): string {
  return `rpodder_positions_${username.toLowerCase()}`;
}

function readAll(username: string): Record<string, LocalEntry> {
  try {
    const raw = localStorage.getItem(storageKey(username));
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

export function localGet(username: string, episodeUrl: string): LocalEntry | null {
  return readAll(username)[normalizeUrl(episodeUrl)] ?? null;
}

export function localSet(username: string, episodeUrl: string, entry: LocalEntry): void {
  try {
    const all = readAll(username);
    all[normalizeUrl(episodeUrl)] = entry;
    localStorage.setItem(storageKey(username), JSON.stringify(evict(all, LOCAL_CAP)));
  } catch {
    // storage unavailable or full: local positions are best effort
  }
}

export function localClear(username: string): void {
  try {
    localStorage.removeItem(storageKey(username));
  } catch {
    // ignore
  }
}

export function localInProgress(username: string, limit: number): PlayItem[] {
  return Object.entries(readAll(username))
    .filter(([, e]) => e.position > 0 && (e.total == null || e.position < e.total - 30))
    .sort(([, a], [, b]) => (a.timestamp < b.timestamp ? 1 : a.timestamp > b.timestamp ? -1 : 0))
    .slice(0, limit)
    .map(([episodeUrl, e]) => ({
      podcast_url: e.podcastUrl,
      podcast_title: e.podcastTitle || null,
      podcast_logo_url: e.imageUrl,
      episode_url: episodeUrl,
      episode_title: e.episodeTitle || null,
      mimetype: null,
      position: e.position,
      total: e.total,
      timestamp: e.timestamp,
      device: e.device,
    }));
}

export type PositionsResult = { positions: Map<string, KnownPosition>; remoteKnown: boolean };

export async function podcastPositions(username: string, podcastUrl: string, sync: boolean): Promise<PositionsResult> {
  const podcastKey = normalizeUrl(podcastUrl);
  const locals = new Map<string, KnownPosition>();
  for (const [key, e] of Object.entries(readAll(username))) {
    if (normalizeUrl(e.podcastUrl) === podcastKey) locals.set(key, e);
  }
  if (!sync) return { positions: locals, remoteKnown: false };

  let remote: PlayItem[];
  try {
    remote = await getPositions(podcastUrl);
  } catch (e) {
    if (isAuthError(e)) {
      sessionExpired(username);
      throw e;
    }
    return { positions: locals, remoteKnown: false };
  }
  const merged = new Map(locals);
  for (const item of remote) {
    const key = normalizeUrl(item.episode_url);
    const best = pickResume(item, locals.get(key));
    if (best) merged.set(key, best);
  }
  return { positions: merged, remoteKnown: true };
}
