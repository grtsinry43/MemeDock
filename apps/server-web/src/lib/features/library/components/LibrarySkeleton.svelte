<script lang="ts">
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
</script>

<div aria-hidden="true">
  <Skeleton class="mb-4 h-4 w-28 rounded-control" />
  <div class="grid grid-cols-2 gap-x-4 gap-y-5 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
    {#each Array.from({ length: 10 }) as _, index (index)}
      <div class="min-w-0">
        <Skeleton class="aspect-square w-full rounded-control" />
        <Skeleton class="mt-2 h-4 w-2/3 rounded-control" />
      </div>
    {/each}
  </div>
</div>
