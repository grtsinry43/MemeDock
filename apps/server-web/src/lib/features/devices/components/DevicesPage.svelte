<script lang="ts">
  import { Laptop } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import type { DeviceInfo } from '#lib/generated/api.ts';
  import { devicesQuery } from '../queries.ts';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import PageHeader from '#lib/ui/layout/PageHeader.svelte';
  import Button from '#lib/ui/primitives/Button.svelte';
  import CreateDeviceDialog from './CreateDeviceDialog.svelte';
  import DeviceList from './DeviceList.svelte';
  import RevokeDeviceDialog from './RevokeDeviceDialog.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  const devices = createQuery(() => devicesQuery());
  let selected = $state<DeviceInfo | null>(null);
</script>

<PageHeader title={i18n.t.devices.title} description={i18n.t.devices.description}>
  {#snippet actions()}<CreateDeviceDialog />{/snippet}
</PageHeader>

{#if devices.isError}
  <div role="alert" class="mb-5 flex flex-wrap items-center gap-3 text-body text-danger">
    <p class="min-w-0 break-words">{errorMessage(devices.error)}</p>
    <Button variant="secondary" onclick={() => devices.refetch()}>{i18n.t.common.reload}</Button>
  </div>
{/if}

{#if devices.isPending}
  <p role="status" class="panel p-8 text-body text-secondary">{i18n.t.devices.loading}</p>
{:else if devices.data?.items.length}
  <DeviceList devices={devices.data.items} onrevoke={device => { selected = device; }} />
{:else if devices.data}
  <div class="panel flex flex-col items-center px-6 py-16 text-center">
    <span class="mb-5 flex size-16 items-center justify-center rounded-panel bg-input text-secondary">
      <Laptop size={28} />
    </span>
    <h2 class="text-section font-semibold">{i18n.t.devices.emptyTitle}</h2>
    <p class="mt-3 max-w-sm text-body leading-6 text-secondary">{i18n.t.devices.emptyDesc}</p>
  </div>
{/if}

<p class="mt-5 text-caption leading-5 text-secondary">
  {i18n.t.devices.footerNote}
</p>

<RevokeDeviceDialog bind:device={selected} />
