<script lang="ts">
  import { errorMessage } from '#lib/shared/clients/api.ts';
  import Button from '#lib/ui/primitives/Button.svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let {
    error,
    retry,
  }: {
    error: unknown;
    retry: () => unknown;
  } = $props();
</script>

{#if error}
  <div role="alert" class="mb-5 flex flex-wrap items-center gap-3 text-body text-danger">
    <p class="min-w-0 flex-1 break-words">{errorMessage(error)}</p>
    <Button variant="secondary" onclick={() => retry()}>{i18n.t.common.reload}</Button>
  </div>
{/if}
