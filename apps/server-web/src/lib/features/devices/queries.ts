import { queryOptions } from '@tanstack/svelte-query';
import { getDevices } from './api.ts';

export const devicesKey = ['devices'] as const;
export const devicesQuery = () => queryOptions({ queryKey: devicesKey, queryFn: ({ signal }) => getDevices(signal) });
