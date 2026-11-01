import { useEffect, useState } from 'react'
import { Download, Menu } from 'lucide-react'
import { useI18n } from '../../lib/i18n'
import { REPO_URL } from '../../lib/github'
import { useActiveSection } from '../../lib/useActiveSection'
import { useStarCount } from '../../lib/useGithub'
import { cn } from '../../lib/cn'
import { Logo } from '../brand/Logo'
import { GithubIcon } from '../brand/GithubIcon'
import { MobileMenu } from './MobileMenu'
import { LanguageToggle, ThemeToggle } from './Toggles'

const SECTION_IDS = [
  'collect',
  'organize',
  'share',
  'keep',
  'sync',
  'native',
  'faq',
  'download',
] as const

const FEATURE_SECTIONS = ['collect', 'organize', 'share', 'keep']

export function Nav() {
  const { t } = useI18n()
  const [scrolled, setScrolled] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const active = useActiveSection(SECTION_IDS)
  const stars = useStarCount()

  useEffect(() => {
    let frame = 0
    const measure = () => {
      frame = 0
      setScrolled(window.scrollY > 12)
    }
    const onScroll = () => {
      if (frame) return
      frame = window.requestAnimationFrame(measure)
    }
    measure()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      if (frame) window.cancelAnimationFrame(frame)
      window.removeEventListener('scroll', onScroll)
    }
  }, [])

  const isActive = (href: string) => {
    if (href === '#collect') return active !== null && FEATURE_SECTIONS.includes(active)
    return active === href.slice(1)
  }

  const links = [
    { href: '#collect', label: t.nav.collect },
    { href: '#sync', label: t.nav.sync },
    { href: '#native', label: t.nav.native },
    { href: '#download', label: t.nav.download },
  ]

  const starLabel = stars === null ? null : formatStars(stars)

  return (
    <>
      <header className={cn('site-nav', scrolled && 'is-scrolled')}>
        <div className="shell flex h-[var(--nav-height)] items-center gap-5">
          <a href="#top" className="flex shrink-0 items-center" aria-label="MemeDock">
            <Logo size={29} />
          </a>

          <nav className="ml-2 hidden items-center gap-7 lg:flex" aria-label={t.nav.menu}>
            {links.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="nav-link"
                aria-current={isActive(link.href) ? 'true' : undefined}
              >
                {link.label}
              </a>
            ))}
          </nav>

          <div className="ml-auto flex items-center gap-1.5">
            <a
              href={REPO_URL}
              target="_blank"
              rel="noreferrer"
              className="hidden items-center gap-2 rounded-full px-2.5 py-1.5 text-sm text-ink-soft transition-colors hover:text-ink md:flex"
              aria-label={t.nav.github}
            >
              <GithubIcon size={16} />
              <span className="hidden text-[0.8rem] font-medium xl:inline">GitHub</span>
              {starLabel ? <span className="nav-count">{starLabel}</span> : null}
            </a>
            <LanguageToggle className="hidden md:inline-flex" />
            <ThemeToggle className="hidden md:inline-flex" />
            <a href="#download" className="btn btn-primary ml-1 hidden sm:inline-flex">
              <Download size={15} aria-hidden="true" />
              {t.nav.download}
            </a>
            <button
              type="button"
              className="icon-btn lg:hidden"
              onClick={() => setMenuOpen(true)}
              aria-label={t.nav.menu}
              aria-expanded={menuOpen}
            >
              <Menu size={19} aria-hidden="true" />
            </button>
          </div>
        </div>
      </header>
      <MobileMenu open={menuOpen} onClose={() => setMenuOpen(false)} />
    </>
  )
}

function formatStars(value: number) {
  if (value >= 1000) return `${(value / 1000).toFixed(value >= 10000 ? 0 : 1)}k`
  return String(value)
}
