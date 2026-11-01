<script lang="ts">
  import { page } from '$app/state';
  import AppShell from './AppShell.svelte';
  import Sidebar from './Sidebar.svelte';
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
</script>

<AppShell>
  {#snippet navigation()}
    <Sidebar pathname={page.url.pathname}>
      {#snippet account()}
        <div class="flex min-h-11 items-center gap-3 px-2 md:min-h-9" aria-hidden="true">
          <Skeleton class="size-8 shrink-0 rounded-full" />
          <Skeleton class="h-3 w-24 rounded-control" />
        </div>
      {/snippet}
    </Sidebar>
  {/snippet}
  <div aria-busy="true">
    <span class="sr-only" role="status">{i18n.t.common.loading}</span>
    <div class="mb-8" aria-hidden="true">
      <Skeleton class="h-[34px] w-40 rounded-control" />
      <Skeleton class="mt-2 h-5 w-full max-w-md rounded-control" />
    </div>
    <div
      class="grid grid-cols-2 gap-x-4 gap-y-5 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5"
      aria-hidden="true"
    >
      {#each Array.from({ length: 10 }) as _, index (index)}
        <div class="min-w-0">
          <Skeleton class="aspect-square w-full rounded-control" />
          <Skeleton class="mt-2 h-4 w-2/3 rounded-control" />
        </div>
      {/each}
    </div>
  </div>
</AppShell>
