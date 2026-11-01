<script lang="ts">
  import { Cat, X } from '@lucide/svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (!open) return;
    timer = setTimeout(() => {
      open = false;
      timer = null;
    }, 4_000);
    return () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    };
  });

  function dismiss() {
    open = false;
  }
</script>

{#if open}
  <div class="pointer-events-none fixed inset-x-0 bottom-5 z-[60] flex justify-center px-4">
    <div
      role="status"
      class="egg-in pointer-events-auto flex max-w-sm items-center gap-3 rounded-message bg-surface p-3.5 shadow-[0_12px_48px_-8px_rgb(0_0_0_/_0.24)] ring-1 ring-separator/60"
    >
      <span class="flex size-10 shrink-0 items-center justify-center rounded-control bg-input text-accent">
        <Cat size={20} />
      </span>
      <div class="min-w-0 flex-1">
        <p class="text-body font-semibold">{i18n.t.egg.title}</p>
        <p class="mt-0.5 text-caption text-secondary">{i18n.t.egg.message}</p>
      </div>
      <button
        type="button"
        class="action action-quiet size-9 shrink-0 p-0"
        aria-label={i18n.t.egg.dismiss}
        onclick={dismiss}
      >
        <X size={16} />
      </button>
    </div>
  </div>
{/if}
