<script lang="ts">
  import { Monitor, Sun, Moon } from '@lucide/svelte';
  import { theme, type ThemePreference } from '#lib/shared/theme/theme.svelte.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  const options: { value: ThemePreference; label: () => string; icon: typeof Monitor }[] = [
    { value: 'system', label: () => i18n.t.settings.themeSystem, icon: Monitor },
    { value: 'light', label: () => i18n.t.settings.themeLight, icon: Sun },
    { value: 'dark', label: () => i18n.t.settings.themeDark, icon: Moon },
  ];
</script>

<section class="panel p-6">
  <h2 class="text-section font-semibold">{i18n.t.settings.themeTitle}</h2>
  <p class="mt-2 text-body text-secondary">{i18n.t.settings.themeDesc}</p>
  <fieldset class="mt-5 flex flex-wrap gap-2">
    <legend class="sr-only">{i18n.t.settings.themeFieldsetLegend}</legend>
    {#each options as option}
      <label
        class={[
          'action relative cursor-pointer gap-2 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent',
          theme.preference === option.value ? 'bg-input font-semibold' : 'hover:bg-input text-secondary',
        ]}
      >
        <input
          type="radio"
          name="theme"
          value={option.value}
          checked={theme.preference === option.value}
          onchange={() => theme.set(option.value)}
          class="absolute inset-0 size-full cursor-pointer opacity-0"
        />
        <option.icon size={18} />
        {option.label()}
      </label>
    {/each}
  </fieldset>
</section>
