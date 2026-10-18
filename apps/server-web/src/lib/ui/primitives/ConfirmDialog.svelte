<script lang="ts">
  import { AlertDialog } from 'bits-ui';
  import Button from './Button.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let {
    open = $bindable(false),
    title,
    description,
    busy = false,
    error = '',
    cancelLabel,
    confirmLabel,
    onconfirm,
  }: {
    open?: boolean;
    title: string;
    description: string;
    busy?: boolean;
    error?: string;
    cancelLabel?: string;
    confirmLabel?: string;
    onconfirm: () => void;
  } = $props();
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <AlertDialog.Content class="dialog-panel" escapeKeydownBehavior={busy ? 'ignore' : 'close'}>
      <AlertDialog.Title class="text-section font-semibold">{title}</AlertDialog.Title>
      <AlertDialog.Description class="mt-3 leading-6 text-secondary">{description}</AlertDialog.Description>
      {#if error}<p role="alert" class="mt-4 break-words text-danger">{error}</p>{/if}
      <div class="mt-6 flex justify-end gap-2">
        <AlertDialog.Cancel disabled={busy} class="action action-secondary">
          {cancelLabel ?? i18n.t.common.cancel}
        </AlertDialog.Cancel>
        <Button variant="danger" {busy} onclick={onconfirm}>
          {confirmLabel ?? i18n.t.devices.confirmRevoke}
        </Button>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
