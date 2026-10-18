<script lang="ts">
  import AccountPage from '#lib/features/account/components/AccountPage.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<svelte:head>
  <title>{i18n.t.nav.settings} · {i18n.t.nav.serviceName}</title>
</svelte:head>
<AccountPage />
