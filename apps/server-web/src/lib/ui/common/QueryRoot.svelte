<script lang="ts">
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { Tooltip } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import { createAppQueryClient } from '#lib/shared/clients/query-client.ts';
  let { children }: { children: Snippet } = $props();
  const client = createAppQueryClient();
</script>

<QueryClientProvider {client}><Tooltip.Provider delayDuration={400}>{@render children()}</Tooltip.Provider></QueryClientProvider>
