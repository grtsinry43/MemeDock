import { MutationCache, QueryCache, QueryClient } from '@tanstack/svelte-query';
import { ApiError } from './api.ts';

export function createAppQueryClient() {
  const client = new QueryClient({
    mutationCache: new MutationCache({ onError(error) {
      if (error instanceof ApiError && error.status === 401) void client.invalidateQueries({ queryKey: ['auth', 'admin'] });
    } }),
    queryCache: new QueryCache({
      onError(error, query) {
        if (error instanceof ApiError && error.status === 401 && query.queryKey[0] !== 'auth') {
          // Recheck identity; the protected shell owns navigation and cache cleanup.
          void client.invalidateQueries({ queryKey: ['auth', 'admin'] });
        }
      }
    }),
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        gcTime: 300_000,
        retry: (count, error) => count < 1 && error instanceof ApiError && (error.status === 0 || error.status >= 500),
        refetchOnWindowFocus: true
      },
      mutations: { retry: false, gcTime: 0 }
    }
  });
  return client;
}
