import { zh, en, type Translations } from './translations.ts';

export type LanguagePreference = 'system' | 'zh' | 'en';
export type ActiveLocale = 'zh' | 'en';

const storageKey = 'memedock.language';
const translations: Record<ActiveLocale, Translations> = { zh, en };

class I18nController {
  preference = $state<LanguagePreference>('system');
  locale = $state<ActiveLocale>('zh');
  private listener: (() => void) | null = null;

  initialize() {
    try {
      const saved = localStorage.getItem(storageKey);
      if (saved === 'system' || saved === 'zh' || saved === 'en') {
        this.preference = saved;
      }
    } catch {
      /* Storage unavailable fallback */
    }

    const onLanguageChange = () => {
      if (this.preference === 'system') {
        this.apply();
      }
    };

    if (typeof window !== 'undefined') {
      window.addEventListener('languagechange', onLanguageChange);
      this.listener = onLanguageChange;
    }

    this.apply();

    return () => {
      if (typeof window !== 'undefined' && this.listener) {
        window.removeEventListener('languagechange', this.listener);
        this.listener = null;
      }
    };
  }

  set(preference: LanguagePreference) {
    this.preference = preference;
    try {
      localStorage.setItem(storageKey, preference);
    } catch {
      /* Keep in-memory preference */
    }
    this.apply();
  }

  private resolveSystemLocale(): ActiveLocale {
    if (typeof navigator === 'undefined') return 'zh';
    const lang = (navigator.language || '').toLowerCase();
    return lang.startsWith('zh') ? 'zh' : 'en';
  }

  private apply() {
    this.locale = this.preference === 'system' ? this.resolveSystemLocale() : this.preference;
    if (typeof document !== 'undefined') {
      document.documentElement.lang = this.locale === 'zh' ? 'zh-CN' : 'en';
    }
  }

  get t(): Translations {
    return translations[this.locale];
  }
}

export const i18n = new I18nController();
