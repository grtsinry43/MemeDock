import { queryOptions } from '@tanstack/svelte-query';
import { getVersion } from './api.ts';

export const versionKey = ['meta', 'version'] as const;
export const versionQuery = () =>
  queryOptions({
    queryKey: versionKey,
    queryFn: ({ signal }) => getVersion(signal),
    staleTime: Infinity,
    retry: 1,
  });
