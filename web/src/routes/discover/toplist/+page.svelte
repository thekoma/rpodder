<script lang="ts">
  import { getToplist, type PodcastInfo } from '$lib/api';
  import { browser } from '$app/environment';

  let podcasts = $state<PodcastInfo[]>([]);
  let loading = $state(true);
  let loaded = $state(false);

  $effect(() => {
    if (!browser || loaded) return;
    loaded = true;
    getToplist(50).then(p => { podcasts = p; loading = false; }).catch(() => { loading = false; });
  });
</script>

<div class="space-y-6">
  <h1 class="text-2xl font-bold">Toplist</h1>

  {#if loading}
    <div class="text-center text-text-dim py-12">Loading...</div>
  {:else if podcasts.length === 0}
    <p class="text-text-dim text-center py-8">No podcasts indexed yet.</p>
  {:else}
    <div class="bg-surface border border-border rounded-xl overflow-hidden">
      <table class="w-full">
        <thead>
          <tr class="border-b border-border text-left">
            <th class="px-4 py-3 text-xs text-text-dim font-medium w-12">#</th>
            <th class="px-4 py-3 text-xs text-text-dim font-medium">Podcast</th>
            <th class="px-4 py-3 text-xs text-text-dim font-medium text-right w-40">Subscribers</th>
          </tr>
        </thead>
        <tbody>
          {#each podcasts as podcast, i}
            <tr class="border-b border-border last:border-0 hover:bg-surface-hover transition-colors">
              <td class="px-4 py-3 text-text-dim text-sm">{i + 1}</td>
              <td class="px-4 py-3">
                <div class="flex items-center gap-3">
                  {#if podcast.logo_url}
                    <img src={podcast.logo_url} alt="" class="w-10 h-10 rounded object-cover shrink-0" loading="lazy" />
                  {:else}
                    <div class="w-10 h-10 rounded bg-brand-dim flex items-center justify-center text-lg shrink-0">🎙️</div>
                  {/if}
                  <div class="min-w-0">
                    <p class="font-medium text-sm text-brand truncate">{podcast.title}</p>
                    {#if podcast.author}<p class="text-xs text-text-dim truncate">{podcast.author}</p>{/if}
                  </div>
                </div>
              </td>
              <td class="px-4 py-3 text-right">
                <div class="flex items-center justify-end gap-2">
                  <span class="text-sm text-text-dim">{podcast.subscribers}</span>
                  <div class="w-24 h-2 bg-bg rounded-full overflow-hidden">
                    <div
                      class="h-full bg-brand rounded-full"
                      style="width: {Math.min((podcast.subscribers / (podcasts[0]?.subscribers || 1)) * 100, 100)}%"
                    ></div>
                  </div>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
