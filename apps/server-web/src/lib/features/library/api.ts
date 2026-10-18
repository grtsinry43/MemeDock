import { getApi } from '#lib/shared/clients/api.ts';
import type { LibraryDetail, LibraryFacets, LibraryPage, LibraryQuery } from '#lib/generated/api.ts';

export type LibraryFilters = Required<Pick<LibraryQuery, 'q' | 'starred' | 'deleted' | 'page'>> & Partial<Pick<LibraryQuery, 'collectionId' | 'tagId'>>;
export function getLibrary(filters: LibraryFilters, signal?: AbortSignal) {
  const params = new URLSearchParams({ q: filters.q, starred: String(filters.starred), deleted: String(filters.deleted), page: String(filters.page) });
  if (filters.collectionId) params.set('collectionId', filters.collectionId);
  if (filters.tagId) params.set('tagId', filters.tagId);
  return getApi()<LibraryPage>(`/admin/library/stickers?${params}`, { signal });
}
export const getDetail = (id: string, signal?: AbortSignal) => getApi()<LibraryDetail>(`/admin/library/stickers/${encodeURIComponent(id)}`, { signal });
export const getFacets = (kind: 'collections' | 'tags', q: string, signal?: AbortSignal) => getApi()<LibraryFacets>(`/admin/library/${kind}?${new URLSearchParams({ q })}`, { signal });
export const thumbnailUrl = (id: string) => `/api/v1/admin/assets/${encodeURIComponent(id)}/thumbnail`;
export const originalUrl = (id: string) => `/api/v1/admin/assets/${encodeURIComponent(id)}/original`;
