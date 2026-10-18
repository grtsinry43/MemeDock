import { i18n } from './i18n/i18n.svelte.ts';

export function formatTime(value: string | number | null): string {
  if (value === null) return i18n.t.common.timeNotRecorded;
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.getTime())) return i18n.t.common.timeUnavailable;
  const locale = i18n.locale === 'zh' ? 'zh-CN' : 'en-US';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(parsed);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}
