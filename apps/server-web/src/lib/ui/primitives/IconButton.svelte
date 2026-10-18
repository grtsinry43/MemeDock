<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  let { label, children, class: className = '', type = 'button', ...rest }:
    HTMLButtonAttributes & { label: string; children: Snippet } = $props();
</script>

<button {...rest} {type} aria-label={label} title={label} class={`action action-quiet size-11 shrink-0 p-0 md:size-9 ${className}`}>
  {@render children()}
</button>
