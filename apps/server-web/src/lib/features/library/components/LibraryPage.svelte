<script lang="ts">
  import { Images, RefreshCw, Search, X } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import type { LibraryFacet } from '#lib/generated/api.ts';
  import { libraryQuery } from '../queries.ts';
  import PageHeader from '#lib/ui/layout/PageHeader.svelte';
  import IconButton from '#lib/ui/primitives/IconButton.svelte';
  import QueryFeedback from '#lib/ui/common/QueryFeedback.svelte';
  import Pagination from '#lib/ui/common/Pagination.svelte';
  import FacetPicker from './FacetPicker.svelte';
  import StickerTile from './StickerTile.svelte';
  import StickerDetail from './StickerDetail.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let draft = $state('');
  let search = $state('');
  let collection = $state<LibraryFacet | null>(null);
  let tag = $state<LibraryFacet | null>(null);
  let starred = $state(false);
  let deleted = $state(false);
  let page = $state(1);
  let selected = $state<string | null>(null);

  const filtered = $derived(Boolean(search || collection || tag || starred || deleted));
  const library = createQuery(() =>
    libraryQuery({ q: search, collectionId: collection?.id, tagId: tag?.id, starred, deleted, page })
  );

  function reset() {
    draft = '';
    search = '';
    collection = null;
    tag = null;
    starred = false;
    deleted = false;
    page = 1;
  }
</script>

<PageHeader title={i18n.t.library.title} description={i18n.t.library.description}>
  {#snippet actions()}
    <IconButton
      label={i18n.t.library.refreshLabel}
      disabled={library.isFetching}
      onclick={() => library.refetch()}
    >
      <RefreshCw
        size={18}
        class={library.isFetching ? 'animate-spin motion-reduce:animate-none' : ''}
      />
    </IconButton>
  {/snippet}
</PageHeader>

<form
  role="search"
  class="mb-4 flex items-center gap-2 rounded-control bg-input pl-3"
  onsubmit={event => {
    event.preventDefault();
    search = draft.trim();
    page = 1;
  }}
>
  <Search size={18} class="shrink-0 text-secondary" />
  <input
    type="search"
    aria-label={i18n.t.library.searchAriaLabel}
    placeholder={i18n.t.library.searchPlaceholder}
    bind:value={draft}
    maxlength={256}
    class="min-h-12 min-w-0 flex-1 bg-transparent py-2 text-body outline-none md:min-h-10"
  />
  <button type="submit" class="action action-primary m-1">{i18n.t.common.search}</button>
</form>

<div class="mb-7 flex flex-wrap items-center gap-x-4 gap-y-3">
  <div class="w-44">
    <FacetPicker
      kind="collections"
      label={i18n.t.library.filterCollection}
      selected={collection}
      onchange={value => {
        collection = value;
        page = 1;
      }}
    />
  </div>
  <div class="w-44">
    <FacetPicker
      kind="tags"
      label={i18n.t.library.filterTag}
      selected={tag}
      onchange={value => {
        tag = value;
        page = 1;
      }}
    />
  </div>
  <label class="flex min-h-11 items-center gap-2 text-body">
    <input
      type="checkbox"
      bind:checked={starred}
      onchange={() => {
        page = 1;
      }}
      class="size-4 accent-foreground"
    />
    {i18n.t.library.starredOnly}
  </label>
  <label class="flex min-h-11 items-center gap-2 text-body">
    <input
      type="checkbox"
      bind:checked={deleted}
      onchange={() => {
        page = 1;
      }}
      class="size-4 accent-foreground"
    />
    {i18n.t.library.deletedOnly}
  </label>
  {#if filtered}
    <button class="action action-quiet gap-1 px-2" onclick={reset}>
      <X size={14} />{i18n.t.library.clearFilters}
    </button>
  {/if}
</div>

<QueryFeedback
  pending={library.isPending}
  error={library.error}
  retry={() => library.refetch()}
  loading={i18n.t.library.loading}
/>

{#if library.data}
  <div class="mb-4 text-caption text-secondary" aria-live="polite">
    {filtered
      ? i18n.t.library.countFiltered(library.data.total)
      : i18n.t.library.countAll(library.data.total)}
  </div>
  {#if library.data.items.length}
    <div class="grid grid-cols-2 gap-x-4 gap-y-5 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
      {#each library.data.items as sticker (sticker.id)}
        <StickerTile {sticker} onselect={() => { selected = sticker.id; }} />
      {/each}
    </div>
  {:else}
    <div class="flex flex-col items-center py-20 text-center">
      <Images size={36} class="mb-5 text-secondary/60" />
      <h2 class="text-section font-semibold">
        {filtered ? i18n.t.library.emptyFilteredTitle : i18n.t.library.emptyTitle}
      </h2>
      <p class="mt-3 max-w-sm text-body leading-6 text-secondary">
        {filtered ? i18n.t.library.emptyFilteredDesc : i18n.t.library.emptyDesc}
      </p>
      {#if !filtered}
        <a href="/devices" class="action action-secondary mt-5">{i18n.t.library.connectDeviceAction}</a>
      {/if}
    </div>
  {/if}
  <Pagination
    {page}
    limit={library.data.limit}
    total={library.data.total}
    busy={library.isFetching}
    onchange={value => {
      page = value;
    }}
  />
{/if}

<StickerDetail bind:id={selected} />
