<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import type { Snippet } from 'svelte';
  import { adminQuery } from '../queries.ts';
  import { ApiError, errorMessage } from '#lib/shared/clients/api.ts';
  import AppShell from '#lib/ui/layout/AppShell.svelte';
  import Sidebar from '#lib/ui/layout/Sidebar.svelte';
  import AccountMenu from './AccountMenu.svelte';
  import Button from '#lib/ui/primitives/Button.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { children }: { children: Snippet } = $props();
  const client = useQueryClient();
  const admin = createQuery(() => adminQuery());
  let redirecting = false;

  $effect(() => {
    if (admin.error instanceof ApiError && admin.error.status === 401 && !redirecting) {
      redirecting = true;
      void client.cancelQueries().then(() => {
        client.removeQueries({ predicate: query => query.queryKey[0] !== 'auth' });
        client.getMutationCache().clear();
        return goto('/login', { replace: true });
      });
    }
  });
</script>

{#if admin.isError}
  <div class="mx-auto max-w-lg p-8">
    <p role="alert" class="mb-4 text-danger">{errorMessage(admin.error)}</p>
    <Button onclick={() => admin.refetch()}>{i18n.t.common.retry}</Button>
  </div>
{:else if admin.data}
  <AppShell>
    {#snippet navigation()}
      <Sidebar pathname={page.url.pathname}>
        {#snippet account()}<AccountMenu account={admin.data!} />{/snippet}
      </Sidebar>
    {/snippet}
    {@render children()}
  </AppShell>
{:else}
  <p role="status" class="p-8 text-secondary">{i18n.t.auth.checkingStatus}</p>
{/if}
