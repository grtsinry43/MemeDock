<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  let { value = $bindable(''), class: className = '', ...rest }: HTMLInputAttributes = $props();
</script>

<input {...rest} bind:value class={`field ${className}`} />
