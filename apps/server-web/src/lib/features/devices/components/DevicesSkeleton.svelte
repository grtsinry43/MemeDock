<script lang="ts">
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
</script>

<div class="panel divide-y divide-separator" aria-hidden="true">
  {#each Array.from({ length: 3 }) as _, index (index)}
    <div class="flex items-center gap-4 px-5 py-5 sm:px-6">
      <Skeleton class="size-11 shrink-0 rounded-control" />
      <div class="min-w-0 flex-1">
        <Skeleton class="h-[22px] w-40 max-w-full rounded-control" />
        <Skeleton class="mt-2 h-4 w-56 max-w-full rounded-control" />
      </div>
      <Skeleton class="size-5 shrink-0 rounded-control" />
    </div>
  {/each}
</div>
