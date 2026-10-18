<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import { UserRound, ChevronsUpDown, LogOut } from '@lucide/svelte';
  import { createMutation, useQueryClient } from '@tanstack/svelte-query';
  import { goto } from '$app/navigation';
  import type { AdminInfo } from '#lib/generated/api.ts';
  import Menu from '#lib/ui/primitives/DropdownMenu.svelte';
  import { logout } from '../api.ts';
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { account }: { account: AdminInfo } = $props();
  const client = useQueryClient();
  const mutation = createMutation(() => ({
    mutationFn: logout,
    onSuccess: async () => {
      await client.cancelQueries();
      client.clear();
      await goto('/login');
    },
  }));
</script>

<Menu label={i18n.t.nav.accountMenuLabel}>
  {#snippet trigger()}
    <UserRound size={18} />
    <span class="min-w-0 flex-1 truncate text-left">{account.username}</span>
    <ChevronsUpDown size={14} />
  {/snippet}
  <DropdownMenu.Item class="menu-item" disabled={mutation.isPending} onclick={() => mutation.mutate()}>
    <LogOut size={16} />
    {i18n.t.nav.signOut}
  </DropdownMenu.Item>
</Menu>
{#if mutation.isError}
  <p role="alert" class="mt-2 break-words text-caption text-danger">{errorMessage(mutation.error)}</p>
{/if}
