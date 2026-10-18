<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { X, ImageOff } from '@lucide/svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { detailQuery } from '../queries.ts';
  import { originalUrl } from '../api.ts';
  import { formatBytes, formatTime } from '#lib/shared/format.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { id = $bindable(null) }: { id?: string | null } = $props();
  const query = createQuery(() => detailQuery(id));
  let failed = $state(false);
  let attempt = $state(0);
  $effect(() => {
    if (id) {
      failed = false;
      attempt = 0;
    }
  });
</script>

<Dialog.Root open={id !== null} onOpenChange={open => { if (!open) id = null; }}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content class="dialog-panel max-w-2xl">
      <Dialog.Title class="pr-10 text-section font-semibold">{i18n.t.library.detailTitle}</Dialog.Title>
      <Dialog.Description class="mt-2 text-caption text-secondary">{i18n.t.library.detailDescription}</Dialog.Description>
      <Dialog.Close class="action action-quiet absolute right-4 top-4 size-10 p-0" aria-label={i18n.t.library.closeDetailAria}>
        <X size={20} />
      </Dialog.Close>
      {#if query.data}
        {@const data = query.data}
        <div class="mt-6 flex min-h-48 items-center justify-center rounded-control bg-canvas p-4">
          {#if failed}
            <div class="grid justify-items-center gap-3 text-secondary">
              <ImageOff size={28} />
              <p>{i18n.t.library.originalFailed}</p>
              <button class="action action-secondary" onclick={() => { attempt += 1; failed = false; }}>
                {i18n.t.library.reloadOriginal}
              </button>
            </div>
          {:else}
            <img
              src={`${originalUrl(data.sticker.id)}${attempt ? `?retry=${attempt}` : ''}`}
              alt={data.sticker.title}
              class="max-h-[45dvh] max-w-full object-contain"
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
