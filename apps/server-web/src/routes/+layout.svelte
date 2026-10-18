<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import QueryRoot from '#lib/ui/common/QueryRoot.svelte';
  import { theme } from '#lib/shared/theme/theme.svelte.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
  import './layout.css';
  let { children }: { children: Snippet } = $props();
  onMount(() => {
    const cleanupTheme = theme.initialize();
    const cleanupI18n = i18n.initialize();
    return () => {
      cleanupTheme?.();
      cleanupI18n?.();
    };
  });
</script>
<svelte:head><title>{i18n.t.nav.serviceName} · {i18n.t.nav.serviceTag}</title><meta name="robots" content="noindex,nofollow" /></svelte:head>
<QueryRoot>{@render children()}</QueryRoot>
