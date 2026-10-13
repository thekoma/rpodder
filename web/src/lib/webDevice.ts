// The shared "rpodder-web" device and the subscription rules of the web UI.
import {
  fetchAccountSettings,
  fetchDevices,
  fetchSubscriptions,
  fetchSyncStatus,
  postAccountSettings,
  postDevice,
  postSubscriptionChanges,
  postSyncStatus,
  type Device,
  type SyncGroupInfo,
} from './api';
import { normalizeUrl } from './playerLogic';

export const WEB_DEVICE_ID = 'rpodder-web';
export const WEB_DEVICE_CAPTION = 'Web player';
export const SYNC_POSITIONS_KEY = 'rpodder_web_sync_positions';

const ensured = new Map<string, Promise<void>>();

export async function ensureWebDevice(username: string): Promise<void> {
  let p = ensured.get(username);
  if (!p) {
    p = (async () => {
      const devices = await fetchDevices(username);
      const web = devices.find(d => d.id === WEB_DEVICE_ID);
      if (!web || web.caption === WEB_DEVICE_ID) {
        await postDevice(username, WEB_DEVICE_ID, WEB_DEVICE_CAPTION, 'other');
      }
    })();
    ensured.set(username, p);
    p.catch(() => { if (ensured.get(username) === p) ensured.delete(username); });
  }
  return p;
}

function forgetWebDevice(username: string) {
  ensured.delete(username);
}

async function withDeviceReset<T>(username: string, fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (e) {
    forgetWebDevice(username);
    throw e;
  }
}

function groupOf(groups: SyncGroupInfo[]): SyncGroupInfo | null {
  return groups.find(g => g.devices.includes(WEB_DEVICE_ID)) ?? null;
}

export async function webGroup(username: string): Promise<SyncGroupInfo | null> {
  return groupOf((await fetchSyncStatus(username)).groups ?? []);
}

export async function subscribe(username: string, url: string): Promise<void> {
  return withDeviceReset(username, async () => {
    await ensureWebDevice(username);
    if (await webGroup(username)) {
      await postSubscriptionChanges(username, WEB_DEVICE_ID, [url], []);
      return;
    }
    for (const d of await fetchDevices(username)) {
      await postSubscriptionChanges(username, d.id, [url], []);
    }
  });
}

export async function devicesWith(username: string, urls: string[]): Promise<Map<string, Device[]>> {
  const result = new Map<string, Device[]>();
  for (const u of urls) result.set(normalizeUrl(u), []);
  const devices = await fetchDevices(username);
  for (const d of devices) {
    const subs = new Set((await fetchSubscriptions(username, d.id)).map(normalizeUrl));
    for (const [key, owners] of result) {
      if (subs.has(key)) owners.push(d);
    }
  }
  return result;
}

export async function unsubscribe(username: string, devices: Device[], url: string): Promise<void> {
  return withDeviceReset(username, async () => {
    const target = normalizeUrl(url);
    for (const d of devices) {
      await postSubscriptionChanges(username, d.id, [], [target]);
    }
  });
}

export async function replaceSubscriptions(username: string, pairs: { from: string; to: string }[]): Promise<void> {
  if (pairs.length === 0) return;
  return withDeviceReset(username, async () => {
    const owners = await devicesWith(username, pairs.map(p => p.from));
    const perDevice = new Map<string, { add: string[]; remove: string[] }>();
    for (const p of pairs) {
      for (const d of owners.get(normalizeUrl(p.from)) ?? []) {
        let entry = perDevice.get(d.id);
        if (!entry) { entry = { add: [], remove: [] }; perDevice.set(d.id, entry); }
        entry.add.push(p.to);
        entry.remove.push(p.from);
      }
    }
    for (const [deviceId, { add, remove }] of perDevice) {
      await postSubscriptionChanges(username, deviceId, add, remove);
    }
  });
}

export interface WebSettings {
  syncPositions: boolean;
  group: SyncGroupInfo | null;
  groups: SyncGroupInfo[];
}

const SETTINGS_TTL_MS = 60_000;
let settingsCache: { username: string; at: number; value: WebSettings } | null = null;
let cacheGen = 0;

export async function getWebSettings(username: string): Promise<WebSettings> {
  const gen = cacheGen;
  const [account, status] = await Promise.all([fetchAccountSettings(username), fetchSyncStatus(username)]);
  const raw = account[SYNC_POSITIONS_KEY];
  const groups = status.groups ?? [];
  const value: WebSettings = {
    syncPositions: typeof raw === 'boolean' ? raw : true,
    group: groupOf(groups),
    groups,
  };
  if (gen === cacheGen) settingsCache = { username, at: Date.now(), value };
  return value;
}

/** Cached settings when fresh (60 s), else null. */
export function peekWebSettings(username: string): WebSettings | null {
  if (settingsCache && settingsCache.username === username && Date.now() - settingsCache.at < SETTINGS_TTL_MS) {
    return settingsCache.value;
  }
  return null;
}

export async function getWebSettingsCached(username: string): Promise<WebSettings> {
  return peekWebSettings(username) ?? getWebSettings(username);
}

export function clearWebSettingsCache(): void {
  settingsCache = null;
  cacheGen++;
}

export async function setSyncPositions(username: string, on: boolean): Promise<void> {
  clearWebSettingsCache();
  await postAccountSettings(username, { [SYNC_POSITIONS_KEY]: on });
}

export async function joinSyncGroup(username: string, group: SyncGroupInfo | null): Promise<void> {
  clearWebSettingsCache();
  return withDeviceReset(username, async () => {
    await ensureWebDevice(username);
    if (!group) {
      await postSyncStatus(username, [], [WEB_DEVICE_ID]);
      return;
    }
    await postSyncStatus(username, [{
      devices: [...group.devices.filter(d => d !== WEB_DEVICE_ID), WEB_DEVICE_ID],
      name: group.name || undefined,
    }]);
  });
}

export function groupLabel(g: SyncGroupInfo): string {
  return `${g.name || 'Unnamed group'} — ${g.devices.filter(d => d !== WEB_DEVICE_ID).join(', ')}`;
}

export interface Holdings {
  devices: Device[];
  groups: SyncGroupInfo[];
  /** Normalized feed URL → ids of the devices subscribed to it. */
  held: Map<string, Set<string>>;
}

/** Every device, its sync group, and which feeds each one holds. */
export async function fetchHoldings(username: string): Promise<Holdings> {
  const [devices, status] = await Promise.all([fetchDevices(username), fetchSyncStatus(username)]);
  const lists = await Promise.all(devices.map(d => fetchSubscriptions(username, d.id)));
  const held = new Map<string, Set<string>>();
  devices.forEach((d, i) => {
    for (const url of lists[i]) {
      const key = normalizeUrl(url);
      let set = held.get(key);
      if (!set) { set = new Set(); held.set(key, set); }
      set.add(d.id);
    }
  });
  return { devices, groups: status.groups ?? [], held };
}

/** Index of the device's sync group in `groups`, or -1. */
export function groupIndex(groups: SyncGroupInfo[], deviceId: string): number {
  return groups.findIndex(g => g.devices.includes(deviceId));
}
