// Persistent web audio player with gpodder position sync.
import { browser } from '$app/env';
import { getPositions, uploadEpisodeActions, type PlayItem } from './api';
import { auth, isAuthError, onSessionEnd, sessionExpired } from './auth.svelte';
import { localGet, localSet, podcastPositions } from './positions';
import {
  buildPlayAction,
  chooseSource,
  effectiveTotal,
  isoNow,
  localTimestamp,
  isSafeMediaUrl,
  normalizeUrl,
  pickResume,
  remoteAccepted,
  resumeAt,
  sameUser,
  shouldUpload,
  type KnownPosition,
} from './playerLogic';
import { WEB_DEVICE_ID, clearWebSettingsCache, ensureWebDevice, getWebSettingsCached } from './webDevice';

export interface PlayerEpisode {
  url: string;
  title: string;
  podcastUrl: string;
  podcastTitle: string;
  imageUrl?: string | null;
  duration?: number | null;
}

export const REPORT_INTERVAL_MS = 300_000;
export const LOCAL_SAVE_MS = 5_000;
export const SEEK_SETTLE_MS = 3_000;
export const REMOTE_STALE_MS = 120_000;
// Longest muted wait for the settings/positions reads; the reads themselves are not aborted.
export const POSITIONING_TIMEOUT_MS = 3_000;

function timer(ms: number): { done: Promise<'timeout'>; cancel: () => void } {
  let id: ReturnType<typeof setTimeout> | undefined;
  const done = new Promise<'timeout'>(resolve => { id = setTimeout(() => resolve('timeout'), ms); });
  return { done, cancel: () => clearTimeout(id) };
}

type Lookup = { sync: boolean; positions: Map<string, KnownPosition>; remoteKnown: boolean } | 'stale';
type RemoteRead = { item: PlayItem | null } | 'failed' | 'stale';

const TAB_ID = Math.random().toString(36).slice(2) + Date.now().toString(36);

function reportInterval(): number {
  try {
    const v = Number(localStorage.getItem('rpodder_player_report_interval_ms'));
    if (Number.isFinite(v) && v > 0) return Math.max(5000, v);
  } catch {
    // ignore
  }
  return REPORT_INTERVAL_MS;
}

type PlayerError = 'secure-version-failed' | 'load' | null;

const VOLUME_KEY = 'rpodder_player_volume';

// The listener's volume, kept in this browser.
function savedVolume(): { volume: number; muted: boolean } {
  try {
    const v = JSON.parse(localStorage.getItem(VOLUME_KEY) ?? 'null');
    if (v && typeof v.volume === 'number' && v.volume >= 0 && v.volume <= 1) {
      return { volume: v.volume, muted: v.muted === true };
    }
  } catch {
    // ignore
  }
  return { volume: 1, muted: false };
}

const initialVolume = savedVolume();

class Player {
  episode = $state<PlayerEpisode | null>(null);
  playing = $state(false);
  position = $state(0);
  duration = $state(0);
  rate = $state(1);
  volume = $state(initialVolume.volume);
  muted = $state(initialVolume.muted);
  // iOS ignores element.volume (only muting works): the UI hides the slider there.
  volumeAdjustable = $state(true);
  error = $state<PlayerError>(null);
  recent = $state<Record<string, KnownPosition>>({});

  // Plain (non-reactive) state.
  #audio: HTMLAudioElement | null = null;
  #channel: BroadcastChannel | null = null;
  #username: string | null = null;
  #recentOwner: string | null = null;
  #sync = false;
  #remoteKnown = false;
  #lastRemoteRead = 0;
  #segment: { started: number } | null = null;
  #lastUploaded: number | null = null;
  #seekBurstUntil = 0;
  #burstTarget = 0;
  #gen = 0;
  #src: string | null = null;
  #upgraded = false;
  #ready = false;
  #pendingStart: number | null = null;
  // While true the element may already be playing (muted) but the resume position is not applied yet:
  // no segment, no local save, no upload.
  #positioning = false;
  // Muted while positioning; the listener's volume comes back afterwards.
  #positionMuted = false;
  #localTimer: ReturnType<typeof setInterval> | null = null;
  #reportTimer: ReturnType<typeof setInterval> | null = null;
  #seekTimer: ReturnType<typeof setTimeout> | null = null;

