import { ArrowUpRight } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { Reveal } from '../components/ui/Reveal'
import { renderMarked } from '../components/ui/Emphasis'
import { CountUp } from '../components/ui/CountUp'

export function Manifesto() {
  const { t } = useI18n()
  const m = t.manifesto

  return (
    <section id="why" className="section">
      <div className="shell">
        <div className="flex items-center gap-2 border-t border-line pt-4">
          <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
          <span className="eyebrow">{m.label}</span>
        </div>

        <div className="mt-10 grid gap-10 md:grid-cols-12 md:gap-8">
          <Reveal className="relative md:col-span-7">
            <h2 className="display text-[clamp(2.4rem,5vw,4.8rem)]">
              {m.title.map((line) => (
                <span key={line} className="block">
                  {renderMarked(line)}
                </span>
              ))}
            </h2>
          </Reveal>

          <Reveal className="flex flex-col gap-5 md:col-span-4 md:col-start-9" delay={0.1}>
            {m.body.map((paragraph) => (
              <p key={paragraph} className="body-copy">
                {paragraph}
              </p>
            ))}
          </Reveal>
        </div>

        <Reveal delay={0.12}>
          <div className="mt-14 grid grid-cols-2 gap-px border-y border-line bg-line md:mt-20 md:grid-cols-4">
            {m.stats.map((stat) => (
              <div key={stat.label} className="bg-paper px-4 py-6 md:px-6 md:py-8">
                <div className="numeral text-[clamp(2.5rem,4.2vw,4.1rem)] leading-none">
                  <CountUp value={stat.value} />
                </div>
                <div className="mt-2 text-[0.66rem] text-muted">{stat.label}</div>
              </div>
            ))}
          </div>
          <a
            href="#sync"
            className="mt-4 inline-flex items-center gap-1.5 text-[0.72rem] leading-relaxed text-muted transition-colors hover:text-ink"
          >
            {m.footnote}
            <ArrowUpRight size={12} aria-hidden="true" />
          </a>
        </Reveal>
      </div>
    </section>
  )
}
