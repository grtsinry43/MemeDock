<script lang="ts">
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
</script>

<div aria-hidden="true">
  <div class="grid grid-cols-2 gap-x-6 gap-y-7 border-b border-separator pb-8 sm:grid-cols-4">
    {#each Array.from({ length: 4 }) as _, index (index)}
      <div>
        <Skeleton class="h-4 w-20 rounded-control" />
        <Skeleton class="mt-2 h-[34px] w-24 rounded-control" />
      </div>
    {/each}
  </div>
  <div class="mt-4 flex flex-wrap justify-between gap-2">
    <Skeleton class="h-4 w-32 rounded-control" />
    <Skeleton class="h-4 w-32 rounded-control" />
  </div>
  <section class="mt-10">
    <Skeleton class="mb-2 h-7 w-36 rounded-control" />
    {#each Array.from({ length: 3 }) as _, index (index)}
      <div
        class="grid gap-4 border-b border-separator py-5 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(12rem,0.8fr)] sm:items-center"
      >
        <div class="flex min-w-0 gap-3">
          <Skeleton class="size-10 shrink-0 rounded-control" />
          <div class="min-w-0 flex-1">
            <Skeleton class="h-[22px] w-32 max-w-full rounded-control" />
            <Skeleton class="mt-1 h-4 w-40 max-w-full rounded-control" />
          </div>
        </div>
        <div class="min-w-0">
          <Skeleton class="h-4 w-28 rounded-control" />
          <Skeleton class="mt-2 h-1.5 w-full rounded-full" />
          <Skeleton class="mt-1 h-3 w-24 rounded-control" />
        </div>
      </div>
    {/each}
  </section>
</div>
