<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { X } from '@lucide/svelte';
  import type { Snippet } from 'svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { open = $bindable(false), title, description, trigger, children, dismissible = true }:
    { open?: boolean; title: string; description: string; trigger?: Snippet; children: Snippet; dismissible?: boolean } = $props();
</script>

<Dialog.Root bind:open>
  {#if trigger}<Dialog.Trigger class="action action-primary">{@render trigger()}</Dialog.Trigger>{/if}
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content class="dialog-panel" escapeKeydownBehavior={dismissible ? 'close' : 'ignore'} interactOutsideBehavior={dismissible ? 'close' : 'ignore'}>
      <Dialog.Title class="pr-8 text-section font-semibold">{title}</Dialog.Title>
      <Dialog.Description class="mt-2 text-body leading-6 text-secondary">{description}</Dialog.Description>
      <div class="mt-6">{@render children()}</div>
      {#if dismissible}<Dialog.Close class="action action-quiet absolute top-4 right-4 size-9 p-0" aria-label={i18n.t.common.close}><X size={18} /></Dialog.Close>{/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
