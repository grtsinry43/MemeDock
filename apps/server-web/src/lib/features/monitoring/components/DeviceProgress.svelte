<script lang="ts">
  import { Laptop } from '@lucide/svelte';
  import type { DeviceSyncProgress } from '#lib/generated/api.ts';
  import { formatTime } from '#lib/shared/format.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { device, head }: { device: DeviceSyncProgress; head: number } = $props();
  const behind = $derived(device.appliedCursor === null ? null : Math.max(0, head - device.appliedCursor));
</script>

<article
  class="grid gap-4 border-b border-separator py-5 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(12rem,0.8fr)] sm:items-center"
>
  <div class="flex min-w-0 gap-3">
    <span class="flex size-10 shrink-0 items-center justify-center rounded-control bg-input text-secondary">
      <Laptop size={19} />
    </span>
    <div class="min-w-0">
      <h3 class="break-words text-body font-semibold">{device.name}</h3>
      <p class="mt-1 text-caption text-secondary">
        {device.lastSeenAt
          ? i18n.t.monitoring.lastSeen(formatTime(device.lastSeenAt))
          : i18n.t.monitoring.neverConnected}
      </p>
    </div>
  </div>
  <div class="min-w-0">
    <div class="flex flex-wrap items-center justify-between gap-2 text-caption">
      <span
        class={device.revoked ? 'text-secondary' : behind === 0 ? 'text-success' : 'text-secondary'}
      >
        {device.revoked
          ? i18n.t.monitoring.statusRevoked
          : behind === null
            ? i18n.t.monitoring.statusAwaiting
            : behind === 0
              ? i18n.t.monitoring.statusUpToDate
              : i18n.t.monitoring.statusBehind(behind)}
      </span>
      {#if device.appliedCursor !== null}
        <span class="text-secondary">{device.appliedCursor} / {head}</span>
      {/if}
    </div>
    {#if device.appliedCursor !== null}
      <progress
        aria-label={i18n.t.monitoring.progressAriaLabel(device.name)}
        class="mt-2 h-1.5 w-full overflow-hidden rounded-full accent-foreground"
        max={Math.max(1, head)}
        value={head === 0 ? 1 : device.appliedCursor}
      ></progress>
    {/if}
    {#if device.observedAt !== null}
      <p class="mt-1 text-[11px] text-secondary">
        {i18n.t.monitoring.observedAt(formatTime(device.observedAt))}
      </p>
    {/if}
  </div>
</article>
