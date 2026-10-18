<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { overviewQuery } from '../queries.ts';
  import { formatBytes } from '#lib/shared/format.ts';
  import PageHeader from '#lib/ui/layout/PageHeader.svelte';
  import IconButton from '#lib/ui/primitives/IconButton.svelte';
  import QueryFeedback from '#lib/ui/common/QueryFeedback.svelte';
  import DeviceProgress from './DeviceProgress.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  const overview = createQuery(() => overviewQuery());
</script>

<PageHeader title={i18n.t.monitoring.title} description={i18n.t.monitoring.description}>
  {#snippet actions()}
    <IconButton
      label={i18n.t.monitoring.refreshLabel}
      disabled={overview.isFetching}
      onclick={() => overview.refetch()}
    >
      <RefreshCw
        size={18}
        class={overview.isFetching ? 'animate-spin motion-reduce:animate-none' : ''}
      />
    </IconButton>
  {/snippet}
</PageHeader>

<QueryFeedback pending={overview.isPending} error={overview.error} retry={() => overview.refetch()} />

{#if overview.data}
  {@const data = overview.data}
  <dl class="grid grid-cols-2 gap-x-6 gap-y-7 border-b border-separator pb-8 sm:grid-cols-4">
    {#each [
      { label: i18n.t.monitoring.statStickers, value: data.stickers.toLocaleString() },
      { label: i18n.t.monitoring.statCollections, value: data.collections.toLocaleString() },
      { label: i18n.t.monitoring.statTags, value: data.tags.toLocaleString() },
      { label: i18n.t.monitoring.statStorage, value: formatBytes(data.originalBytes) },
    ] as stat}
      <div>
        <dt class="text-caption text-secondary">{stat.label}</dt>
        <dd class="mt-2 text-title font-semibold tabular-nums">{stat.value}</dd>
      </div>
    {/each}
  </dl>
  <div class="mt-4 flex flex-wrap justify-between gap-2 text-caption text-secondary">
    <span>{i18n.t.monitoring.headSummary(data.head)}</span>
    <span>{i18n.t.monitoring.deletedSummary(data.deleted)}</span>
  </div>
  <section class="mt-10">
    <div class="mb-2 flex flex-wrap items-center justify-between gap-3">
      <h2 class="text-section font-semibold">{i18n.t.monitoring.deviceProgressSection}</h2>
      <a href="/devices" class="action action-quiet px-2">{i18n.t.monitoring.manageDevicesLink}</a>
    </div>
    {#each data.devices as device (device.id)}
      <DeviceProgress {device} head={data.head} />
    {:else}
      <p class="py-10 text-body text-secondary">{i18n.t.monitoring.noDevices}</p>
    {/each}
    <p class="mt-5 max-w-2xl text-caption leading-6 text-secondary">
      {i18n.t.monitoring.progressNote}
    </p>
  </section>
  <a
    href="/activity?result=rejected"
    class="mt-6 inline-flex text-body text-accent underline-offset-4 hover:underline"
  >
    {i18n.t.monitoring.rejectedSubmissionsLink}
  </a>
{/if}
