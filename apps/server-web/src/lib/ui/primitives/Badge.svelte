<script lang="ts">
  import type { Snippet } from 'svelte';
  let { tone = 'neutral', children }: { tone?: 'neutral' | 'success'; children: Snippet } = $props();
</script>

<span class={`inline-flex items-center gap-1.5 rounded-badge px-2 py-1 text-caption font-medium ${tone === 'success' ? 'bg-success-surface text-success' : 'bg-input text-secondary'}`}>
  {@render children()}
</span>
