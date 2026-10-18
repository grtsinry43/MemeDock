<script lang="ts">
  import { createMutation, useQueryClient } from '@tanstack/svelte-query';
  import type { DeviceInfo } from '#lib/generated/api.ts';
  import ConfirmDialog from '#lib/ui/primitives/ConfirmDialog.svelte';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import { revokeDevice } from '../api.ts';
  import { devicesKey } from '../queries.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { device = $bindable(null) }: { device?: DeviceInfo | null } = $props();
  const client = useQueryClient();
  const mutation = createMutation(() => ({
    mutationFn: async () => {
      if (device) await revokeDevice(device.id);
    },
    onSuccess: () => {
      device = null;
      void client.invalidateQueries({ queryKey: devicesKey });
    },
  }));

  $effect(() => {
    if (!device) mutation.reset();
  });
</script>

<ConfirmDialog
  bind:open={() => device !== null, value => { if (!value) device = null; }}
  onconfirm={() => mutation.mutate()}
  title={i18n.t.devices.revokeTitle}
  description={i18n.t.devices.revokeDesc(device?.name ?? '')}
  confirmLabel={i18n.t.devices.confirmRevoke}
  cancelLabel={i18n.t.common.cancel}
  busy={mutation.isPending}
  error={mutation.isError ? errorMessage(mutation.error) : ''}
/>
