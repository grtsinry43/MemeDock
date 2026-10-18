export type ThemePreference = 'system' | 'light' | 'dark';
const key = 'memedock.theme';

class ThemeController {
  preference = $state<ThemePreference>('system');
  dark = $state(false);
  private media: MediaQueryList | null = null;

  initialize() {
    this.media = window.matchMedia('(prefers-color-scheme: dark)');
    try {
      const saved = localStorage.getItem(key);
      if (saved === 'light' || saved === 'dark') this.preference = saved;
    } catch { /* Theme remains usable when browser storage is unavailable. */ }
    const update = () => this.apply();
    this.media.addEventListener('change', update);
    this.apply();
    return () => { this.media?.removeEventListener('change', update); this.media = null; };
  }

  set(preference: ThemePreference) {
    this.preference = preference;
    try { localStorage.setItem(key, preference); } catch { /* Keep the in-memory preference. */ }
    this.apply();
  }

  private apply() {
    this.dark = this.preference === 'dark' || (this.preference === 'system' && (this.media?.matches ?? false));
    document.documentElement.dataset.theme = this.dark ? 'dark' : 'light';
  }
}

export const theme = new ThemeController();
