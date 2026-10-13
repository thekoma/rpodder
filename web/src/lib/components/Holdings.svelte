<script lang="ts">
  import type { Device } from '#lib/api.js';

  // Which of the user's devices hold a feed: one mark per device, filled when
  // the device is subscribed, in its sync group's colour.
  let { devices, held, groupOf, compact = false }: {
    devices: Device[];
    held: Set<string>;
    groupOf: (deviceId: string) => number;
    compact?: boolean;
  } = $props();

  let names = $derived(devices.filter(d => held.has(d.id)).map(d => d.caption || d.id));
  let summary = $derived(
    names.length === 0 ? 'Not on any device' : `On ${names.join(', ')}`
  );
</script>

{#if devices.length > 0}
  <span class="inline-flex items-center gap-1" title={summary}>
    <span class="sr-only">{summary}</span>
    {#each devices as d (d.id)}
      {@const g = groupOf(d.id)}
      {@const on = held.has(d.id)}
      <span aria-hidden="true"
        class="block rounded-[2px] border {compact ? 'w-2 h-2' : 'w-2.5 h-2.5'}"
        style="border-color: {g >= 0 ? `var(--group-${(g % 4) + 1})` : 'var(--ink-3)'}; background: {on ? (g >= 0 ? `var(--group-${(g % 4) + 1})` : 'var(--ink-2)') : 'transparent'}"
      ></span>
    {/each}
  </span>
{/if}
