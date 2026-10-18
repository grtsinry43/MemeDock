<script lang="ts">
  import { Copy, Check } from '@lucide/svelte';
  import Button from '#lib/ui/primitives/Button.svelte';
  import { copyText } from '#lib/shared/dom/clipboard.ts';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { token, onclose }: { token: string; onclose: () => void } = $props();
  let copied = $state(false);
  let failed = $state(false);

  async function copy() {
    copied = await copyText(token);
    failed = !copied;
  }
</script>

<p class="mb-4 text-body leading-6 text-secondary">{i18n.t.devices.credentialDesc}</p>
<label class="grid gap-2 text-body">
  {i18n.t.devices.tokenLabel}
  <textarea readonly value={token} rows="3" class="field resize-none break-all p-3 font-mono text-caption"></textarea>
</label>
{#if failed}
  <p role="alert" class="mt-3 text-body text-danger">{i18n.t.devices.copyFailed}</p>
{/if}
<div class="mt-6 flex flex-wrap justify-end gap-2">
  <Button variant="secondary" onclick={copy}>
    {#if copied}
      <Check size={16} />{i18n.t.devices.copied}
    {:else}
      <Copy size={16} />{i18n.t.devices.copyToken}
    {/if}
  </Button>
  <Button onclick={onclose}>{i18n.t.devices.credentialDone}</Button>
</div>
