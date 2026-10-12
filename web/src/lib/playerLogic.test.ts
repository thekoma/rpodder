// @ts-nocheck -- run by `node --test` (native type stripping); not part of the Vite bundle or svelte-check.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  normalizeUrl,
  isSafeMediaUrl,
  isPlayable,
  chooseSource,
  pickResume,
  resumeAt,
  evict,
  isoNow,
  localTimestamp,
  effectiveTotal,
  shouldUpload,
  buildPlayAction,
  sameUser,
  remoteAccepted,
  type KnownPosition,
  type UploadGuard,
} from './playerLogic.ts';

test('normalizeUrl mirrors rpodder_core::url::normalize_url', () => {
  assert.equal(normalizeUrl('http://example.com/feed/'), 'http://example.com/feed');
  assert.equal(normalizeUrl('http://example.com/feed///'), 'http://example.com/feed');
  assert.equal(normalizeUrl('http://example.com/feed.xml'), 'http://example.com/feed.xml');
  assert.equal(normalizeUrl('HTTP://EXAMPLE.COM/Feed'), 'http://example.com/Feed');
  assert.equal(normalizeUrl('http://example.com/feed#'), 'http://example.com/feed');
  assert.equal(normalizeUrl('http://example.com/feed#frag'), 'http://example.com/feed#frag');
  assert.equal(normalizeUrl('http://example.com/feed?format=rss'), 'http://example.com/feed?format=rss');
  assert.equal(normalizeUrl('  http://example.com/feed  '), 'http://example.com/feed');
  assert.equal(normalizeUrl('http://example.com/'), 'http://example.com/');
  assert.equal(normalizeUrl('not a url'), 'not a url');
});

test('isSafeMediaUrl accepts only http(s)', () => {
  assert.equal(isSafeMediaUrl('https://example.com/a.mp3'), true);
  assert.equal(isSafeMediaUrl('http://example.com/a.mp3'), true);
  assert.equal(isSafeMediaUrl('javascript:alert(1)'), false);
  assert.equal(isSafeMediaUrl('data:audio/mpeg;base64,AAAA'), false);
  assert.equal(isSafeMediaUrl('ftp://example.com/a.mp3'), false);
  assert.equal(isSafeMediaUrl('not a url'), false);
  assert.equal(isSafeMediaUrl(''), false);
  assert.equal(isSafeMediaUrl(null), false);
  assert.equal(isSafeMediaUrl(undefined), false);
});

test('isPlayable', () => {
  const url = 'https://example.com/a.mp3';
  assert.equal(isPlayable({ url, media: true, mimetype: 'audio/mpeg' }), true);
  assert.equal(isPlayable({ url }), true, 'missing mimetype and media flag');
  assert.equal(isPlayable({ url, media: false }), false);
  assert.equal(isPlayable({ url, mimetype: 'text/html' }), false);
  assert.equal(isPlayable({ url, mimetype: 'image/jpeg' }), false);
  assert.equal(isPlayable({ url, mimetype: 'video/mp4' }), true);
  assert.equal(isPlayable({ url: undefined }), false);
  assert.equal(isPlayable({ url: 'javascript:alert(1)', media: true }), false);
});

test('chooseSource upgrades only on https pages', () => {
  assert.deepEqual(chooseSource('https:', 'http://example.com/a.mp3'), { src: 'https://example.com/a.mp3', upgraded: true });
  assert.deepEqual(chooseSource('http:', 'http://example.com/a.mp3'), { src: 'http://example.com/a.mp3', upgraded: false });
  assert.deepEqual(chooseSource('https:', 'https://example.com/a.mp3'), { src: 'https://example.com/a.mp3', upgraded: false });
  assert.deepEqual(chooseSource('https:', 'http://example.com:8080/a.mp3?x=1'), { src: 'https://example.com:8080/a.mp3?x=1', upgraded: true });
});

const pos = (position: number, timestamp: string, total: number | null = 3600, device: string | null = null): KnownPosition =>
  ({ position, total, timestamp, device });

test('pickResume newest wins, either side missing', () => {
  const older = pos(10, '2026-01-01T10:00:00', 3600, 'phone');
  const newer = pos(20, '2026-01-01T11:00:00', 3600, 'rpodder-web');
  assert.equal(pickResume(older, newer), newer);
  assert.equal(pickResume(newer, older), newer);
  assert.equal(pickResume(null, newer), newer);
  assert.equal(pickResume(older, undefined), older);
  assert.equal(pickResume(null, null), null);
  assert.equal(pickResume(undefined, undefined), null);
});

test('resumeAt restarts near the end', () => {
  assert.equal(resumeAt(null, 3600), 0);
  assert.equal(resumeAt(pos(100, 't'), null), 100);
  assert.equal(resumeAt(pos(3570, 't'), null), 0);
  assert.equal(resumeAt(pos(3569, 't'), null), 3569);
  assert.equal(resumeAt(pos(100, 't', null), 120), 0, 'falls back to the media duration');
  assert.equal(resumeAt(pos(100, 't', null), null), 100, 'no total known');
  assert.equal(resumeAt(pos(100, 't', null), 3600), 100);
});

