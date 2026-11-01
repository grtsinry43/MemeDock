import { useState } from 'react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { ArrowUpRight, Plus } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { ISSUES_URL } from '../lib/github'
import { cn } from '../lib/cn'
import { Reveal } from '../components/ui/Reveal'

const EASE = [0.16, 1, 0.3, 1] as const

export function Faq() {
  const { t } = useI18n()
  const f = t.faq
  const [open, setOpen] = useState<number | null>(0)

  return (
    <section id="faq" className="section" aria-labelledby="faq-title">
      <div className="shell">
        <div className="grid gap-10 md:grid-cols-12 md:gap-8">
          <div className="md:col-span-4">
            <div className="flex items-center gap-2 border-t border-line pt-4">
              <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
              <span className="eyebrow">{f.label}</span>
            </div>
            <div className="md:sticky md:top-24">
              <h2 id="faq-title" className="display mt-8 text-[clamp(2.3rem,4.2vw,3.8rem)]">
                {f.title}
              </h2>
              <p className="lede mt-4 max-w-[26rem]">{f.lede}</p>
              <a
                href={ISSUES_URL}
                target="_blank"
                rel="noreferrer"
                className="mt-6 inline-flex items-center gap-1.5 font-brand text-[0.82rem] font-semibold text-blue-deep transition-colors hover:text-ink"
              >
                {f.contact}
                <ArrowUpRight size={14} aria-hidden="true" />
              </a>
            </div>
          </div>

          <Reveal className="md:col-span-7 md:col-start-6" delay={0.08}>
            <div className="border-t border-line">
              {f.items.map((item, index) => {
                const isOpen = open === index
                const panelId = `faq-panel-${index}`
                const triggerId = `faq-trigger-${index}`
                return (
                  <div key={item.q} className="faq-item">
                    <button
                      type="button"
                      id={triggerId}
                      className="faq-question"
                      aria-expanded={isOpen}
                      aria-controls={panelId}
                      onClick={() => setOpen(isOpen ? null : index)}
                    >
                      <span className="flex items-baseline gap-4">
                        <span className="mono">{String(index + 1).padStart(2, '0')}</span>
                        <span className="font-brand text-[0.95rem] font-semibold md:text-[1.02rem]">
                          {item.q}
                        </span>
                      </span>
                      <span
                        className={cn(
                          'flex size-7 shrink-0 items-center justify-center rounded-full border border-line text-muted transition-transform duration-300',
                          isOpen && 'rotate-45 border-blue/30 text-blue',
                        )}
                        aria-hidden="true"
                      >
                        <Plus size={14} />
                      </span>
                    </button>
                    <AnimatePresence initial={false}>
                      {isOpen ? (
                        <m.div
                          id={panelId}
                          role="region"
                          aria-labelledby={triggerId}
                          className="faq-answer"
                          initial={{ height: 0, opacity: 0 }}
                          animate={{ height: 'auto', opacity: 1 }}
                          exit={{ height: 0, opacity: 0 }}
                          transition={{ duration: 0.32, ease: EASE }}
                        >
                          <p className="pr-8 pb-5 pl-8 text-[0.82rem] leading-relaxed text-muted">
                            {item.a}
                          </p>
                        </m.div>
                      ) : null}
                    </AnimatePresence>
                  </div>
                )
              })}
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}
