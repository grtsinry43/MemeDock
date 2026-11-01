import { useEffect, useRef } from 'react'
import { ArrowUpRight, Download, X } from 'lucide-react'
import { useI18n } from '../../lib/i18n'
import { REPO_URL } from '../../lib/github'
import { Logo } from '../brand/Logo'
import { GithubIcon } from '../brand/GithubIcon'
import { LanguageToggle, ThemeToggle } from './Toggles'

type MobileMenuProps = {
  open: boolean
  onClose: () => void
}

export function MobileMenu({ open, onClose }: MobileMenuProps) {
  const { t } = useI18n()
  const ref = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    if (open) {
      if (!dialog.open) dialog.showModal()
      document.body.style.overflow = 'hidden'
    }
    if (!open && dialog.open) dialog.close()
    return () => {
      document.body.style.overflow = ''
    }
  }, [open])

  useEffect(() => {
    return () => {
      document.body.style.overflow = ''
    }
  }, [])

  const links = [
    { href: '#collect', label: t.nav.collect, index: '01' },
    { href: '#sync', label: t.nav.sync, index: '02' },
    { href: '#native', label: t.nav.native, index: '03' },
    { href: '#download', label: t.nav.download, index: '04' },
  ]

  return (
    <dialog
      ref={ref}
      className="mobile-dialog"
      onClose={onClose}
      aria-labelledby="mobile-menu-title"
    >
      <div className="mobile-dialog-content flex h-full flex-col">
        <div className="flex items-center justify-between">
          <Logo size={28} />
          <button type="button" className="icon-btn" onClick={onClose} aria-label={t.nav.close}>
            <X size={20} aria-hidden="true" />
          </button>
        </div>
        <h2 id="mobile-menu-title" className="sr-only">
          {t.nav.menu}
        </h2>

        <nav className="mt-12 flex flex-col" aria-label={t.nav.menu}>
          {links.map((link) => (
            <a key={link.href} href={link.href} className="mobile-link" onClick={onClose}>
              <span className="mono w-7 shrink-0">{link.index}</span>
              <span className="flex-1">{link.label}</span>
              <ArrowUpRight size={20} className="text-muted" aria-hidden="true" />
            </a>
          ))}
        </nav>

        <div className="mt-auto space-y-5 pt-10">
          <div className="flex items-center justify-between gap-4">
            <LanguageToggle />
            <ThemeToggle />
          </div>
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="flex items-center justify-between border-t border-line py-4 text-sm text-ink-soft"
          >
            <span className="flex items-center gap-2">
              <GithubIcon size={16} />
              {t.nav.github}
            </span>
            <ArrowUpRight size={16} aria-hidden="true" />
          </a>
          <a href="#download" className="btn btn-primary w-full" onClick={onClose}>
            <Download size={16} aria-hidden="true" />
            {t.hero.primary}
          </a>
          <p className="mono text-center">MemeDock · MIT · LOCAL-FIRST</p>
        </div>
      </div>
    </dialog>
  )
}
