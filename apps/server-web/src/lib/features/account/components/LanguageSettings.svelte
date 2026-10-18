<script lang="ts">
  import { Languages } from '@lucide/svelte';
  import { i18n, type LanguagePreference } from '#lib/shared/i18n/i18n.svelte.ts';

  const options: { value: LanguagePreference; label: () => string }[] = [
    { value: 'system', label: () => i18n.t.settings.languageSystem },
    { value: 'zh', label: () => i18n.t.settings.languageZh },
    { value: 'en', label: () => i18n.t.settings.languageEn },
  ];
</script>

<section class="panel p-6">
  <h2 class="text-section font-semibold">{i18n.t.settings.languageTitle}</h2>
  <p class="mt-2 text-body text-secondary">{i18n.t.settings.languageDesc}</p>
  <fieldset class="mt-5 flex flex-wrap gap-2">
    <legend class="sr-only">{i18n.t.settings.languageFieldsetLegend}</legend>
    {#each options as option}
      <label
        class={[
          'action relative cursor-pointer gap-2 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent',
          i18n.preference === option.value ? 'bg-input font-semibold' : 'hover:bg-input text-secondary',
        ]}
      >
        <input
          type="radio"
          name="language"
          value={option.value}
          checked={i18n.preference === option.value}
          onchange={() => i18n.set(option.value)}
          class="absolute inset-0 size-full cursor-pointer opacity-0"
        />
        <Languages size={18} />
        {option.label()}
      </label>
    {/each}
  </fieldset>
</section>
