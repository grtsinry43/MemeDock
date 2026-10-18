<script lang="ts">
  import { ListFilter, RefreshCw } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { goto } from '$app/navigation';
  import { page as route } from '$app/state';
  import { activityQuery } from '../queries.ts';
  import { parseResult } from '../model.ts';
  import { devicesQuery } from '#lib/features/devices/queries.ts';
  import PageHeader from '#lib/ui/layout/PageHeader.svelte';
  import IconButton from '#lib/ui/primitives/IconButton.svelte';
  import QueryFeedback from '#lib/ui/common/QueryFeedback.svelte';
  import Pagination from '#lib/ui/common/Pagination.svelte';
  import ActivityList from './ActivityList.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  const result = $derived(parseResult(route.url.searchParams.get('result')));
  const device = $derived(route.url.searchParams.get('deviceId') ?? '');
  const page = $derived(Math.max(1, Math.min(10_000, Number(route.url.searchParams.get('page')) || 1)));
  const activity = createQuery(() => activityQuery({ result, deviceId: device || undefined, page }));
  const devices = createQuery(() => devicesQuery());

  function change(key: string, value: string) {
    const params = new URLSearchParams(route.url.searchParams.toString());
    if (value) params.set(key, value);
    else params.delete(key);
    if (key !== 'page') params.delete('page');
    void goto(`/activity${params.size ? `?${params}` : ''}`, { reset: false });
  }
</script>

<PageHeader title={i18n.t.activity.title} description={i18n.t.activity.description}>
  {#snippet actions()}
    <IconButton
      label={i18n.t.activity.refreshLabel}
      disabled={activity.isFetching}
      onclick={() => activity.refetch()}
    >
      <RefreshCw
        size={18}
        class={activity.isFetching ? 'animate-spin motion-reduce:animate-none' : ''}
      />
    </IconButton>
  {/snippet}
</PageHeader>

<div class="mb-7 flex flex-wrap items-end gap-4">
  <label class="grid min-w-40 gap-2 text-caption text-secondary">
    {i18n.t.activity.filterResult}
    <select
      class="field text-foreground"
      aria-label={i18n.t.activity.filterResultAria}
      value={result ?? ''}
      onchange={event => change('result', event.currentTarget.value)}
    >
      <option value="">{i18n.t.activity.allResults}</option>
      <option value="accepted">{i18n.t.activity.resultAccepted}</option>
      <option value="no_op">{i18n.t.activity.resultNoOp}</option>
      <option value="rejected">{i18n.t.activity.resultRejected}</option>
    </select>
  </label>
  <label class="grid min-w-40 gap-2 text-caption text-secondary">
    {i18n.t.activity.filterDevice}
    <select
      class="field max-w-xs text-foreground"
      aria-label={i18n.t.activity.filterDeviceAria}
      value={device}
      onchange={event => change('deviceId', event.currentTarget.value)}
    >
      <option value="">{i18n.t.activity.allDevices}</option>
      {#each devices.data?.items ?? [] as item (item.id)}
        <option value={item.id}>{item.name}</option>
      {/each}
    </select>
  </label>
</div>

<QueryFeedback pending={activity.isPending} error={activity.error} retry={() => activity.refetch()} />
<QueryFeedback pending={false} error={devices.error} retry={() => devices.refetch()} />

{#if activity.data}
  {#if activity.data.items.length}
    <ActivityList items={activity.data.items} />
  {:else}
    <div class="grid justify-items-center gap-3 py-16 text-center">
      <ListFilter size={32} class="text-secondary/60" />
      <h2 class="text-section font-semibold">{i18n.t.activity.emptyTitle}</h2>
      <p class="text-body text-secondary">{i18n.t.activity.emptyDesc}</p>
    </div>
  {/if}
  <Pagination
    {page}
    limit={activity.data.limit}
    total={activity.data.total}
    busy={activity.isFetching}
    onchange={value => change('page', String(value))}
  />
{/if}

<p class="mt-6 text-caption leading-6 text-secondary">
  {i18n.t.activity.receiptNote}
</p>
