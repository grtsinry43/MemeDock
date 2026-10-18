<script lang="ts">
  import LibraryPage from '#lib/features/library/components/LibraryPage.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<svelte:head>
  <title>{i18n.t.nav.library} · {i18n.t.nav.serviceName}</title>
</svelte:head>
<LibraryPage />
