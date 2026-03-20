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
  is_admin: boolean;
  devices: number;
  subscriptions: number;
}

export interface MeInfo {
  username: string;
  email?: string;
  is_admin: boolean;
  is_active: boolean;
}

export async function getMe(): Promise<MeInfo | null> {
  try {
    const resp = await request('/api/2/me');
    if (!resp.ok) return null;
    return resp.json();
  } catch {
    return null;
  }
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

export async function activateUser(username: string): Promise<boolean> {
  const resp = await request(`/api/admin/users/${username}/activate`, { method: 'POST' });
  return resp.ok;
}

export async function setUserRole(username: string, isAdmin: boolean): Promise<boolean> {
  const resp = await request(`/api/admin/users/${username}/role`, {
    method: 'POST',
    body: JSON.stringify({ is_admin: isAdmin }),
  });
  return resp.ok;
}

export async function adminResetPassword(username: string): Promise<{ ok: boolean; message?: string }> {
  const resp = await request(`/api/admin/users/${username}/reset-password`, { method: 'POST' });
  if (!resp.ok) return { ok: false };
  const data = await resp.json();
  return { ok: true, message: data.status || data.error };
}

export async function adminSetPassword(username: string, password: string): Promise<boolean> {
  const resp = await request(`/api/admin/users/${username}/password`, {
    method: 'POST',
    body: JSON.stringify({ password }),
  });
  return resp.ok;
}

export async function changeMyPassword(oldPassword: string | null, newPassword: string): Promise<{ ok: boolean; error?: string }> {
  const body: Record<string, string> = { new_password: newPassword };
  if (oldPassword) body.old_password = oldPassword;
  const resp = await request('/api/2/me/password', {
    method: 'POST',
    body: JSON.stringify(body),
  });
  if (!resp.ok) {
    try {
      const data = await resp.json();
      return { ok: false, error: data.error || 'Failed to change password' };
    } catch {
      return { ok: false, error: 'Failed to change password' };
    }
  }
  return { ok: true };
}

export async function requestPasswordReset(email: string): Promise<boolean> {
  const resp = await fetch(`${API_BASE}/api/2/password-reset`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email }),
  });
  return resp.ok;
}

export async function confirmPasswordReset(token: string, newPassword: string): Promise<boolean> {
  const resp = await fetch(`${API_BASE}/api/2/password-reset/confirm`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token, new_password: newPassword }),
  });
  return resp.ok;
}

export async function deleteUser(username: string): Promise<boolean> {
  const resp = await request(`/api/admin/users/${username}`, { method: 'DELETE' });
  return resp.ok;
}

export interface AdminStats {
  users: number;
  devices: number;
  subscriptions: number;
  podcasts: number;
  episode_actions: number;
}

export async function getAdminStats(): Promise<AdminStats | null> {
  try {
    const resp = await request('/api/admin/stats');
    if (!resp.ok) return null;
    return resp.json();
  } catch {
    return null;
  }
}

export async function forceUpdateFeeds(): Promise<boolean> {
  const resp = await request('/api/admin/feeds/update', { method: 'POST' });
  return resp.ok;
}

export async function forceUpdateSingleFeed(url: string): Promise<boolean> {
  const resp = await request(`/api/admin/feeds/update/single?url=${encodeURIComponent(url)}`, { method: 'POST' });
  return resp.ok;
}

export interface SsoInfo {
  enabled: boolean;
  provider_name: string;
  registration: string;
  podcastindex: boolean;
}

export async function getSsoInfo(): Promise<SsoInfo> {
  try {
    const resp = await fetch(`${API_BASE}/auth/sso/info`);
    if (!resp.ok) return { enabled: false, provider_name: 'SSO', registration: 'open', podcastindex: false };
    return resp.json();
  } catch {
    return { enabled: false, provider_name: 'SSO', registration: 'open', podcastindex: false };
  }
}

export async function searchPodcasts(query: string): Promise<PodcastInfo[]> {
  const resp = await request(`/search.json?q=${encodeURIComponent(query)}`);
  if (!resp.ok) return [];
  return resp.json();
}

export interface ExternalPodcast {
  title: string;
  url: string;
  description?: string;
  author?: string;
  logo_url?: string;
  language?: string;
  source: string;
}

export interface CombinedSearchResult {
  local: PodcastInfo[];
  external: ExternalPodcast[];
}

export async function getTrending(lang?: string, max: number = 20): Promise<ExternalPodcast[]> {
  const params = new URLSearchParams({ max: String(max) });
  if (lang) params.set('lang', lang);
  const resp = await request(`/api/2/trending?${params}`);
  if (!resp.ok) return [];
  return resp.json();
}

export async function searchAll(query: string): Promise<CombinedSearchResult> {
  const resp = await request(`/api/2/search/all?q=${encodeURIComponent(query)}`);
  if (!resp.ok) return { local: [], external: [] };
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
  build_tag: string;
  build_sha: string;
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

export interface UpgradeableSub {
  http_url: string;
  https_url: string;
  title: string;
}

export async function getSubscriptionUpgrades(): Promise<UpgradeableSub[]> {
  const resp = await request('/api/2/me/upgrades');
  if (!resp.ok) return [];
  return resp.json();
}

// --- Sync Groups ---

export interface SyncStatus {
  synchronized: string[][];
  'not-synchronized': string[];
}

export async function getSyncStatus(username: string): Promise<SyncStatus> {
  const resp = await request(`/api/2/sync-devices/${username}.json`);
  if (!resp.ok) return { synchronized: [], 'not-synchronized': [] };
  return resp.json();
}

export async function updateSyncStatus(
  username: string,
  synchronize: string[][],
  stopSynchronize?: string[]
): Promise<boolean> {
  const body: Record<string, unknown> = { synchronize };
  if (stopSynchronize && stopSynchronize.length > 0) {
    body['stop-synchronize'] = stopSynchronize;
  }
  const resp = await request(`/api/2/sync-devices/${username}.json`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
  return resp.ok;
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
