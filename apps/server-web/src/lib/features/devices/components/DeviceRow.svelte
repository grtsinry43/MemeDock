<script lang="ts">
  import { Laptop, MoreHorizontal, Unplug } from '@lucide/svelte';
  import { DropdownMenu } from 'bits-ui';
  import type { DeviceInfo } from '#lib/generated/api.ts';
  import Menu from '#lib/ui/primitives/DropdownMenu.svelte';
  import Badge from '#lib/ui/primitives/Badge.svelte';
  import { formatTime } from '#lib/shared/format.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { device, onrevoke }: { device: DeviceInfo; onrevoke: (device: DeviceInfo) => void } = $props();
</script>

<li class="flex min-w-0 items-start gap-4 px-5 py-5 sm:items-center sm:px-6">
  <span class="mt-1 flex size-11 shrink-0 items-center justify-center rounded-control bg-input text-secondary">
    <Laptop size={21} />
  </span>
  <div class="min-w-0 flex-1">
    <div class="flex flex-wrap items-center gap-2">
      <h2 class="break-all text-body font-semibold">{device.name}</h2>
      <Badge tone={device.revokedAt ? 'neutral' : 'success'}>
        {device.revokedAt ? i18n.t.devices.statusRevoked : i18n.t.devices.statusActive}
      </Badge>
    </div>
    <p class="mt-2 text-caption leading-5 text-secondary">
      {device.lastSeenAt ? i18n.t.devices.lastSeen(formatTime(device.lastSeenAt)) : i18n.t.devices.neverConnected}
      <span class="mx-2 hidden sm:inline">·</span>
      <span class="block sm:inline">{i18n.t.devices.createdAt(formatTime(device.createdAt))}</span>
    </p>
  </div>
  {#if !device.revokedAt}
    <Menu label={i18n.t.devices.moreActionsAria(device.name)}>
      {#snippet trigger()}<MoreHorizontal size={20} />{/snippet}
      <DropdownMenu.Item class="menu-item text-danger" onclick={() => onrevoke(device)}>
        <Unplug size={16} />{i18n.t.devices.revokeAction}
      </DropdownMenu.Item>
    </Menu>
  {/if}
</li>
