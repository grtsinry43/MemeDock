<script lang="ts">
  import type { ActivityEntry } from '#lib/generated/api.ts';
  import { formatTime } from '#lib/shared/format.ts';
  import { operationLabel, rejectionLabel, resultLabel } from '../model.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { items }: { items: ActivityEntry[] } = $props();
</script>

<ol aria-label={i18n.t.activity.listAriaLabel} class="divide-y divide-separator">
  {#each items as item (`${item.deviceId}:${item.opId}`)}
    <li class="grid min-w-0 gap-3 py-5 sm:grid-cols-[minmax(0,1fr)_auto]">
      <div class="min-w-0">
        <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
          <h2 class="text-body font-semibold">{operationLabel(item.kind)}</h2>
          <span
            class={`text-caption ${item.result === 'rejected' ? 'text-danger' : item.result === 'accepted' ? 'text-success' : 'text-secondary'}`}
          >
            {resultLabel(item.result)}
          </span>
          {#if item.eventSeq !== null}
            <span class="font-mono text-caption text-secondary">#{item.eventSeq}</span>
          {/if}
        </div>
        <p class="mt-1 text-caption text-secondary">{item.deviceName}</p>
        {#if item.errorCode}
          <p class="mt-2 text-caption text-danger">
            {rejectionLabel(item.errorCode)} <span class="font-mono">({item.errorCode})</span>
          </p>
        {/if}
        <details class="mt-2 text-caption text-secondary">
          <summary class="w-fit cursor-pointer rounded-sm py-1 hover:text-foreground">
            {i18n.t.activity.detailsToggle}
          </summary>
          <dl class="mt-1 grid grid-cols-[4rem_minmax(0,1fr)] gap-2">
            <dt>{i18n.t.activity.detailOpId}</dt>
            <dd class="break-all font-mono">{item.opId}</dd>
            {#if item.target}
              <dt>{i18n.t.activity.detailTarget}</dt>
              <dd class="break-all font-mono">{item.target}</dd>
            {/if}
            <dt>{i18n.t.activity.detailDevice}</dt>
            <dd class="break-all font-mono">{item.deviceId}</dd>
          </dl>
        </details>
      </div>
      <time class="text-caption text-secondary sm:text-right">
        {item.receivedAt === null ? i18n.t.common.timeNotRecorded : formatTime(item.receivedAt)}
      </time>
    </li>
  {/each}
</ol>
