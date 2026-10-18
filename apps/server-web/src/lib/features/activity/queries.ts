import { queryOptions } from '@tanstack/svelte-query';
import { getActivity, type ActivityFilters } from './api.ts';
export const activityQuery = (filters: ActivityFilters) => queryOptions({ queryKey: ['sync', 'activity', filters], queryFn: ({ signal }) => getActivity(filters, signal), refetchInterval: filters.page === 1 ? 5_000 : false });
