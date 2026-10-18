<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  onMount(() => {
    void goto('/library', { replace: true });
  });
</script>

<p>{i18n.t.library.opening}</p>
