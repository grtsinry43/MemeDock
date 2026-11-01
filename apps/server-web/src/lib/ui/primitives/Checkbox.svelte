<script lang="ts">
  import { Check } from '@lucide/svelte';
  import type { Snippet } from 'svelte';

  let {
    checked = $bindable(false),
    disabled = false,
    onchange,
    children,
  }: {
    checked?: boolean;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    children: Snippet;
  } = $props();
</script>

<label
  class="flex min-h-11 w-fit items-center gap-2.5 text-body select-none has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-40"
>
  <span class="relative size-[18px] shrink-0">
    <input
      type="checkbox"
      bind:checked
      {disabled}
      onchange={event => onchange?.(event.currentTarget.checked)}
      class="peer size-full appearance-none rounded-badge border border-secondary/40 bg-surface transition-[background-color,border-color] duration-180 ease-memedock hover:border-secondary/70 checked:border-accent checked:bg-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed"
    />
    <Check
      size={12}
      strokeWidth={3}
      aria-hidden="true"
      class="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 scale-75 text-on-inverse opacity-0 transition-[opacity,scale] duration-140 ease-memedock peer-checked:scale-100 peer-checked:opacity-100"
    />
  </span>
  {@render children()}
</label>
