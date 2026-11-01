import { useMemo, useState } from 'react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { RotateCcw, Search, Star, X } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { stickers, stickerText, type StickerDef } from '../content/stickers'
import type { SiteContent } from '../content/zh'
import type { Locale } from '../lib/locale'
import { Mascot } from '../components/brand/Mascot'

const EASE = [0.16, 1, 0.3, 1] as const
const SPRING = { type: 'spring', stiffness: 460, damping: 30 } as const

const initialStars = new Set(stickers.filter((sticker) => sticker.starred).map((sticker) => sticker.id))

export function StickerSorter() {
  const { t, locale } = useI18n()
  const demo = t.organize.demo
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState('all')
  const [starred, setStarred] = useState<Set<string>>(() => new Set(initialStars))

  const tagLabels = useMemo(() => {
    const map = new Map<string, string>()
    for (const item of demo.filters) map.set(item.id, item.label)
    return map
  }, [demo.filters])

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return stickers.filter((sticker) => {
      if (filter === 'starred' && !starred.has(sticker.id)) return false
      if (filter !== 'all' && filter !== 'starred' && !sticker.tags.includes(filter)) return false
      if (!needle) return true
      const copy = sticker[locale]
      const tagText = sticker.tags.map((tag) => tagLabels.get(tag) ?? tag).join(' ')
      return `${copy.text} ${copy.sub} ${tagText}`.toLowerCase().includes(needle)
    })
  }, [filter, locale, query, starred, tagLabels])

  const reset = () => {
    setQuery('')
    setFilter('all')
    setStarred(new Set(initialStars))
  }

  const toggleStar = (id: string) => {
    setStarred((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <div className="workbench">
      <div className="workbench-top">
        <span className="eyebrow flex items-center gap-2">
          <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
          {demo.eyebrow}
        </span>
        <span className="mono ml-auto" role="status" aria-live="polite">
          {filtered.length} / {stickers.length} {demo.resultUnit}
        </span>
        <button
          type="button"
          className="ml-2 inline-flex items-center gap-1.5 rounded-full border border-line px-2.5 py-1 text-[0.65rem] text-muted transition-colors hover:border-line-strong hover:text-ink"
          onClick={reset}
          aria-label={demo.reset}
        >
          <RotateCcw size={12} aria-hidden="true" />
          <span className="hidden sm:inline">{demo.reset}</span>
        </button>
      </div>

      <div className="workbench-body">
        <div className="flex flex-col gap-3">
          <div className="search-field w-full">
            <Search size={16} className="shrink-0 text-muted" aria-hidden="true" />
            <label htmlFor="sticker-search" className="sr-only">
              {demo.searchLabel}
            </label>
            <input
              id="sticker-search"
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={demo.searchPlaceholder}
              autoComplete="off"
            />
            {query ? (
              <button
                type="button"
                className="text-muted transition-colors hover:text-ink"
                onClick={() => setQuery('')}
                aria-label={demo.clear}
              >
                <X size={14} aria-hidden="true" />
              </button>
            ) : null}
          </div>

          <div className="no-scrollbar -mx-0.5 flex gap-2 overflow-x-auto px-0.5 pb-0.5">
            {demo.filters.map((item) => (
              <button
                key={item.id}
                type="button"
                className="filter-chip"
                aria-pressed={filter === item.id}
                onClick={() => setFilter(item.id)}
              >
                {filter === item.id ? (
                  <m.span layoutId="chip-active" className="chip-active" transition={SPRING} />
                ) : null}
                <span className="relative z-10">{item.label}</span>
              </button>
            ))}
          </div>
        </div>

        <m.div layout className="sticker-grid mt-5">
          <AnimatePresence mode="popLayout" initial={false}>
            {filtered.map((sticker) => (
              <StickerCard
                key={sticker.id}
                sticker={sticker}
                locale={locale}
                t={t}
                starred={starred.has(sticker.id)}
                onToggleStar={toggleStar}
                tagLabel={(tag) => tagLabels.get(tag) ?? tag}
              />
            ))}
          </AnimatePresence>
        </m.div>

        {filtered.length === 0 ? (
          <m.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="flex flex-col items-center gap-3 py-12 text-center"
          >
            <Mascot size={76} />
            <p className="text-sm text-muted">{demo.empty}</p>
            <button
              type="button"
              className="btn btn-ghost min-h-9 px-4 text-xs"
              onClick={reset}
            >
              {demo.clear}
            </button>
          </m.div>
        ) : null}

        <p className="mt-5 text-center text-[0.68rem] text-muted">{demo.hint}</p>
      </div>
    </div>
  )
}

type StickerCardProps = {
  sticker: StickerDef
  locale: Locale
  t: SiteContent
  starred: boolean
  onToggleStar: (id: string) => void
  tagLabel: (tag: string) => string
}

function StickerCard({ sticker, locale, t, starred, onToggleStar, tagLabel }: StickerCardProps) {
  const copy = stickerText(sticker, locale)
  const long = copy.text.length > 4

  return (
    <m.div
      layout
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96 }}
      transition={{ duration: 0.28, ease: EASE }}
      whileHover={{ y: -4 }}
      className="sticker"
      data-tone={sticker.tone}
    >
      <button
        type="button"
        className="sticker-star"
        aria-pressed={starred}
        aria-label={starred ? t.organize.demo.unstar : t.organize.demo.star}
        onClick={() => onToggleStar(sticker.id)}
      >
        <Star size={13} fill={starred ? 'currentColor' : 'none'} aria-hidden="true" />
      </button>
      <div className="pt-4">
        <div
          className="sticker-text"
          style={long ? { fontSize: 'clamp(1.05rem, 2.4vw, 1.7rem)' } : undefined}
        >
          {copy.text}
        </div>
        <div className="sticker-sub">{copy.sub}</div>
      </div>
      <div className="sticker-tags">
        {sticker.tags.map((tag) => (
          <span key={tag} className="sticker-tag">
            {tagLabel(tag)}
          </span>
        ))}
      </div>
    </m.div>
  )
}
