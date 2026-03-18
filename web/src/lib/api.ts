// rpodder API client

const API_BASE = import.meta.env.VITE_API_BASE || '';

async function request(path: string, options: RequestInit = {}): Promise<Response> {
  const resp = await fetch(`${API_BASE}${path}`, {
    credentials: 'include',
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
  });
  return resp;
}

export interface LoginResult {
  ok: boolean;
  sessionCookie?: string;
}

export async function login(username: string, password: string): Promise<LoginResult> {
  const resp = await fetch(`${API_BASE}/api/2/auth/${username}/login.json`, {
    method: 'POST',
    credentials: 'include',
    headers: {
      'Authorization': 'Basic ' + btoa(`${username}:${password}`),
    },
  });
  return { ok: resp.ok };
}

export async function logout(username: string): Promise<void> {
  await fetch(`${API_BASE}/api/2/auth/${username}/logout.json`, {
    method: 'POST',
    credentials: 'include',
  });
}

export interface Device {
  id: string;
  caption: string;
  type: string;
  subscriptions: number;
}

export async function getDevices(username: string): Promise<Device[]> {
  const resp = await request(`/api/2/devices/${username}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function updateDevice(username: string, deviceId: string, caption: string, type: string): Promise<boolean> {
  const resp = await request(`/api/2/devices/${username}/${deviceId}.json`, {
    method: 'POST',
    body: JSON.stringify({ caption, type }),
  });
  return resp.ok;
}

export async function deleteDevice(username: string, deviceId: string): Promise<boolean> {
  const resp = await request(`/api/2/devices/${username}/${deviceId}.json`, {
    method: 'DELETE',
  });
  return resp.ok;
}

export async function getSubscriptions(username: string, device: string): Promise<string[]> {
  const resp = await request(`/subscriptions/${username}/${device}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function getAllSubscriptions(username: string): Promise<string[]> {
  const resp = await request(`/subscriptions/${username}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export interface PodcastInfo {
  url: string;
  title: string;
  description: string;
  website?: string;
  subscribers: number;
  logo_url?: string;
  author?: string;
  language?: string;
}

export async function getPodcastInfo(url: string): Promise<PodcastInfo | null> {
  try {
    const resp = await request(`/api/2/data/podcast.json?url=${encodeURIComponent(url)}`);
    if (!resp.ok) return null;
    return resp.json();
  } catch {
    return null;
  }
}

export interface EpisodeListItem {
  title: string;
  description?: string;
  released?: string;
  duration?: number;
  mimetype?: string;
}

export interface PodcastEpisodesResponse {
  podcast: PodcastInfo;
  episodes: EpisodeListItem[];
  total: number;
  page: number;
  per_page: number;
}

export async function getPodcastEpisodes(url: string, page: number = 0): Promise<PodcastEpisodesResponse | null> {
  try {
    const resp = await request(`/api/2/data/podcast/episodes.json?url=${encodeURIComponent(url)}&page=${page}&per_page=30`);
    if (!resp.ok) return null;
    return resp.json();
  } catch {
    return null;
  }
}

export interface HistoryItem {
  podcast_title: string;
  podcast_url: string;
  episode_title: string;
  action: string;
  timestamp: string;
  position?: number;
  total?: number;
}

export async function getHistory(username: string, page: number = 0): Promise<HistoryItem[]> {
  const resp = await request(`/api/2/history/${username}.json?page=${page}`);
  if (!resp.ok) return [];
  return resp.json();
}

export interface AdminUser {
  username: string;
  email?: string;
  active: boolean;
  devices: number;
  subscriptions: number;
}

export async function getAdminUsers(): Promise<AdminUser[]> {
  const resp = await request('/api/admin/users');
  if (!resp.ok) return [];
  return resp.json();
}

export async function createAdminUser(username: string, password: string, email?: string): Promise<boolean> {
  // Try public registration endpoint first, fallback to admin endpoint
  const resp = await fetch(`${API_BASE}/api/2/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password, email }),
  });
  return resp.ok;
}

export async function deactivateUser(username: string): Promise<boolean> {
  const resp = await request(`/api/admin/users/${username}/deactivate`, { method: 'POST' });
  return resp.ok;
}

export async function forceUpdateFeeds(): Promise<boolean> {
  const resp = await request('/api/admin/feeds/update', { method: 'POST' });
  return resp.ok;
}

export async function forceUpdateSingleFeed(url: string): Promise<boolean> {
  const resp = await request(`/api/admin/feeds/update/single?url=${encodeURIComponent(url)}`, { method: 'POST' });
  return resp.ok;
}

export async function searchPodcasts(query: string): Promise<PodcastInfo[]> {
  const resp = await request(`/search.json?q=${encodeURIComponent(query)}`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function getToplist(count: number = 50): Promise<PodcastInfo[]> {
  const resp = await request(`/toplist/${count}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export interface TagInfo {
  tag: string;
  usage: number;
}

export async function getTopTags(count: number = 50): Promise<TagInfo[]> {
  const resp = await request(`/api/2/tags/${count}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function getPodcastsForTag(tag: string, count: number = 50): Promise<PodcastInfo[]> {
  const resp = await request(`/api/2/tag/${tag}/${count}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function getSuggestions(username: string, count: number = 10): Promise<PodcastInfo[]> {
  const resp = await request(`/suggestions/${count}.json`);
  if (!resp.ok) return [];
  return resp.json();
}

export interface HealthInfo {
  status: string;
  version: string;
  database: string;
}

export async function getHealth(): Promise<HealthInfo | null> {
  try {
    const resp = await fetch(`${API_BASE}/health`);
    if (!resp.ok) return null;
    return resp.json();
  } catch {
    return null;
  }
}

export async function uploadSubscriptionChanges(
  username: string,
  device: string,
  add: string[],
  remove: string[]
): Promise<boolean> {
  const resp = await request(`/api/2/subscriptions/${username}/${device}.json`, {
    method: 'POST',
    body: JSON.stringify({ add, remove }),
  });
  return resp.ok;
}
