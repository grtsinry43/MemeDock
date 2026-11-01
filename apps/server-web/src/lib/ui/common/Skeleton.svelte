<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements';

  let { class: className = '', ...rest }: HTMLAttributes<HTMLSpanElement> = $props();
</script>

<span aria-hidden="true" class={`skeleton block ${className}`} {...rest}></span>