  #stale(gen: number): boolean {
    return gen !== this.#gen || !sameUser(auth.username, this.#username);
  }

  // Early exit of an async step. If no newer operation took over (same generation, so the user
  // changed under us) the player is reset, which also unmutes and ends positioning.
  #bail(gen: number): void {
    if (gen === this.#gen) this.reset();
  }

  #ensureAudio(): HTMLAudioElement {
    if (this.#audio) return this.#audio;
    const a = document.createElement('audio');
    a.preload = 'metadata';
    document.body.appendChild(a);
    this.#audio = a;
    a.volume = 0.5;
    this.volumeAdjustable = a.volume === 0.5;
    this.#applyVolume();

    const current = () => this.#src !== null && a.src === this.#src;

    a.addEventListener('loadedmetadata', () => {
      if (!current()) return;
      this.#ready = true;
      a.playbackRate = this.rate;
      this.#mirrorDuration();
      this.#finishPositioning();
    });
    a.addEventListener('durationchange', () => { if (current()) this.#mirrorDuration(); });
    a.addEventListener('timeupdate', () => { if (current() && this.#ready && !this.#positioning) this.position = a.currentTime; });
    a.addEventListener('play', () => { if (current()) this.playing = true; });
    a.addEventListener('playing', () => {
      if (!current()) return;
      this.playing = true;
      this.#channel?.postMessage({ type: 'playing', tab: TAB_ID });
      if (!this.#ready || this.#positioning) return;
      this.#openSegment();
      this.#startTimers();
    });
    a.addEventListener('pause', () => {
      if (!current()) return;
      this.playing = false;
      this.#stopTimers();
      if (!this.#ready || this.#positioning) return;
      this.#saveLocal();
      void this.#endSegment();
    });
    a.addEventListener('ended', () => {
      if (!current()) return;
      this.playing = false;
      this.#stopTimers();
      if (!this.#ready) return;
      const end = Math.floor(Number.isFinite(a.duration) ? a.duration : a.currentTime);
      this.#saveLocal(end);
      void this.#endSegment(end);
    });
    a.addEventListener('error', () => {
      if (!current()) return;
      void this.#endSegment();
      this.#ready = false;
      this.#positioning = false;
      this.#pendingStart = null;
      this.#restoreVolume();
      this.playing = false;
      this.#stopTimers();
      this.error = this.#upgraded ? 'secure-version-failed' : 'load';
    });

    window.addEventListener('pagehide', () => {
      if (!this.#ready) return;
      this.#saveLocal();
      void this.#endSegment();
    });
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'hidden' || !this.#ready) return;
      this.#saveLocal();
      if (this.#segment) void this.#upload(this.#segment.started, Math.floor(a.currentTime));
    });

    if (typeof BroadcastChannel !== 'undefined') {
      this.#channel = new BroadcastChannel('rpodder-player');
      this.#channel.onmessage = (ev: MessageEvent) => {
        const msg = ev.data;
        if (msg && msg.type === 'playing' && msg.tab !== TAB_ID && !a.paused) a.pause();
      };
    }

    if (typeof navigator !== 'undefined' && 'mediaSession' in navigator) {
      const ms = navigator.mediaSession;
      const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
        ['play', () => { if (a.paused) this.toggle(); }],
        ['pause', () => { if (!a.paused) a.pause(); }],
        ['seekbackward', d => this.skip(-(d.seekOffset ?? 15))],
        ['seekforward', d => this.skip(d.seekOffset ?? 30)],
        ['seekto', d => { if (d.seekTime != null) this.seek(d.seekTime); }],
      ];
      for (const [action, handler] of handlers) {
        try { ms.setActionHandler(action, handler); } catch { /* unsupported action */ }
      }
    }
    return a;
  }

  #mirrorDuration() {
    const a = this.#audio;
    if (!a) return;
    this.duration = Number.isFinite(a.duration) && a.duration > 0 ? a.duration : (this.episode?.duration ?? 0);
  }

  #mute() {
    const a = this.#audio;
    if (!a) return;
    this.#positionMuted = true;
    // volume is read-only on iOS, muted works everywhere
    a.volume = 0;
    a.muted = true;
  }

  #restoreVolume() {
    const a = this.#audio;
    if (!a || !this.#positionMuted) return;
    this.#positionMuted = false;
    this.#applyVolume();
  }

  // Apply the listener's volume, unless positioning holds the element muted.
  #applyVolume() {
    const a = this.#audio;
    if (!a || this.#positionMuted) return;
    a.volume = this.volume;
    a.muted = this.muted;
  }

  #saveVolume() {
    try {
      localStorage.setItem(VOLUME_KEY, JSON.stringify({ volume: this.volume, muted: this.muted }));
    } catch {
      // ignore
    }
  }

  setVolume(v: number): void {
    if (!Number.isFinite(v)) return;
    this.volume = Math.min(1, Math.max(0, v));
    // Moving the slider up un-mutes, like a hardware volume knob.
    if (this.volume > 0) this.muted = false;
    this.#saveVolume();
    this.#applyVolume();
  }

  toggleMute(): void {
    // Follow what the UI shows: a volume of 0 counts as muted.
    if (this.muted || this.volume === 0) {
      this.muted = false;
      if (this.volume === 0) this.volume = 0.5;
    } else {
      this.muted = true;
    }
    this.#saveVolume();
    this.#applyVolume();
  }

  #openSegment() {
    const a = this.#audio;
    if (a && !this.#segment && Date.now() >= this.#seekBurstUntil) {
      this.#segment = { started: Math.floor(a.currentTime) };
    }
  }

  // Apply the resume position once it is known and metadata is loaded, then unmute.
  #finishPositioning() {
    const a = this.#audio;
    if (!a || !this.#positioning || this.#pendingStart === null || !this.#ready) return;
    const start = this.#pendingStart;
    this.#pendingStart = null;
    if (Math.abs(a.currentTime - start) > 0.5) a.currentTime = start;
    this.position = start;
    this.#positioning = false;
    this.#restoreVolume();
    if (!a.paused) {
      this.#openSegment();
      this.#startTimers();
    }
  }

  #startTimers() {
    if (!this.#localTimer) this.#localTimer = setInterval(() => this.#saveLocal(), LOCAL_SAVE_MS);
    if (!this.#reportTimer) {
      this.#reportTimer = setInterval(() => {
        const a = this.#audio;
        if (a && this.#segment && this.#ready) void this.#upload(this.#segment.started, Math.floor(a.currentTime));
      }, reportInterval());
    }
  }

  #stopTimers() {
    if (this.#localTimer) { clearInterval(this.#localTimer); this.#localTimer = null; }
    if (this.#reportTimer) { clearInterval(this.#reportTimer); this.#reportTimer = null; }
  }

  #saveLocal(position?: number) {
    const a = this.#audio;
    const ep = this.episode;
    const u = this.#username;
    if (!a || !ep || !u || !this.#ready || this.#positioning) return;
    const pos = Math.floor(position ?? a.currentTime);
    const prev = this.recent[normalizeUrl(ep.url)] ?? localGet(u, ep.url);
    const known: KnownPosition = {
      position: pos,
      total: effectiveTotal(a.duration, ep.duration),
      timestamp: localTimestamp(prev, pos, isoNow()),
      device: WEB_DEVICE_ID,
    };
    localSet(u, ep.url, {
      ...known,
      podcastUrl: ep.podcastUrl,
      podcastTitle: ep.podcastTitle,
      episodeTitle: ep.title,
      imageUrl: ep.imageUrl ?? null,
    });
    this.recent[normalizeUrl(ep.url)] = known;
  }

  #endSegment(position?: number): Promise<void> {
    const seg = this.#segment;
    const a = this.#audio;
    if (!seg || !a) return Promise.resolve();
    this.#segment = null;
    return this.#upload(seg.started, Math.floor(position ?? a.currentTime));
  }

  // Everything is captured synchronously; only the network part is async.
  #upload(started: number, position: number): Promise<void> {
    const a = this.#audio;
    const ep = this.episode;
    const u = this.#username;
    if (!a || !ep || !u) return Promise.resolve();
    const total = effectiveTotal(a.duration, ep.duration);
    if (!shouldUpload({ sync: this.#sync, remoteKnown: this.#remoteKnown, position, total, lastUploaded: this.#lastUploaded })) {
      return Promise.resolve();
    }
    this.#lastUploaded = position;
    const action = buildPlayAction({
      podcastUrl: ep.podcastUrl,
      episodeUrl: ep.url,
      device: WEB_DEVICE_ID,
      started,
      position,
      total: total as number,
      now: new Date(),
    });
    // The keepalive upload goes first so it survives tab close; the server auto-creates the
    // device on upload and ensureWebDevice() afterwards fixes its caption.
    return (async () => {
      try {
        await uploadEpisodeActions(u, [action], { keepalive: true });
      } catch (e) {
        if (this.#lastUploaded === position) this.#lastUploaded = null;
        if (isAuthError(e)) {
          sessionExpired(u);
          return;
        }
        console.warn('Position upload failed', e);
        return;
      }
      ensureWebDevice(u).catch(e => console.warn('Web device setup failed', e));
    })();
  }

  #beginSeekBurst(target: number) {
    const a = this.#audio;
    if (a && this.#segment) void this.#endSegment(Math.floor(a.currentTime));
    this.#burstTarget = target;
    this.#seekBurstUntil = Date.now() + SEEK_SETTLE_MS;
    if (this.#seekTimer) clearTimeout(this.#seekTimer);
    this.#seekTimer = setTimeout(() => {
      this.#seekTimer = null;
      const el = this.#audio;
      if (el && this.#ready && !el.paused && !this.#segment) {
        this.#segment = { started: Math.floor(Math.min(this.#burstTarget, el.currentTime)) };
      }
    }, SEEK_SETTLE_MS);
  }

  #setMediaMetadata(ep: PlayerEpisode | null) {
    if (typeof navigator === 'undefined' || !('mediaSession' in navigator)) return;
    try {
      navigator.mediaSession.metadata = ep && typeof MediaMetadata !== 'undefined'
        ? new MediaMetadata({
          title: ep.title,
          artist: ep.podcastTitle,
          artwork: ep.imageUrl && isSafeMediaUrl(ep.imageUrl) ? [{ src: ep.imageUrl }] : [],
        })
        : null;
    } catch {
      // ignore
    }
  }

  // Read the remote position of the loaded episode. Leaves #remoteKnown to the caller.
  async #readRemote(gen: number): Promise<RemoteRead> {
    const ep = this.episode;
    const u = this.#username;
    if (!ep || !u) return 'failed';
    try {
      const items = await getPositions(ep.podcastUrl);
      if (this.#stale(gen)) return 'stale';
      const key = normalizeUrl(ep.url);
      return { item: items.find(i => normalizeUrl(i.episode_url) === key) ?? null };
    } catch (e) {
      if (isAuthError(e)) {
        sessionExpired(u);
        return 'stale';
      }
      return this.#stale(gen) ? 'stale' : 'failed';
    }
  }

  #localKnown(ep: PlayerEpisode): KnownPosition | null {
    const u = this.#username;
    return pickResume(this.recent[normalizeUrl(ep.url)], u ? localGet(u, ep.url) : null);
  }

  #mediaDuration(ep: PlayerEpisode): number | null {
    const a = this.#audio;
    return a && Number.isFinite(a.duration) && a.duration > 0 ? a.duration : (ep.duration ?? null);
  }

  // Settings, then positions. Sets #sync as soon as the settings are known.
  async #lookup(gen: number, username: string, ep: PlayerEpisode): Promise<Lookup> {
    let sync = false;
    try {
      sync = (await getWebSettingsCached(username)).syncPositions;
    } catch (e) {
      if (isAuthError(e)) { sessionExpired(username); return 'stale'; }
    }
    if (this.#stale(gen)) return 'stale';
    this.#sync = sync;
    try {
      return { sync, ...(await podcastPositions(username, ep.podcastUrl, sync)) };
    } catch {
      return 'stale'; // auth errors only; podcastPositions already called sessionExpired()
    }
  }

  async play(username: string, ep: PlayerEpisode): Promise<void> {
    if (!browser) return;
    if (!isSafeMediaUrl(ep.url)) { this.error = 'load'; return; }
    if (!sameUser(username, this.#username)) {
      this.reset();
      this.#username = username;
    }
    if (!sameUser(username, this.#recentOwner)) {
      this.recent = {};
      this.#recentOwner = username;
    }
    // Same episode: toggle, unless its load failed (then retry).
    if (this.episode && !this.error && normalizeUrl(ep.url) === normalizeUrl(this.episode.url)) {
      this.toggle();
      return;
    }
    const gen = ++this.#gen;
    const a = this.#ensureAudio();

    if (this.episode && this.#ready && !this.#positioning) {
      this.#saveLocal();
      void this.#endSegment();
    }
    this.#segment = null;
    this.#stopTimers();
    if (this.#seekTimer) { clearTimeout(this.#seekTimer); this.#seekTimer = null; }
    this.#seekBurstUntil = 0;
    this.#ready = false;
    this.#pendingStart = null;

    this.episode = ep;
    this.error = null;
    this.playing = false;
    this.position = 0;
    this.duration = ep.duration ?? 0;
    this.#lastUploaded = null;
    this.#remoteKnown = false;
    this.#setMediaMetadata(ep);

    // Start playback synchronously, inside the user gesture (WebKit), muted until the
    // resume position is known and applied.
    const { src, upgraded } = chooseSource(location.protocol, ep.url);
    this.#upgraded = upgraded;
    this.#positioning = true;
    this.#mute();
    a.src = src;
    this.#src = a.src;
    a.playbackRate = this.rate;
    a.play().catch(() => { /* autoplay blocked or superseded: stay paused */ });

    // The reads may be slow (cold server): wait at most POSITIONING_TIMEOUT_MS muted, then resume
    // from the local position with uploads blocked, and let the reads finish in the background.
    this.#sync = false;
    const lookup = this.#lookup(gen, username, ep);
    const t = timer(POSITIONING_TIMEOUT_MS);
    const first = await Promise.race([lookup, t.done]);
    t.cancel();
    if (first === 'stale' || this.#stale(gen)) { this.#bail(gen); return; }

    const key = normalizeUrl(ep.url);
    if (first !== 'timeout') {
      this.#remoteKnown = first.remoteKnown;
      if (first.remoteKnown) this.#lastRemoteRead = Date.now();
      this.#startAt(resumeAt(pickResume(first.positions.get(key), this.recent[key]), ep.duration ?? null));
      return;
    }

    const resumedFrom = this.#localKnown(ep);
    const start = resumeAt(resumedFrom, ep.duration ?? null);
    this.#startAt(start);
    const late = await lookup;
    if (late === 'stale' || this.#stale(gen) || !late.remoteKnown) return;
    if (remoteAccepted(late.positions.get(key), resumedFrom, start, this.#mediaDuration(ep))) {
      this.#remoteKnown = true;
      this.#lastRemoteRead = Date.now();
    }
    // else: the remote is newer than where we resumed; uploads stay blocked and the next resume re-reads.
  }

  #startAt(start: number) {
    this.#pendingStart = start;
    this.position = start;
    this.#finishPositioning();
  }

  toggle(): void {
    const a = this.#audio;
    if (!a || !this.episode) return;
    if (!a.paused) { a.pause(); return; }
    if (this.#positioning) {
      // still resolving the start position: just (re)start, muted until it is applied
      a.play().catch(() => { /* autoplay blocked */ });
      return;
    }
    if (!this.#ready) return;
    const gen = ++this.#gen;
    const needsRemote = this.#sync && (Date.now() - this.#lastRemoteRead > REMOTE_STALE_MS || !this.#remoteKnown);
    const pausedAt = a.currentTime;
    if (needsRemote) {
      this.#positioning = true;
      this.#mute();
    }
    // play() inside the user gesture (WebKit); the remote check runs in the background.
    a.play().catch(() => { /* autoplay blocked */ });
    if (needsRemote) void this.#resumeFromRemote(gen, pausedAt);
  }

  async #resumeFromRemote(gen: number, pausedAt: number): Promise<void> {
    const ep = this.episode;
    if (!ep) return;
    const resumedFrom = this.#localKnown(ep);
    const read = this.#readRemote(gen);
    const t = timer(POSITIONING_TIMEOUT_MS);
    const first = await Promise.race([read, t.done]);
    t.cancel();
    if (first === 'stale' || this.#stale(gen)) { this.#bail(gen); return; }

    if (first !== 'timeout') {
      let target = pausedAt;
      if (first !== 'failed') {
        this.#remoteKnown = true;
        this.#lastRemoteRead = Date.now();
        if (first.item && pickResume(first.item, resumedFrom) === first.item) {
          target = resumeAt(first.item, this.#mediaDuration(ep));
        }
      } else {
        this.#remoteKnown = false;
      }
      this.#startAt(target);
      return;
    }

    this.#remoteKnown = false;
    this.#startAt(pausedAt);
    const late = await read;
    if (late === 'stale' || late === 'failed' || this.#stale(gen)) return;
    if (remoteAccepted(late.item, resumedFrom, pausedAt, this.#mediaDuration(ep))) {
      this.#remoteKnown = true;
      this.#lastRemoteRead = Date.now();
    }
  }

  seek(seconds: number): void {
    const a = this.#audio;
    if (!a || !this.#ready || this.#positioning) return;
    const max = Number.isFinite(a.duration) ? a.duration : Infinity;
    const target = Math.max(0, Math.min(seconds, max));
    this.#beginSeekBurst(target);
    a.currentTime = target;
    this.position = target;
    this.#saveLocal(target);
  }

  skip(delta: number): void {
    const a = this.#audio;
    if (!a) return;
    this.seek(a.currentTime + delta);
  }

  setRate(r: number): void {
    this.rate = r;
    const a = this.#audio;
    if (a) {
      a.defaultPlaybackRate = r;
      a.playbackRate = r;
    }
  }

  async setSync(username: string, on: boolean): Promise<void> {
    clearWebSettingsCache();
    if (!sameUser(username, this.#username)) return;
    this.#sync = on;
    this.#remoteKnown = false;
    // Only bump the generation when re-reading; bumping it during a load would abandon that play().
    const a = this.#audio;
    if (!on || !a || !this.episode || !this.#ready || this.#positioning) return;
    const gen = ++this.#gen;
    const ep = this.episode;
    const read = await this.#readRemote(gen);
    if (read === 'stale' || read === 'failed' || this.#stale(gen) || this.#positioning) return;
    const newer = !!read.item && pickResume(read.item, this.#localKnown(ep)) === read.item;
    if (newer && a.paused) {
      const target = resumeAt(read.item, this.#mediaDuration(ep));
      a.currentTime = target;
      this.position = target;
    } else if (newer) {
      return; // playing and another device is ahead: no uploads until the next resume re-reads
    }
    this.#remoteKnown = true;
    this.#lastRemoteRead = Date.now();
  }

  async close(opts?: { timeoutMs?: number }): Promise<void> {
    const timeoutMs = opts?.timeoutMs ?? 3000;
    let pending: Promise<void> = Promise.resolve();
    if (this.episode && this.#ready && !this.#positioning) {
      this.#saveLocal();
      pending = this.#endSegment();
    }
    this.reset();
    const t = timer(timeoutMs);
    await Promise.race([pending, t.done]);
    t.cancel();
  }

  reset(): void {
    this.#gen++;
    this.#stopTimers();
    if (this.#seekTimer) { clearTimeout(this.#seekTimer); this.#seekTimer = null; }
    this.#ready = false;
    this.#src = null;
    this.#pendingStart = null;
    this.#positioning = false;
    this.#segment = null;
    this.#restoreVolume();
    const a = this.#audio;
    if (a) {
      a.pause();
      a.removeAttribute('src');
      a.load();
    }
    this.episode = null;
    this.playing = false;
    this.position = 0;
    this.duration = 0;
    this.error = null;
    this.#username = null;
    this.#sync = false;
    this.#remoteKnown = false;
    this.#lastRemoteRead = 0;
    this.#lastUploaded = null;
    this.#seekBurstUntil = 0;
    clearWebSettingsCache();
    this.#setMediaMetadata(null);
  }

  clearRecent(): void {
    this.recent = {};
    this.#recentOwner = null;
  }
}

export const player = new Player();

onSessionEnd(() => {
  player.reset();
  player.clearRecent();
});
