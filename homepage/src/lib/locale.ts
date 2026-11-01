export type Locale = 'zh' | 'en'

export const LOCALE_KEY = 'memedock-locale'

export function localeFromDocument(): Locale {
  return document.documentElement.lang.toLowerCase().startsWith('en') ? 'en' : 'zh'
}

export function localeLang(locale: Locale): string {
  return locale === 'en' ? 'en' : 'zh-CN'
}
