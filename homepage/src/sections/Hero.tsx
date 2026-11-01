import { useRef } from 'react'
import { useReducedMotion, useScroll, useTransform } from 'motion/react'
import * as m from 'motion/react-m'
import { ArrowDown, Download } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { ButtonLink } from '../components/ui/Button'
import { Specimen } from '../components/ui/Specimen'
import heroShot from '../../../assets/screenshots/hero-desktop.webp'
import androidShot from '../../../assets/screenshots/android-overview.webp'

const EASE = [0.16, 1, 0.3, 1] as const

const platformItems = [
  'macOS / SwiftUI + AppKit',
  'iOS / SwiftUI',
  'Android / Jetpack Compose',
  'Windows / WinUI 3',
  'Linux / GTK4 + Libadwaita',
  'Rust core',
  'MIT',
]

export function Hero() {
  const { t } = useI18n()
  const heroRef = useRef<HTMLElement>(null)
  const reduce = useReducedMotion()
  const { scrollYProgress } = useScroll({
    target: heroRef,
    offset: ['start start', 'end start'],
  })
  const parallaxY = useTransform(scrollYProgress, [0, 1], [0, reduce ? 0 : 32])
  const parallaxScale = useTransform(scrollYProgress, [0, 1], [1, reduce ? 1 : 0.95])

  return (
    <section ref={heroRef} id="top" className="hero">
      <div className="shell">
        <div className="grid gap-10 md:grid-cols-12 md:items-end md:gap-8">
          <div className="md:col-span-8">
            <m.p
              className="eyebrow flex items-center gap-2"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.5, ease: EASE }}
            >
              <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
              {t.hero.eyebrow}
            </m.p>

            <h1 className="display hero-title mt-6">
              {t.hero.title.map((line, index) => (
                <span
                  key={line}
                  className="-mb-[0.14em] block overflow-hidden pb-[0.14em] last:mb-0 last:pb-0"
                >
                  <m.span
                    className="block"
                    initial={{ y: '112%' }}
                    animate={{ y: 0 }}
                    transition={{ duration: 0.92, delay: 0.06 + index * 0.075, ease: EASE }}
                  >
                    {index === t.hero.title.length - 1 ? accentPeriod(line) : line}
                  </m.span>
                </span>
              ))}
            </h1>
          </div>

          <m.div
            className="md:col-span-4 md:pb-1"
            initial={{ opacity: 0, y: 14 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.36, ease: EASE }}
          >
            <p className="lede max-w-[28rem]">{t.hero.lede}</p>
            <div className="mt-6 flex flex-wrap items-center gap-2.5">
              <ButtonLink href="#download">
                <Download size={16} aria-hidden="true" />
                {t.hero.primary}
              </ButtonLink>
              <ButtonLink href="#collect" variant="ghost">
                {t.hero.secondary}
                <ArrowDown size={15} aria-hidden="true" />
              </ButtonLink>
            </div>
            <div className="mt-6 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[0.72rem] text-muted">
              <span className="font-brand font-semibold">{t.hero.platforms}</span>
              <span className="hidden size-1 rounded-full bg-line-strong sm:block" aria-hidden="true" />
              <span className="mono">{t.hero.version}</span>
              <span>{t.hero.license}</span>
            </div>
          </m.div>
        </div>
      </div>

      <div className="shell">
        <m.div style={{ y: parallaxY, scale: parallaxScale }} className="hero-visual hidden md:block">
          <m.div
            initial={{ opacity: 0, y: 34 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 1, delay: 0.48, ease: EASE }}
          >
            <Specimen
              src={heroShot}
              alt={t.hero.alt}
              width={2000}
              height={1400}
              figure={t.hero.figure}
              label={t.hero.figureLabel}
              caption={t.hero.figureCaption}
              annotations={[
                { n: 1, x: 7.5, y: 34 },
                { n: 2, x: 35, y: 13 },
              ]}
              priority
            />
            <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1.5 pl-1">
              {t.hero.annotations.map((annotation, index) => (
                <span
                  key={annotation}
                  className="flex items-center gap-1.5 text-[0.72rem] text-muted"
                >
                  <span className="flex size-4 items-center justify-center rounded-full bg-blue text-[0.58rem] font-semibold text-white">
                    {index + 1}
                  </span>
                  {annotation}
                </span>
              ))}
            </div>
          </m.div>
        </m.div>

        <div className="mt-12 flex justify-center md:hidden">
          <div>
            <div className="phone-crop">
              <img
                src={androidShot}
                alt={t.hero.phoneAlt}
                width={2000}
                height={1400}
                loading="eager"
                fetchPriority="high"
                decoding="async"
              />
            </div>
            <p className="mt-3 text-center text-[0.72rem] text-muted">{t.hero.phoneCaption}</p>
          </div>
        </div>
      </div>

      <div className="shell mt-12 md:mt-16">
        <div className="marquee border-y border-line py-4" aria-hidden="true">
          <div className="marquee-track">
            {[...platformItems, ...platformItems].map((item, index) => (
              <span key={`${item}-${index}`} className="mono flex items-center gap-4 pr-10">
                <span className="size-1 rounded-full bg-blue" />
                {item}
              </span>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}

function accentPeriod(line: string) {
  const last = line.slice(-1)
  return (
    <>
      {line.slice(0, -1)}
      <span className="text-blue">{last}</span>
    </>
  )
}
