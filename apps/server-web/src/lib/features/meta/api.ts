import { getApi } from '#lib/shared/clients/api.ts';
import type { VersionInfo } from '#lib/generated/api.ts';

export const getVersion = (signal?: AbortSignal) =>
  getApi()<VersionInfo>('/version', { signal });
