import { queryOptions } from '@tanstack/svelte-query';
import { getLibrary, getDetail, getFacets, type LibraryFilters } from './api.ts';
export const libraryQuery = (filters: LibraryFilters) => queryOptions({ queryKey: ['library', 'stickers', filters], queryFn: ({ signal }) => getLibrary(filters, signal), refetchInterval: 10_000 });
export const detailQuery = (id: string | null) => queryOptions({ queryKey: ['library', 'detail', id], queryFn: ({ signal }) => getDetail(id!, signal), enabled: id !== null, refetchInterval: 10_000 });
export const facetsQuery = (kind: 'collections' | 'tags', q: string) => queryOptions({ queryKey: ['library', kind, q], queryFn: ({ signal }) => getFacets(kind, q, signal) });
