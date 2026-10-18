<script lang="ts">
  import type { Snippet } from 'svelte';
  let { title, description, actions }: { title: string; description: string; actions?: Snippet } = $props();
</script>
<header class="mb-8 flex items-start justify-between gap-5"><div class="min-w-0 flex-1"><h1 class="text-title font-semibold tracking-tight">{title}</h1><p class="mt-2 text-body leading-6 text-secondary">{description}</p></div>{#if actions}<div class="shrink-0">{@render actions()}</div>{/if}</header>
