<script lang="ts">
  import PageHeader from '#lib/ui/layout/PageHeader.svelte';
  import LanguageSettings from './LanguageSettings.svelte';
  import ThemeSettings from './ThemeSettings.svelte';
  import PasswordForm from './PasswordForm.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<PageHeader title={i18n.t.settings.title} description={i18n.t.settings.description} />
<div class="grid gap-6">
  <LanguageSettings />
  <ThemeSettings />
  <PasswordForm />
</div>
