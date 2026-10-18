<script lang="ts">
  import { Plus } from '@lucide/svelte';
  import { createMutation, useQueryClient } from '@tanstack/svelte-query';
  import Dialog from '#lib/ui/primitives/Dialog.svelte';
  import Button from '#lib/ui/primitives/Button.svelte';
  import Input from '#lib/ui/primitives/Input.svelte';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import { createDevice } from '../api.ts';
  import { devicesKey } from '../queries.ts';
  import DeviceCredential from './DeviceCredential.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let open = $state(false);
  let name = $state('');
  let token = $state('');
  const client = useQueryClient();
  const mutation = createMutation(() => ({
    // Return no credential: the token belongs only to this mounted dialog.
    mutationFn: async () => {
      const credential = await createDevice({ name: name.trim() });
      token = credential.token;
    },
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: devicesKey });
    },
  }));

  $effect(() => {
    if (!open) {
      name = '';
      token = '';
      mutation.reset();
    }
  });
</script>

<Dialog
  bind:open
  title={token ? i18n.t.devices.createdTitle : i18n.t.devices.createTitle}
  description={token ? i18n.t.devices.credentialDesc : i18n.t.devices.createDesc}
  dismissible={!mutation.isPending}
>
  {#snippet trigger()}
    <Plus size={18} />{i18n.t.devices.addDevice}
  {/snippet}
  {#if token}
    <DeviceCredential {token} onclose={() => { open = false; }} />
  {:else}
    <form
      class="grid gap-5"
      onsubmit={event => {
        event.preventDefault();
        if (!mutation.isPending && name.trim()) mutation.mutate();
      }}
    >
      <label class="grid gap-2 text-body">
        {i18n.t.devices.nameLabel}
        <Input
          bind:value={name}
          placeholder={i18n.t.devices.namePlaceholder}
          required
          maxlength={128}
          disabled={mutation.isPending}
        />
      </label>
      {#if mutation.isError}
        <p role="alert" class="break-words text-body text-danger">{errorMessage(mutation.error)}</p>
      {/if}
      <div class="flex justify-end gap-2">
        <Button
          variant="secondary"
          disabled={mutation.isPending}
          onclick={() => { open = false; }}
        >
          {i18n.t.common.cancel}
        </Button>
        <Button type="submit" busy={mutation.isPending} disabled={!name.trim()}>
          {i18n.t.devices.createSubmit}
        </Button>
      </div>
    </form>
  {/if}
</Dialog>
