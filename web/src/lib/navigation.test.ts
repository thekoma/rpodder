// @ts-nocheck -- run by `node --test` (native type stripping); not part of the Vite bundle or svelte-check.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rememberNext, safeNext, takeNext } from './navigation.ts';

const ORIGIN = 'https://pod.example.com';

function memoryStore() {
  const data = new Map();
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
  };
}

test('safeNext accepts local paths', () => {
  assert.equal(safeNext('/settings?x=1', ORIGIN), '/settings?x=1');
  assert.equal(safeNext('/login/flow#abc', ORIGIN), '/login/flow#abc');
});

test('safeNext refuses anything that could leave the site', () => {
  for (const value of [
    '//evil.com', '/\\evil.com', '/\t/evil.com', '/%5Cevil.com', '/%2F%2Fevil.com',
    '/%2f/evil.com', 'https://evil.com', 'javascript:alert(1)', 'settings', '%zz', '', null,
  ]) {
    assert.equal(safeNext(value, ORIGIN), null, String(value));
  }
});

test('takeNext returns a fresh target once', () => {
  const store = memoryStore();
  rememberNext('/login/flow#abc', store, 1_000);
  assert.equal(takeNext(ORIGIN, store, 2_000), '/login/flow#abc');
  assert.equal(takeNext(ORIGIN, store, 3_000), null);
});

test('takeNext drops stale and unsafe targets', () => {
  const store = memoryStore();
  rememberNext('/login/flow#abc', store, 0);
  assert.equal(takeNext(ORIGIN, store, 16 * 60 * 1000), null);

  rememberNext('//evil.com', store, 0);
  assert.equal(takeNext(ORIGIN, store, 1_000), null);

  store.setItem('rpodder_next', 'not json');
  assert.equal(takeNext(ORIGIN, store, 1_000), null);
});

test('takeNext without storage is null', () => {
  assert.equal(takeNext(ORIGIN, null), null);
});
