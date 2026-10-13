<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, subtitle = '', children, footer }: {
    title: string;
    subtitle?: string;
    children: Snippet;
    footer?: Snippet;
  } = $props();
</script>

<div class="max-w-[25rem] mx-auto pt-4 sm:pt-12">
  <div class="catalog-card shadow-[var(--shadow)] p-6 sm:p-8">
    <div class="mb-7">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{title}</h1>
      {#if subtitle}<p class="text-ink-2 mt-1">{subtitle}</p>{/if}
    </div>
    {@render children()}
  </div>
  {#if footer}
    <div class="mt-5 text-center text-sm text-ink-2 space-y-2">
      {@render footer()}
    </div>
  {/if}
</div>
