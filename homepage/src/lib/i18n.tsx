import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { en } from '../content/en'
import { zh, type SiteContent } from '../content/zh'
import { LOCALE_KEY, localeFromDocument, localeLang, type Locale } from './locale'

type I18nValue = {
  locale: Locale
  setLocale: (locale: Locale) => void
  toggleLocale: () => void
  t: SiteContent
}

const I18nContext = createContext<I18nValue | null>(null)

const dictionaries: Record<Locale, SiteContent> = { zh, en }

function storeLocale(locale: Locale) {
  try {
    window.localStorage.setItem(LOCALE_KEY, locale)
  } catch {
    // Storage can be unavailable in private windows. The switch still works for this visit.
  }
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(localeFromDocument)
  const t = dictionaries[locale]

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next)
    storeLocale(next)
  }, [])

  const toggleLocale = useCallback(() => {
    setLocale(locale === 'zh' ? 'en' : 'zh')
  }, [locale, setLocale])

  useEffect(() => {
    const html = document.documentElement
    html.lang = localeLang(locale)
    document.title = t.meta.title
    const description = document.querySelector('meta[name="description"]')
    description?.setAttribute('content', t.meta.description)
  }, [locale, t])

  const value = useMemo(
    () => ({ locale, setLocale, toggleLocale, t }),
    [locale, setLocale, toggleLocale, t],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n() {
  const value = useContext(I18nContext)
  if (!value) throw new Error('useI18n must be used inside I18nProvider')
  return value
}
