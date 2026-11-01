import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'

export type Theme = 'light' | 'dark' | 'system'
export type ResolvedTheme = 'light' | 'dark'

const THEME_KEY = 'memedock-theme'
const THEME_COLOR = { light: '#FBFBFC', dark: '#0F1115' } as const

type ThemeValue = {
  theme: Theme
  resolvedTheme: ResolvedTheme
  setTheme: (theme: Theme) => void
  toggleTheme: () => void
}

const ThemeContext = createContext<ThemeValue | null>(null)

function readTheme(): Theme {
  try {
    const stored = window.localStorage.getItem(THEME_KEY)
    if (stored === 'light' || stored === 'dark' || stored === 'system') return stored
  } catch {
    // Fall through to the system preference.
  }
  return 'system'
}

function prefersDark() {
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

function storeTheme(theme: Theme) {
  try {
    window.localStorage.setItem(THEME_KEY, theme)
  } catch {
    // The theme still applies for this visit.
  }
}

function paint(resolved: ResolvedTheme) {
  const html = document.documentElement
  html.classList.toggle('dark', resolved === 'dark')
  html.style.colorScheme = resolved
  const meta = document.getElementById('theme-color')
  meta?.setAttribute('content', THEME_COLOR[resolved])
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readTheme)
  const [systemDark, setSystemDark] = useState(prefersDark)

  const resolvedTheme: ResolvedTheme =
    theme === 'system' ? (systemDark ? 'dark' : 'light') : theme

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches)
    media.addEventListener('change', onChange)
    return () => media.removeEventListener('change', onChange)
  }, [])

  useEffect(() => {
    paint(resolvedTheme)
  }, [resolvedTheme])

  const setTheme = useCallback((next: Theme) => {
    setThemeState(next)
    storeTheme(next)
  }, [])

  const toggleTheme = useCallback(() => {
    setTheme(resolvedTheme === 'dark' ? 'light' : 'dark')
  }, [resolvedTheme, setTheme])

  const value = useMemo(
    () => ({ theme, resolvedTheme, setTheme, toggleTheme }),
    [theme, resolvedTheme, setTheme, toggleTheme],
  )

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
}

export function useTheme() {
  const value = useContext(ThemeContext)
  if (!value) throw new Error('useTheme must be used inside ThemeProvider')
  return value
}
