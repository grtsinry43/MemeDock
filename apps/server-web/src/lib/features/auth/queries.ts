import { queryOptions } from '@tanstack/svelte-query';
import { getAdmin } from './api.ts';

export const adminKey = ['auth', 'admin'] as const;
export const adminQuery = () => queryOptions({ queryKey: adminKey, queryFn: ({ signal }) => getAdmin(signal), retry: false });
