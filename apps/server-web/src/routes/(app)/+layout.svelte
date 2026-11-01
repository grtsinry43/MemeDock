<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import ProtectedApp from '#lib/features/auth/components/ProtectedApp.svelte';
  let { children }: { children: Snippet } = $props();
</script>
<ProtectedApp>
  {#key page.url.pathname}
    <div class="page-enter">{@render children()}</div>
  {/key}
</ProtectedApp>
