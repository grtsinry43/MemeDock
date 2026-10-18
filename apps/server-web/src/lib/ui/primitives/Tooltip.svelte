<script lang="ts">
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';
  let { label, children }: { label: string; children: Snippet } = $props();
</script>

<Tooltip.Root>
  <Tooltip.Trigger class="action action-quiet size-11 p-0 md:size-9" aria-label={label}>{@render children()}</Tooltip.Trigger>
  <Tooltip.Portal><Tooltip.Content sideOffset={6} class="z-60 rounded-control bg-foreground px-3 py-2 text-caption text-on-inverse shadow-sm">{label}</Tooltip.Content></Tooltip.Portal>
</Tooltip.Root>
