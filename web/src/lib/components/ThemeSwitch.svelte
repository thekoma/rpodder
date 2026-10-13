<script lang="ts">
  import { Monitor, Sun, Moon } from '@lucide/svelte';
  import { theme, type ThemeChoice } from '#lib/theme.svelte.js';

  const options: { value: ThemeChoice; label: string; icon: typeof Sun }[] = [
    { value: 'system', label: 'System', icon: Monitor },
    { value: 'light', label: 'Light', icon: Sun },
    { value: 'dark', label: 'Dark', icon: Moon },
  ];
</script>

<div role="radiogroup" aria-label="Theme" class="inline-flex p-0.5 rounded-[6px] bg-panel-2 border border-rule">
  {#each options as o (o.value)}
    {@const Icon = o.icon}
    <button role="radio" aria-checked={theme.choice === o.value}
      onclick={() => theme.set(o.value)}
      class="inline-flex items-center gap-1.5 h-8 px-3 rounded-[4px] text-sm font-semibold cursor-pointer transition-colors
        {theme.choice === o.value ? 'bg-panel text-ink shadow-[var(--shadow)]' : 'text-ink-2 hover:text-ink'}">
      <Icon class="w-4 h-4" />
      {o.label}
    </button>
  {/each}
</div>
