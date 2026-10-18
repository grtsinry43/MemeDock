<script lang="ts">
  import { DropdownMenu } from 'bits-ui';
  import type { Snippet } from 'svelte';
  let { label, trigger, children }: { label: string; trigger: Snippet; children: Snippet } = $props();
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger aria-label={label} class="action action-quiet gap-2 px-2">{@render trigger()}</DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content class="popup" sideOffset={6} align="end">{@render children()}</DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
