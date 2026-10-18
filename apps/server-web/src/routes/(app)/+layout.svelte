<script lang="ts">
  import type { Snippet } from 'svelte';
  import ProtectedApp from '#lib/features/auth/components/ProtectedApp.svelte';
  let { children }: { children: Snippet } = $props();
</script>
<ProtectedApp>{@render children()}</ProtectedApp>
