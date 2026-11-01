import { useEffect, useRef, useState } from 'react'
import { AnimatePresence, useReducedMotion } from 'motion/react'
import * as m from 'motion/react-m'
import { ArrowUp, ArrowUpRight, Download } from 'lucide-react'
import { useI18n } from '../../lib/i18n'
import {
  AUTHOR_URL,
  BLOG_URL,
  ISSUES_URL,
  LICENSE_URL,
  RELEASES_URL,
  REPO_URL,
  SERVER_SOURCE_URL,
} from '../../lib/github'
import { Logo } from '../brand/Logo'
import { Mascot } from '../brand/Mascot'
import { LanguageToggle, ThemeToggle } from './Toggles'

const EASE = [0.16, 1, 0.3, 1] as const

export function Footer() {
  const { t } = useI18n()
  const f = t.footer
  const reduce = useReducedMotion()
  const [found, setFound] = useState(false)
  const taps = useRef<number[]>([])
  const hideTimer = useRef<number | null>(null)
  const scrollTimer = useRef<number | null>(null)

  useEffect(() => {
    return () => {
      if (hideTimer.current) window.clearTimeout(hideTimer.current)
      if (scrollTimer.current) window.clearTimeout(scrollTimer.current)
    }
  }, [])

  const handleLogoTap = () => {
    const now = Date.now()
    taps.current = [...taps.current.filter((at) => now - at < 2600), now]
    if (scrollTimer.current) {
      window.clearTimeout(scrollTimer.current)
      scrollTimer.current = null
    }

    if (taps.current.length >= 7) {
      taps.current = []
      setFound(true)
      if (hideTimer.current) window.clearTimeout(hideTimer.current)
      hideTimer.current = window.setTimeout(() => setFound(false), 4200)
      return
    }

    scrollTimer.current = window.setTimeout(() => {
      taps.current = []
      window.scrollTo({ top: 0, behavior: reduce ? 'auto' : 'smooth' })
    }, 360)
  }

  const productLinks = [
    { label: f.links.collect, href: '#collect' },
    { label: f.links.sync, href: '#sync' },
    { label: f.links.native, href: '#native' },
    { label: f.links.download, href: '#download' },
  ]

  const resourceLinks = [
    { label: f.links.github, href: REPO_URL },
    { label: f.links.releases, href: RELEASES_URL },
    { label: f.links.license, href: LICENSE_URL },
    { label: f.links.server, href: SERVER_SOURCE_URL },
  ]

  return (
    <footer className="relative overflow-hidden border-t border-line pt-16 md:pt-24">
      <div className="shell">
        <div className="grid gap-12 md:grid-cols-12 md:gap-8">
          <div className="md:col-span-6">
            <button
              type="button"
              onClick={handleLogoTap}
              className="rounded-full text-left transition-opacity hover:opacity-75"
              aria-label={f.backToTop}
            >
              <Logo size={30} />
            </button>
            <h2 className="display mt-8 whitespace-pre-line text-[clamp(2.2rem,4.2vw,3.8rem)]">
              {f.statement}
            </h2>
            <a href="#download" className="btn btn-primary mt-6">
              <Download size={15} aria-hidden="true" />
              {t.hero.primary}
            </a>
          </div>

          <div className="grid grid-cols-2 gap-8 sm:grid-cols-3 md:col-span-5 md:col-start-8">
            <FooterColumn title={f.product} links={productLinks} />
            <FooterColumn title={f.resources} links={resourceLinks} external />
            <div>
              <h3 className="mono">{f.about}</h3>
              <ul className="mt-4 space-y-2.5">
                <li>
                  <a
                    href={AUTHOR_URL}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 text-[0.8rem] text-ink-soft transition-colors hover:text-ink"
                  >
                    grtsinry43
                    <ArrowUpRight size={11} aria-hidden="true" />
                  </a>
                </li>
                <li>
                  <a
                    href={BLOG_URL}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 text-[0.8rem] text-ink-soft transition-colors hover:text-ink"
                  >
                    {f.links.blog}
                    <ArrowUpRight size={11} aria-hidden="true" />
                  </a>
                </li>
                <li>
                  <a
                    href={ISSUES_URL}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1 text-[0.8rem] text-ink-soft transition-colors hover:text-ink"
                  >
                    {f.links.issues}
                    <ArrowUpRight size={11} aria-hidden="true" />
                  </a>
                </li>
              </ul>
              <div className="mt-5 flex flex-col items-start gap-2.5">
                <LanguageToggle />
                <ThemeToggle className="-ml-2" />
              </div>
            </div>
          </div>
        </div>

        <div className="mt-16 flex items-center justify-between gap-3 border-t border-line py-5 text-[0.66rem] text-muted">
          <span>
            © 2026{' '}
            <a
              href={AUTHOR_URL}
              target="_blank"
              rel="noreferrer"
              className="font-medium text-ink-soft transition-colors hover:text-ink"
            >
              grtsinry43
            </a>{' '}
            · MemeDock · MIT License
          </span>
          <a href="#top" className="inline-flex items-center gap-1.5 transition-colors hover:text-ink">
            {f.backToTop}
            <ArrowUp size={12} aria-hidden="true" />
          </a>
        </div>
      </div>

      <div className="mt-10 overflow-hidden md:mt-16" aria-hidden="true">
        <div className="footer-wordmark px-4 md:px-8">
          MemeDock<span className="text-blue">.</span>
        </div>
      </div>

      <AnimatePresence>
        {found ? (
          <m.div
            className="fixed bottom-[clamp(18px,4vw,36px)] left-1/2 z-80 w-[min(92vw,420px)]"
            role="status"
            aria-live="polite"
            initial={{ opacity: 0, y: 18, x: '-50%', scale: 0.98 }}
            animate={{ opacity: 1, y: 0, x: '-50%', scale: 1 }}
            exit={{ opacity: 0, y: 14, x: '-50%', scale: 0.98 }}
            transition={{ duration: 0.35, ease: EASE }}
          >
            <div className="toast-card flex items-center gap-3">
              <Mascot size={42} />
              <div className="min-w-0">
                <p className="font-brand text-[0.82rem] font-semibold">{f.egg}</p>
                <p className="mono mt-0.5">{f.eggHint}</p>
              </div>
              <span className="ml-auto font-mono text-[0.62rem] text-muted">7 / 7</span>
            </div>
          </m.div>
        ) : null}
      </AnimatePresence>
    </footer>
  )
}

function FooterColumn({
  title,
  links,
  external = false,
}: {
  title: string
  links: { label: string; href: string }[]
  external?: boolean
}) {
  return (
    <div>
      <h3 className="mono">{title}</h3>
      <ul className="mt-4 space-y-2.5">
        {links.map((link) => (
          <li key={link.href}>
            <a
              href={link.href}
              {...(external ? { target: '_blank', rel: 'noreferrer' } : {})}
              className="inline-flex items-center gap-1 text-[0.8rem] text-ink-soft transition-colors hover:text-ink"
            >
              {link.label}
              {external ? <ArrowUpRight size={11} aria-hidden="true" /> : null}
            </a>
          </li>
        ))}
      </ul>
    </div>
  )
}
