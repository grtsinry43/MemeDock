<script lang="ts">
  import type { Snippet } from 'svelte';
  let { navigation, children }: { navigation: Snippet; children: Snippet } = $props();
</script>
<div class="min-h-dvh md:grid md:grid-cols-[232px_minmax(0,1fr)]">
  <aside class="bg-surface p-4 md:sticky md:top-0 md:flex md:h-dvh md:flex-col md:p-6">{@render navigation()}</aside>
  <main class="min-w-0 px-5 py-8 md:px-12 md:py-12"><div class="mx-auto max-w-6xl">{@render children()}</div></main>
</div>
