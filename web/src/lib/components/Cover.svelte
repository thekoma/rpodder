<script lang="ts">
  import { fileLetter } from '$lib/format';

  // Covers come in three fixed scales so rows share one baseline.
  let { src = null, title = '', size = 'md', class: extra = '' }: {
    src?: string | null;
    title?: string;
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'fill';
    class?: string;
  } = $props();

  let failed = $state(false);
  $effect(() => {
    src;
    failed = false;
  });

  const sizes = {
    xs: 'w-9 h-9 text-sm',
    sm: 'w-12 h-12 text-base',
    md: 'w-14 h-14 text-lg',
    lg: 'w-36 h-36 sm:w-40 sm:h-40 text-5xl',
    fill: 'w-full aspect-square text-4xl',
  };
</script>

{#if src && !failed}
  <img src={src} alt="" loading="lazy" decoding="async" onerror={() => (failed = true)}
    class="{sizes[size]} shrink-0 rounded-[4px] object-cover bg-panel-2 border border-rule {extra}" />
{:else}
  <div aria-hidden="true"
    class="{sizes[size]} shrink-0 rounded-[4px] bg-brass-soft text-brass border border-rule flex items-center justify-center font-bold select-none {extra}">
    {fileLetter(title || '#')}
  </div>
{/if}
