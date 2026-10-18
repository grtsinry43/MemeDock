import { getApi } from '#lib/shared/clients/api.ts';
import type { ActivityPage, ActivityQuery } from '#lib/generated/api.ts';
export type ActivityFilters = Required<Pick<ActivityQuery, 'page'>> & Partial<Pick<ActivityQuery, 'deviceId' | 'result'>>;
export const getActivity = (filters: ActivityFilters, signal?: AbortSignal) => {
  const params = new URLSearchParams({ page: String(filters.page) });
  if (filters.deviceId) params.set('deviceId', filters.deviceId);
  if (filters.result) params.set('result', filters.result);
  return getApi()<ActivityPage>(`/admin/sync/operations?${params}`, { signal });
};
