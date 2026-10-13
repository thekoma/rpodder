// The position shown for an episode: live from the player when it is the
// loaded episode, otherwise the newest known (synced or this browser's).
// Reads player state, so call it inside $derived to stay reactive.
import { player } from './player.svelte';
import { normalizeUrl, pickResume, type KnownPosition } from './playerLogic';

export interface ShownProgress {
  position: number;
  total: number;
  loaded: boolean;
}

export function shownProgress(url: string, duration: number | null | undefined, progress: KnownPosition | null | undefined): ShownProgress {
  const key = normalizeUrl(url);
  const loaded = !!player.episode && normalizeUrl(player.episode.url) === key;
  if (loaded) {
    return { position: player.position, total: player.duration || duration || 0, loaded };
  }
  const known = pickResume(progress ?? null, player.recent[key]);
  // A total of 0 means the client didn't know it: fall back to the feed's duration.
  return { position: known?.position ?? 0, total: known?.total || duration || 0, loaded };
}
