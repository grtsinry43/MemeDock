<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import type { Snippet } from 'svelte';
  import { LoaderCircle } from '@lucide/svelte';

  let { variant = 'primary', busy = false, disabled = false, type = 'button', class: className = '', children, ...rest }:
    HTMLButtonAttributes & { variant?: 'primary' | 'secondary' | 'quiet' | 'danger'; busy?: boolean; children?: Snippet } = $props();
  const variants = { primary: 'action-primary', secondary: 'action-secondary', quiet: 'action-quiet', danger: 'action-danger' };
</script>

<button {...rest} {type} disabled={disabled || busy} aria-busy={busy || undefined} class={`action ${variants[variant]} ${className}`}>
  {#if busy}<LoaderCircle size={16} class="animate-spin" aria-hidden="true" />{/if}
  {@render children?.()}
</button>
