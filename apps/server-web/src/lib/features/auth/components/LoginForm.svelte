<script lang="ts">
  import { createMutation, useQueryClient } from '@tanstack/svelte-query';
  import { goto } from '$app/navigation';
  import Input from '#lib/ui/primitives/Input.svelte';
  import Button from '#lib/ui/primitives/Button.svelte';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import { login } from '../api.ts';
  import { adminKey } from '../queries.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let username = $state('');
  let password = $state('');
  const client = useQueryClient();
  const mutation = createMutation(() => ({
    mutationFn: () => login({ username: username.trim(), password }),
    onSuccess: async account => {
      password = '';
      await client.cancelQueries();
      client.clear();
      client.setQueryData(adminKey, account);
      await goto('/library', { replace: true });
    },
  }));
</script>

<form
  class="grid gap-5"
  onsubmit={event => {
    event.preventDefault();
    if (!mutation.isPending) mutation.mutate();
  }}
>
  <label class="grid gap-2 text-body">
    {i18n.t.login.usernameLabel}
    <Input bind:value={username} autocomplete="username" required maxlength={128} />
  </label>
  <label class="grid gap-2 text-body">
    {i18n.t.login.passwordLabel}
    <Input
      type="password"
      bind:value={password}
      autocomplete="current-password"
      required
      maxlength={1024}
    />
  </label>
  {#if mutation.isError}
    <p role="alert" class="break-words text-body text-danger">{errorMessage(mutation.error)}</p>
  {/if}
  <Button type="submit" busy={mutation.isPending} class="mt-2 w-full">
    {mutation.isPending ? i18n.t.login.submitting : i18n.t.login.submit}
  </Button>
</form>
