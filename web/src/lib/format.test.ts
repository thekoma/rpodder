// @ts-nocheck -- run by `node --test` (native type stripping); not part of the Vite bundle or svelte-check.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  dayLabel, feedFallbackTitle, fileKey, fileLetter, formatClock, formatDuration, groupByLetter, looksLikeFeedUrl, minutesLeft,
  plainText, plural, uniqueByUrl,
} from './format.ts';

test('looksLikeFeedUrl accepts http(s) addresses only', () => {
  assert.equal(looksLikeFeedUrl('https://feeds.example.com/show.xml'), true);
  assert.equal(looksLikeFeedUrl('  http://example.com/rss  '), true);
  assert.equal(looksLikeFeedUrl('planet money'), false);
  assert.equal(looksLikeFeedUrl('feeds.example.com/show.xml'), false);
  assert.equal(looksLikeFeedUrl('https://exa mple.com/x'), false);
  assert.equal(looksLikeFeedUrl('ftp://example.com/x'), false);
});

test('fileLetter and fileKey ignore leading articles', () => {
  assert.equal(fileLetter('The Daily'), 'D');
  assert.equal(fileLetter('a16z Podcast'), 'A');
  assert.equal(fileLetter('99% Invisible'), '#');
  assert.equal(fileLetter('Élan'), 'E');
  assert.equal(fileKey('The Vergecast'), 'vergecast');
  assert.ok(fileKey('The Daily').localeCompare(fileKey('Darknet Diaries')) < 0);
});

test('durations and clocks', () => {
  assert.equal(formatDuration(0), '');
  assert.equal(formatDuration(30), '1 min');
  assert.equal(formatDuration(2520), '42 min');
  assert.equal(formatDuration(3900), '1h 05m');
  assert.equal(formatClock(65), '1:05');
  assert.equal(formatClock(3725), '1:02:05');
  assert.equal(minutesLeft(1260, 1680), 7);
  assert.equal(minutesLeft(2000, 1000), 0);
});

test('dayLabel names today and yesterday', () => {
  const now = new Date(2026, 9, 13, 10, 0);
  assert.equal(dayLabel(new Date(2026, 9, 13, 1, 0).toISOString(), now), 'Today');
  assert.equal(dayLabel(new Date(2026, 9, 12, 23, 0).toISOString(), now), 'Yesterday');
  assert.equal(dayLabel('not a date', now), '');
});

test('plainText strips markup (no DOM: regex fallback)', () => {
  assert.equal(plainText('Hello <em>world</em><br/>again'), 'Hello world again');
  assert.equal(plainText('Plain text'), 'Plain text');
  assert.equal(plainText(null), '');
});

test('fallback titles and plurals', () => {
  assert.equal(feedFallbackTitle('https://www.example.com/feed.xml'), 'example.com');
  assert.equal(plural(1, 'device'), '1 device');
  assert.equal(plural(3, 'device'), '3 devices');
});

test('groupByLetter files each letter once, whatever the script', () => {
  const titles = ['99% Invisible', 'Darknet Diaries', 'Радио Т', 'Łódź Stories', 'Lex Fridman', 'Mystery', 'Øresund', 'Oa', 'Pod', 'The Daily'];
  const groups = groupByLetter(titles, t => t);
  const letters = groups.map(g => g.letter);
  assert.deepEqual(letters, [...new Set(letters)], `letters repeat: ${letters.join(' ')}`);
  assert.equal(letters[0], '#');
  assert.deepEqual(groups.find(g => g.letter === 'D')!.rows, ['The Daily', 'Darknet Diaries']);
  assert.equal(groups.reduce((n, g) => n + g.rows.length, 0), titles.length);
});

test('uniqueByUrl keeps the first of duplicate feeds', () => {
  const items = [{ url: 'https://a.example/feed' }, { url: 'https://A.example/feed/' }, { url: 'https://b.example/feed' }];
  assert.deepEqual(uniqueByUrl(items).map(i => i.url), ['https://a.example/feed', 'https://b.example/feed']);
});
