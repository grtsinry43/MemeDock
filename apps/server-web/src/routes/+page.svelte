<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import AppShellSkeleton from '#lib/ui/layout/AppShellSkeleton.svelte';

  onMount(() => {
    void goto('/library', { replace: true });
  });
</script>

<AppShellSkeleton />
