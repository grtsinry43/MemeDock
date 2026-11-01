import { Moon, Sun } from 'lucide-react'
import { useI18n } from '../../lib/i18n'
import { useTheme } from '../../lib/theme'
import { cn } from '../../lib/cn'

export function LanguageToggle({ className }: { className?: string }) {
  const { locale, setLocale, t } = useI18n()

  return (
    <div className={cn('lang-toggle', className)} role="group" aria-label={t.nav.language}>
      <button
        type="button"
        lang="zh-CN"
        aria-pressed={locale === 'zh'}
        onClick={() => setLocale('zh')}
      >
        {t.nav.languageZh}
      </button>
      <button type="button" lang="en" aria-pressed={locale === 'en'} onClick={() => setLocale('en')}>
        {t.nav.languageEn}
      </button>
    </div>
  )
}

export function ThemeToggle({ className }: { className?: string }) {
  const { resolvedTheme, toggleTheme } = useTheme()
  const { t } = useI18n()
  const label = resolvedTheme === 'dark' ? t.nav.themeLight : t.nav.themeDark

  return (
    <button
      type="button"
      className={cn('icon-btn', className)}
      onClick={toggleTheme}
      aria-label={label}
      title={label}
    >
      {resolvedTheme === 'dark' ? (
        <Sun size={17} aria-hidden="true" />
      ) : (
        <Moon size={17} aria-hidden="true" />
      )}
    </button>
  )
}
