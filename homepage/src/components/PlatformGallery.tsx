import { useState } from 'react'
import { AnimatePresence, useReducedMotion } from 'motion/react'
import * as m from 'motion/react-m'
import { ArrowLeft, ArrowRight } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import macShot from '../../../assets/screenshots/hero-desktop.webp'
import windowsShot from '../../../assets/screenshots/windows-overview.webp'
import androidShot from '../../../assets/screenshots/android-overview.webp'

const EASE = [0.16, 1, 0.3, 1] as const

const shots = [
  { id: 'windows', name: 'Windows', src: windowsShot, width: 2000, height: 1077 },
  { id: 'macos', name: 'macOS', src: macShot, width: 2000, height: 1396 },
  { id: 'android', name: 'Android', src: androidShot, width: 1920, height: 1331 },
]

export function PlatformGallery() {
  const { t } = useI18n()
  const g = t.native.gallery
  const reduce = useReducedMotion()
  const [index, setIndex] = useState(0)
  const [direction, setDirection] = useState(1)
  const current = shots[index]

  const go = (dir: number) => {
    setDirection(dir)
    setIndex((i) => (i + dir + shots.length) % shots.length)
  }

  const goTo = (i: number) => {
    if (i === index) return
    setDirection(i > index ? 1 : -1)
    setIndex(i)
  }

  const variants = {
    enter: (dir: number) => ({ opacity: 0, x: reduce ? 0 : dir * 64 }),
    center: { opacity: 1, x: 0 },
    exit: (dir: number) => ({ opacity: 0, x: reduce ? 0 : dir * -64 }),
  }

  return (
    <div className="mx-auto max-w-[880px]">
      <div
        className="relative overflow-hidden rounded-xl border border-line bg-paper"
        style={{ aspectRatio: `${current.width} / ${current.height}` }}
      >
        <AnimatePresence custom={direction} initial={false}>
          <m.img
            key={current.id}
            src={current.src}
            alt={`${current.name} — ${g.label}`}
            width={current.width}
            height={current.height}
            custom={direction}
            variants={variants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.45, ease: EASE }}
            className="absolute inset-0 h-full w-full"
            decoding="async"
          />
        </AnimatePresence>

        <button
          type="button"
          onClick={() => go(-1)}
          aria-label={g.prev}
          className="absolute top-1/2 left-3 z-10 flex size-9 -translate-y-1/2 items-center justify-center rounded-full bg-black/40 text-white backdrop-blur transition hover:bg-black/60"
        >
          <ArrowLeft size={17} aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={() => go(1)}
          aria-label={g.next}
          className="absolute top-1/2 right-3 z-10 flex size-9 -translate-y-1/2 items-center justify-center rounded-full bg-black/40 text-white backdrop-blur transition hover:bg-black/60"
        >
          <ArrowRight size={17} aria-hidden="true" />
        </button>
      </div>

      <div className="mt-4 flex items-center justify-center gap-7">
        {shots.map((s, i) => (
          <button
            key={s.id}
            type="button"
            onClick={() => goTo(i)}
            aria-current={i === index ? 'true' : undefined}
            className={`relative font-brand text-[0.82rem] font-semibold transition-colors ${
              i === index ? 'text-ink' : 'text-muted hover:text-ink-soft'
            }`}
          >
            {s.name}
            {i === index ? (
              <span
                className="absolute -bottom-1.5 left-1/2 size-1 -translate-x-1/2 rounded-full bg-blue"
                aria-hidden="true"
              />
            ) : null}
          </button>
        ))}
      </div>
    </div>
  )
}
