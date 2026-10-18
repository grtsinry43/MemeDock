import type { ApiEnvelope, ApiMeta } from '#lib/generated/api.ts';

export class ApiError extends Error {
  constructor(message: string, public status: number, public code?: number, public bizErr?: string, public meta?: ApiMeta) {
    super(message);
    this.name = 'ApiError';
  }
}

function isEnvelope(value: unknown): value is ApiEnvelope<unknown> {
  if (!value || typeof value !== 'object') return false;
  return 'code' in value && typeof value.code === 'number'
    && 'bizErr' in value && typeof value.bizErr === 'string'
    && 'msg' in value && typeof value.msg === 'string' && 'data' in value
    && 'meta' in value && !!value.meta && typeof value.meta === 'object'
    && 'requestId' in value.meta && typeof value.meta.requestId === 'string'
    && 'timestamp' in value.meta && typeof value.meta.timestamp === 'string';
}

import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

export function getApi(fetcher: typeof fetch = fetch) {
  return async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const headers = new Headers(options.headers);
    if (options.body) headers.set('Content-Type', 'application/json');
    let response: Response;
    try {
      response = await fetcher(`/api/v1${path}`, { ...options, headers, credentials: 'same-origin', cache: 'no-store' });
    } catch (cause) {
      if (options.signal?.aborted) throw cause;
      throw new ApiError(i18n.t.errors.connectionFailed, 0);
    }
    let payload: unknown;
    try { payload = await response.json(); }
    catch (cause) {
      if (options.signal?.aborted) throw cause;
      throw new ApiError(i18n.t.errors.parseFailed, response.status);
    }
    if (!isEnvelope(payload)) throw new ApiError(i18n.t.errors.incompatibleResponse, response.status);
    if (!response.ok || payload.code !== 0) {
      throw new ApiError(payload.msg, response.status, payload.code, payload.bizErr, payload.meta);
    }
    // The generated DTO describes the trusted server's wire contract.
    return payload.data as T;
  };
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return `${error.message}${error.meta ? ` (${i18n.t.errors.requestId} ${error.meta.requestId})` : ''}`;
  return i18n.t.errors.operationFailed;
}
