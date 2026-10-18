import { getApi } from '#lib/shared/clients/api.ts';
import type { CreateDeviceRequest, DeviceCredential, DeviceList } from '#lib/generated/api.ts';

export const getDevices = (signal?: AbortSignal) => getApi()<DeviceList>('/admin/devices', { signal });
export const createDevice = (body: CreateDeviceRequest) => getApi()<DeviceCredential>('/admin/devices', { method: 'POST', body: JSON.stringify(body) });
export const revokeDevice = (id: string) => getApi()<null>(`/admin/devices/${encodeURIComponent(id)}`, { method: 'DELETE' });
