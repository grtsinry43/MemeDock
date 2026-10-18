<script lang="ts">
  import ActivityPage from '#lib/features/activity/components/ActivityPage.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<svelte:head>
  <title>{i18n.t.nav.activity} · {i18n.t.nav.serviceName}</title>
</svelte:head>
<ActivityPage />
