<script lang="ts">
  import MonitoringPage from '#lib/features/monitoring/components/MonitoringPage.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<svelte:head>
  <title>{i18n.t.nav.sync} · {i18n.t.nav.serviceName}</title>
</svelte:head>
<MonitoringPage />