test('evict keeps the newest entries up to cap', () => {
  const entries: Record<string, { timestamp: string }> = {
    a: { timestamp: '2026-01-01T10:00:00' },
    b: { timestamp: '2026-01-01T12:00:00' },
    c: { timestamp: '2026-01-01T11:00:00' },
  };
  assert.deepEqual(Object.keys(evict(entries, 2)).sort(), ['b', 'c']);
  assert.deepEqual(Object.keys(evict(entries, 5)).sort(), ['a', 'b', 'c']);
  assert.deepEqual(Object.keys(evict(entries, 0)), []);
});

test('isoNow formats without milliseconds or zone', () => {
  assert.equal(isoNow(new Date('2026-10-12T10:11:12.345Z')), '2026-10-12T10:11:12');
  assert.match(isoNow(), /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d$/);
});

test('effectiveTotal fallbacks', () => {
  assert.equal(effectiveTotal(180.7, 100), 180);
  assert.equal(effectiveTotal(NaN, 100), 100);
  assert.equal(effectiveTotal(Infinity, 100), 100);
  assert.equal(effectiveTotal(0, 100), 100);
  assert.equal(effectiveTotal(NaN, 0), null);
  assert.equal(effectiveTotal(NaN, null), null);
  assert.equal(effectiveTotal(NaN, undefined), null);
});

test('shouldUpload checks every guard', () => {
  const ok: UploadGuard = { sync: true, remoteKnown: true, position: 10, total: 100, lastUploaded: null };
  assert.equal(shouldUpload(ok), true);
  assert.equal(shouldUpload({ ...ok, sync: false }), false);
  assert.equal(shouldUpload({ ...ok, remoteKnown: false }), false);
  assert.equal(shouldUpload({ ...ok, total: null }), false);
  assert.equal(shouldUpload({ ...ok, position: 0 }), false);
  assert.equal(shouldUpload({ ...ok, lastUploaded: 10 }), false);
  assert.equal(shouldUpload({ ...ok, lastUploaded: 9 }), true);
});

test('buildPlayAction clamps started and formats the timestamp', () => {
  const now = new Date('2026-10-12T10:00:00.999Z');
  const a = buildPlayAction({ podcastUrl: 'p', episodeUrl: 'e', device: 'rpodder-web', started: 60, position: 40, total: 180, now });
  assert.deepEqual(a, {
    podcast: 'p', episode: 'e', device: 'rpodder-web', action: 'play',
    started: 40, position: 40, total: 180, timestamp: '2026-10-12T10:00:00',
  });
  assert.equal(buildPlayAction({ podcastUrl: 'p', episodeUrl: 'e', device: 'd', started: 10, position: 40, total: 180, now }).started, 10);
});

test('localTimestamp keeps the old timestamp for an unchanged position', () => {
  const prev = { position: 64, timestamp: '2026-10-12T10:00:00' };
  assert.equal(localTimestamp(prev, 64, '2026-10-12T11:00:00'), '2026-10-12T10:00:00');
  assert.equal(localTimestamp(prev, 65, '2026-10-12T11:00:00'), '2026-10-12T11:00:00');
  assert.equal(localTimestamp(null, 64, '2026-10-12T11:00:00'), '2026-10-12T11:00:00');
});

test('sameUser compares case-insensitively, null only equals null', () => {
  assert.equal(sameUser('alice', 'Alice'), true);
  assert.equal(sameUser('ALICE', 'alice'), true);
  assert.equal(sameUser('alice', 'bob'), false);
  assert.equal(sameUser('alice', null), false);
  assert.equal(sameUser(undefined, 'alice'), false);
  assert.equal(sameUser(null, null), true);
  assert.equal(sameUser(null, undefined), true);
});

test('remoteAccepted: a late remote read must not be newer than where playback resumed', () => {
  const local = pos(100, '2026-01-01T10:00:00', 3600, 'rpodder-web');
  assert.equal(remoteAccepted(null, local, 100, null), true, 'no remote entry');
  assert.equal(remoteAccepted(undefined, null, 0, null), true);
  assert.equal(remoteAccepted(pos(50, '2026-01-01T09:00:00'), local, 100, null), true, 'remote older');
  assert.equal(remoteAccepted(pos(50, '2026-01-01T10:00:00'), local, 100, null), true, 'tie keeps local');
  assert.equal(remoteAccepted(pos(900, '2026-01-01T11:00:00'), local, 100, null), false, 'remote newer');
  assert.equal(remoteAccepted(pos(900, '2026-01-01T11:00:00'), null, 0, null), false, 'resumed from 0, remote has a position');
  assert.equal(remoteAccepted(pos(3590, '2026-01-01T11:00:00', 3600), null, 0, null), true, 'remote finished: same restart point');
  assert.equal(remoteAccepted(pos(100, '2026-01-01T11:00:00'), local, 100.7, null), true, 'same position');
});
