import { getApi } from '#lib/shared/clients/api.ts';
import type { AdminInfo, LoginRequest } from '#lib/generated/api.ts';

export const getAdmin = (signal?: AbortSignal) => getApi()<AdminInfo>('/admin/me', { signal });
export const login = (body: LoginRequest) => getApi()<AdminInfo>('/admin/login', { method: 'POST', body: JSON.stringify(body) });
export const logout = () => getApi()<null>('/admin/logout', { method: 'POST' });
