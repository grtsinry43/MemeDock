<script lang="ts">
  import { Combobox } from 'bits-ui';
  import { ChevronDown, X } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import type { LibraryFacet } from '#lib/generated/api.ts';
  import { facetsQuery } from '../queries.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let {
    kind,
    label,
    selected = $bindable(null),
    onchange,
  }: {
    kind: 'collections' | 'tags';
    label: string;
    selected?: LibraryFacet | null;
    onchange: (value: LibraryFacet | null) => void;
  } = $props();

  let search = $state('');
  let open = $state(false);
  const query = createQuery(() => facetsQuery(kind, search));
</script>

<div class="flex items-center gap-1">
  <Combobox.Root
    type="single"
    value={selected?.name ?? ''}
    onValueChange={name => {
      const found = query.data?.items.find(item => item.name === name);
      if (found) {
        onchange(found);
        open = false;
      }
    }}
    bind:open
  >
    <div class="relative min-w-0 flex-1">
      <Combobox.Input
        clearOnDeselect
        aria-label={label}
        placeholder={label}
        oninput={event => {
          search = event.currentTarget.value;
          open = true;
        }}
        class="field min-w-0 pr-9"
      />
      <Combobox.Trigger
        class="absolute right-2 top-1/2 -translate-y-1/2 rounded-control p-1 text-secondary"
        aria-label={kind === 'collections'
          ? i18n.t.library.expandCollectionAria(label)
          : i18n.t.library.expandTagAria(label)}
      >
        <ChevronDown size={16} />
      </Combobox.Trigger>
    </div>
    <Combobox.Portal>
      <Combobox.Content class="popup min-w-44" sideOffset={6}>
        {#if query.isPending}
          <p role="status" class="px-3 py-4 text-caption text-secondary">{i18n.t.library.facetLoading}</p>
        {/if}
        {#each query.data?.items ?? [] as item (item.id)}
          <Combobox.Item value={item.name} label={item.name} class="menu-item">
            {item.name}
          </Combobox.Item>
        {/each}
        {#if query.data?.items.length === 0}
          <p class="px-3 py-4 text-caption text-secondary">{i18n.t.library.facetNoMatch}</p>
        {/if}
        {#if query.data?.hasMore}
          <p class="px-3 py-2 text-caption text-secondary">{i18n.t.library.facetMore}</p>
        {/if}
      </Combobox.Content>
    </Combobox.Portal>
  </Combobox.Root>
  {#if selected}
    <button
      type="button"
      class="action action-quiet size-9 shrink-0 p-0"
      aria-label={kind === 'collections'
        ? i18n.t.library.clearCollectionAria(label)
        : i18n.t.library.clearTagAria(label)}
      onclick={() => onchange(null)}
    >
      <X size={15} />
    </button>
  {/if}
</div>
