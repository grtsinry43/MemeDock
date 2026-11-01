<script lang="ts">
  import { Check, Copy, ExternalLink } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { versionQuery } from '../queries.ts';
  import { copyText } from '#lib/shared/dom/clipboard.ts';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  const version = createQuery(() => versionQuery());
  const origin = typeof window === 'undefined' ? '' : window.location.origin;
  let copyState = $state<'idle' | 'copied' | 'failed'>('idle');
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const copyValue = $derived(
    version.data
      ? `MemeDock ${version.data.version} · protocol ${version.data.protocolVersion} · ${version.data.debug ? 'debug' : 'release'} · ${origin}`
      : ''
  );

  $effect(() => () => {
    if (copyTimer !== null) clearTimeout(copyTimer);
  });

  async function copyInfo() {
    if (!copyValue) return;
    copyState = (await copyText(copyValue)) ? 'copied' : 'failed';
    if (copyTimer !== null) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copyState = 'idle';
      copyTimer = null;
    }, 2000);
  }
</script>

<section class="panel p-6">
  <h2 class="text-section font-semibold">{i18n.t.about.title}</h2>
  <p class="mt-2 text-body text-secondary">{i18n.t.about.description}</p>

  <dl class="mt-5 divide-y divide-separator">
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.version}</dt>
      <dd class="min-w-0 text-body">
        {#if version.isPending}
          <Skeleton class="h-5 w-16 rounded-control" />
        {:else if version.data}
          <span class="font-mono">v{version.data.version}</span>
        {:else}
          <span class="text-secondary">—</span>
        {/if}
      </dd>
    </div>
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.protocolVersion}</dt>
      <dd class="min-w-0 text-body">
        {#if version.isPending}
          <Skeleton class="h-5 w-10 rounded-control" />
        {:else if version.data}
          <span class="font-mono">{version.data.protocolVersion}</span>
        {:else}
          <span class="text-secondary">—</span>
        {/if}
      </dd>
    </div>
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.buildMode}</dt>
      <dd class="min-w-0 text-body">
        {#if version.isPending}
          <Skeleton class="h-5 w-14 rounded-control" />
        {:else if version.data}
          {version.data.debug ? i18n.t.about.buildDebug : i18n.t.about.buildRelease}
        {:else}
          <span class="text-secondary">—</span>
        {/if}
      </dd>
    </div>
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.serviceAddress}</dt>
      <dd class="min-w-0 max-w-full break-all font-mono text-caption text-secondary">{origin}</dd>
    </div>
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.license}</dt>
      <dd class="min-w-0 text-body">MIT</dd>
    </div>
    <div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 py-3">
      <dt class="text-body text-secondary">{i18n.t.about.projectHome}</dt>
      <dd class="min-w-0 text-body">
        <a
          href="https://github.com/grtsinry43/MemeDock"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1 text-accent underline-offset-4 hover:underline"
        >
          github.com/grtsinry43/MemeDock
          <ExternalLink size={13} />
        </a>
      </dd>
    </div>
  </dl>

  {#if version.isError}
    <div role="alert" class="mt-3 flex flex-wrap items-center gap-3 text-caption text-danger">
      <p class="min-w-0 flex-1 break-words">{errorMessage(version.error)}</p>
      <button type="button" class="action action-quiet px-2" onclick={() => version.refetch()}>
        {i18n.t.common.retry}
      </button>
    </div>
  {/if}

  <div class="mt-5 flex flex-wrap items-center gap-3">
    <button type="button" class="action action-secondary gap-1.5" disabled={!copyValue} onclick={copyInfo}>
      {#if copyState === 'copied'}<Check size={16} />{:else}<Copy size={16} />{/if}
      {copyState === 'copied'
        ? i18n.t.about.copied
        : copyState === 'failed'
          ? i18n.t.about.copyFailed
          : i18n.t.about.copyInfo}
    </button>
    {#if copyState !== 'idle'}
      <span class="sr-only" role="status">
        {copyState === 'copied' ? i18n.t.about.copied : i18n.t.about.copyFailed}
      </span>
    {/if}
  </div>
</section>
