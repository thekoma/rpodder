<script lang="ts">
  let { children } = $props();
</script>

<div class="flex gap-6">
  <!-- Sidebar -->
  <nav class="w-48 shrink-0 hidden md:block">
    <div class="sticky top-20 space-y-1">
      <h3 class="text-xs text-text-dim uppercase tracking-wider mb-2 px-3">Discover</h3>
      <a href="/discover" class="block px-3 py-1.5 text-sm rounded-md hover:bg-surface-hover transition-colors text-text-dim hover:text-text">Directory</a>
      <a href="/discover/toplist" class="block px-3 py-1.5 text-sm rounded-md hover:bg-surface-hover transition-colors text-text-dim hover:text-text">Toplist</a>

      <h3 class="text-xs text-text-dim uppercase tracking-wider mt-4 mb-2 px-3">Search</h3>
      <a href="/discover?search=1" class="block px-3 py-1.5 text-sm rounded-md hover:bg-surface-hover transition-colors text-text-dim hover:text-text">Search</a>
    </div>
  </nav>

  <!-- Main content -->
  <div class="flex-1 min-w-0">
    {@render children()}
  </div>
</div>
