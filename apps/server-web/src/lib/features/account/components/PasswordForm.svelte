<script lang="ts">
  import { createMutation, useQueryClient } from '@tanstack/svelte-query';
  import { goto } from '$app/navigation';
  import Button from '#lib/ui/primitives/Button.svelte';
  import Input from '#lib/ui/primitives/Input.svelte';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import { changePassword } from '../api.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let oldPassword = $state('');
  let newPassword = $state('');
  let confirmation = $state('');
  let validation = $state('');
  const client = useQueryClient();
  const mutation = createMutation(() => ({
    mutationFn: () => changePassword({ oldPassword, newPassword }),
    onSuccess: async () => {
      oldPassword = '';
      newPassword = '';
      confirmation = '';
      await client.cancelQueries();
      client.clear();
      await goto('/login?passwordChanged=1');
    },
  }));

  function submit(event: SubmitEvent) {
    event.preventDefault();
    validation = '';
    if (mutation.isPending) return;
    const length = new TextEncoder().encode(newPassword).length;
    if (length < 12 || length > 1024) {
      validation = i18n.t.settings.passwordValidationLength;
      return;
    }
    if (newPassword !== confirmation) {
      validation = i18n.t.settings.passwordValidationMismatch;
      return;
    }
    mutation.mutate();
  }
</script>

<section class="panel p-6">
  <h2 class="text-section font-semibold">{i18n.t.settings.passwordTitle}</h2>
  <p class="mt-2 text-body leading-6 text-secondary">{i18n.t.settings.passwordDesc}</p>
  <form class="mt-6 grid max-w-md gap-5" onsubmit={submit}>
    <label class="grid gap-2 text-body">
      {i18n.t.settings.currentPasswordLabel}
      <Input
        type="password"
        bind:value={oldPassword}
        autocomplete="current-password"
        required
        maxlength={1024}
        disabled={mutation.isPending}
      />
    </label>
    <div class="grid gap-2">
      <label class="grid gap-2 text-body">
        {i18n.t.settings.newPasswordLabel}
        <Input
          type="password"
          bind:value={newPassword}
          autocomplete="new-password"
          required
          maxlength={1024}
          disabled={mutation.isPending}
          aria-describedby="new-password-help"
        />
      </label>
      <p id="new-password-help" class="text-caption text-secondary">{i18n.t.settings.newPasswordHint}</p>
    </div>
    <label class="grid gap-2 text-body">
      {i18n.t.settings.confirmPasswordLabel}
      <Input
        type="password"
        bind:value={confirmation}
        autocomplete="new-password"
        required
        maxlength={1024}
        disabled={mutation.isPending}
      />
    </label>
    {#if validation || mutation.isError}
      <p role="alert" class="break-words text-body text-danger">
        {validation || errorMessage(mutation.error)}
      </p>
    {/if}
    <div>
      <Button type="submit" busy={mutation.isPending}>{i18n.t.settings.savePasswordSubmit}</Button>
    </div>
  </form>
</section>
