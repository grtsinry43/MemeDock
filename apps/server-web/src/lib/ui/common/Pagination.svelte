<script lang="ts">
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import IconButton from '#lib/ui/primitives/IconButton.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let {
    page,
    limit,
    total,
    busy = false,
    onchange,
  }: {
    page: number;
    limit: number;
    total: number;
    busy?: boolean;
    onchange: (page: number) => void;
  } = $props();

  const pages = $derived(Math.max(1, Math.ceil(total / limit)));
</script>

{#if total > limit || page > 1}
  <nav
    aria-label={i18n.t.common.paginationLabel}
    class="mt-7 flex items-center justify-between gap-3 border-t border-separator pt-4 text-caption text-secondary"
  >
    <span>{i18n.t.common.itemsCount(total)}</span>
    <div class="flex items-center gap-3">
      <IconButton label={i18n.t.common.previous} disabled={busy || page <= 1} onclick={() => onchange(page - 1)}>
        <ChevronLeft size={18} />
      </IconButton>
      <span aria-live="polite">{i18n.t.common.pageIndicator(page, pages)}</span>
      <IconButton label={i18n.t.common.next} disabled={busy || page >= pages} onclick={() => onchange(page + 1)}>
        <ChevronRight size={18} />
      </IconButton>
    </div>
  </nav>
{/if}
