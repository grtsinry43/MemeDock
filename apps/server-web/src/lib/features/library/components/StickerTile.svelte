<script lang="ts">
  import { ImageOff, Star } from '@lucide/svelte';
  import type { LibrarySticker } from '#lib/generated/api.ts';
  import { thumbnailUrl } from '../api.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { sticker, onselect }: { sticker: LibrarySticker; onselect: () => void } = $props();
  let failed = $state(false);
  let attempt = $state(0);
</script>

<div class="group min-w-0">
  <button
    type="button"
    aria-label={i18n.t.library.viewStickerAria(sticker.title || sticker.originalName)}
    onclick={onselect}
    class="relative flex aspect-square w-full items-center justify-center overflow-hidden rounded-control bg-transparent p-4 transition-colors hover:bg-input focus-visible:outline-accent"
  >
    {#if failed}
      <span class="grid place-items-center gap-2 text-caption text-secondary">
        <ImageOff size={24} />{i18n.t.library.previewFailed}
      </span>
    {:else}
      <img
        src={`${thumbnailUrl(sticker.id)}${attempt ? `?retry=${attempt}` : ''}`}
        alt=""
        loading="lazy"
        decoding="async"
        class="max-h-full max-w-full object-contain transition-transform duration-180 group-hover:scale-[1.035] motion-reduce:transform-none"
        onerror={() => {
          failed = true;
        }}
      />
    {/if}
    {#if sticker.starred}
      <span class="absolute right-2 top-2 text-accent" aria-label={i18n.t.library.starredBadge}>
        <Star size={14} fill="currentColor" />
      </span>
    {/if}
    {#if sticker.asset.animated}
      <span class="absolute bottom-2 left-2 rounded-badge bg-canvas/90 px-1.5 py-0.5 text-[10px] text-secondary">
        {i18n.t.library.animatedBadge}
      </span>
    {/if}
  </button>
  <p
    class="mt-2 truncate px-0.5 text-caption text-secondary"
    title={sticker.title || sticker.originalName}
  >
    {sticker.title || sticker.originalName}
  </p>
  {#if failed}
    <button
      type="button"
      class="mt-1 text-caption text-accent underline underline-offset-2"
      onclick={() => {
        attempt += 1;
        failed = false;
      }}
    >
      {i18n.t.library.reloadPreview}
    </button>
  {/if}
</div>
