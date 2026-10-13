<script lang="ts">
  import { CircleCheck, CircleAlert, X } from '@lucide/svelte';
  import { toasts, dismiss } from '$lib/toast.svelte';
</script>

<div class="fixed z-[70] inset-x-0 top-16 flex flex-col items-center gap-2 px-4 pointer-events-none" aria-live="polite">
  {#each toasts.items as t (t.id)}
    <div class="pointer-events-auto flex items-center gap-2.5 max-w-md w-full sm:w-auto pl-3 pr-1.5 py-1.5 rounded-[6px] border bg-panel shadow-[var(--shadow-lift)] text-sm
      {t.kind === 'error' ? 'border-danger/50' : 'border-rule'}"
      role={t.kind === 'error' ? 'alert' : 'status'}>
      {#if t.kind === 'error'}
        <CircleAlert class="w-4 h-4 text-danger shrink-0" />
      {:else}
        <CircleCheck class="w-4 h-4 text-success shrink-0" />
      {/if}
      <span class="flex-1 min-w-0">{t.text}</span>
      <button class="btn btn-quiet btn-sm btn-icon" aria-label="Dismiss" onclick={() => dismiss(t.id)}>
        <X class="w-4 h-4" />
      </button>
    </div>
  {/each}
</div>
