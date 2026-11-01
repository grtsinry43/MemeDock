<script lang="ts">
  import Skeleton from '#lib/ui/common/Skeleton.svelte';
</script>

<ol aria-hidden="true" class="divide-y divide-separator">
  {#each Array.from({ length: 6 }) as _, index (index)}
    <li class="grid min-w-0 gap-3 py-5 sm:grid-cols-[minmax(0,1fr)_auto]">
      <div class="min-w-0">
        <Skeleton class="h-[22px] w-36 max-w-full rounded-control" />
        <Skeleton class="mt-1 h-4 w-28 max-w-full rounded-control" />
      </div>
      <Skeleton class="h-4 w-28 rounded-control sm:justify-self-end" />
    </li>
  {/each}
</ol>
