import type { SyncOverview } from '#lib/generated/api.ts';
import { getApi } from '#lib/shared/clients/api.ts';
export const getOverview = (signal?: AbortSignal) => getApi()<SyncOverview>('/admin/sync/overview', { signal });
