import { queryOptions } from '@tanstack/svelte-query';
import { getOverview } from './api.ts';
export const overviewQuery = () => queryOptions({ queryKey: ['sync', 'overview'], queryFn: ({ signal }) => getOverview(signal), refetchInterval: 5_000 });
