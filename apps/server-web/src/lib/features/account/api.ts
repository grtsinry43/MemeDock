import { getApi } from '#lib/shared/clients/api.ts';
import type { ChangePasswordRequest } from '#lib/generated/api.ts';

export const changePassword = (body: ChangePasswordRequest) => getApi()<null>('/admin/password', { method: 'PUT', body: JSON.stringify(body) });
