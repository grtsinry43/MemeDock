<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { X, ImageOff } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { detailQuery } from '../queries.ts';
  import { originalUrl } from '../api.ts';
  import { formatBytes, formatTime } from '#lib/shared/format.ts';
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { id = $bindable(null) }: { id?: string | null } = $props();
  const query = createQuery(() => detailQuery(id));
  let failed = $state(false);
  let loaded = $state(false);
  let attempt = $state(0);
  $effect(() => {
    if (id) {
      failed = false;
      loaded = false;
      attempt = 0;
    }
  });
</script>

<Dialog.Root open={id !== null} onOpenChange={open => { if (!open) id = null; }}>
  <Dialog.Portal>
    <Dialog.Overlay class="overlay-enter fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content class="dialog-panel max-w-2xl">
      <Dialog.Title class="pr-10 text-section font-semibold">{i18n.t.library.detailTitle}</Dialog.Title>
      <Dialog.Description class="mt-2 text-caption text-secondary">{i18n.t.library.detailDescription}</Dialog.Description>
      <Dialog.Close class="action action-quiet absolute right-4 top-4 size-10 p-0" aria-label={i18n.t.library.closeDetailAria}>
        <X size={20} />
      </Dialog.Close>
      {#if query.isPending}
        <div aria-busy="true">
          <span class="sr-only" role="status">{i18n.t.common.loading}</span>
          <div class="mt-6 grid min-h-48 place-items-center rounded-control bg-canvas p-4" aria-hidden="true">
            <Skeleton class="h-64 w-full max-w-md rounded-control" />
          </div>
          <div class="mt-6 grid gap-3" aria-hidden="true">
            {#each Array.from({ length: 9 }) as _, index (index)}
              <div class="grid grid-cols-[5rem_minmax(0,1fr)] gap-x-4">
                <Skeleton class="h-5 w-16 rounded-control" />
                <Skeleton class="h-5 w-2/3 rounded-control" />
              </div>
            {/each}
          </div>
        </div>
      {:else if query.data}
        {@const data = query.data}
        <div class="mt-6 grid min-h-48 place-items-center rounded-control bg-canvas p-4">
          {#if failed}
            <div class="grid justify-items-center gap-3 text-secondary">
              <ImageOff size={28} />
              <p>{i18n.t.library.originalFailed}</p>
              <button class="action action-secondary" onclick={() => { attempt += 1; failed = false; loaded = false; }}>
                {i18n.t.library.reloadOriginal}
              </button>
            </div>
          {:else}
            {#if !loaded}
              <Skeleton class="col-start-1 row-start-1 h-64 max-h-[45dvh] w-full max-w-md rounded-control" />
            {/if}
            <img
              src={`${originalUrl(data.sticker.id)}${attempt ? `?retry=${attempt}` : ''}`}
              alt={data.sticker.title}
              class={`col-start-1 row-start-1 max-h-[45dvh] max-w-full object-contain transition-opacity duration-180 ease-memedock ${loaded ? 'opacity-100' : 'opacity-0'}`}
              onload={() => { loaded = true; }}
              onerror={() => { failed = true; }}
            />
          {/if}
        </div>
        {#if data.sticker.deleted}
          <p class="mt-2 text-caption text-danger">{i18n.t.library.deletedWarning}</p>
        {/if}
        <dl class="mt-6 grid grid-cols-[5rem_minmax(0,1fr)] gap-x-4 gap-y-3 text-body">
          <dt class="text-secondary">{i18n.t.library.fieldCollection}</dt>
          <dd>{data.collection?.name ?? i18n.t.library.collectionNone}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldTags}</dt>
          <dd class="flex flex-wrap gap-2">
            {#each data.tags as tag (tag.id)}
              <span class="rounded-badge bg-input px-2 py-0.5 text-caption">{tag.name}</span>
            {:else}
              <span class="text-secondary">{i18n.t.library.tagsNone}</span>
            {/each}
          </dd>
          <dt class="text-secondary">{i18n.t.library.fieldOriginalFile}</dt>
          <dd class="break-all">{data.sticker.originalName}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldFormat}</dt>
          <dd>{data.sticker.asset.mime}{data.sticker.asset.animated ? i18n.t.library.formatAnimatedSuffix : ''}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldDimensions}</dt>
          <dd>{data.sticker.asset.width} × {data.sticker.asset.height}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldSize}</dt>
          <dd>{formatBytes(data.sticker.asset.byteSize)}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldCreatedAt}</dt>
          <dd>{formatTime(data.sticker.createdAt)}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldUpdatedAt}</dt>
          <dd>{formatTime(data.sticker.updatedAt)}</dd>
          <dt class="text-secondary">{i18n.t.library.fieldId}</dt>
          <dd class="break-all font-mono text-caption text-secondary">{data.sticker.id}</dd>
        </dl>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
