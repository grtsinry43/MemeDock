import {
  Archive,
  ChevronLeft,
  ChevronRight,
  FileArchive,
  MoreHorizontal,
  ShieldCheck,
  Trash2,
} from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'

const featureIcons = [ShieldCheck, Trash2, Archive]

export function Keep() {
  const { t } = useI18n()
  const k = t.keep
  const backup = k.backup

  return (
    <section id="keep" className="ink-section section" aria-labelledby="keep-title">
      <div className="shell">
        <SectionHeader
          id="keep-title"
          index={k.index}
          label={k.label}
          catalog={k.catalog}
          title={k.title}
          lede={k.lede}
        />

        <Reveal>
          <div className="grid grid-cols-2 gap-px overflow-hidden border-y border-line bg-line md:grid-cols-4">
            {k.ledger.map((item, index) => (
              <div key={item.label} className="bg-paper px-4 py-5 md:px-5 md:py-6">
                <div
                  className={`numeral text-[clamp(2.3rem,3.8vw,3.6rem)] leading-none ${
                    index === k.ledger.length - 1 ? 'text-blue' : ''
                  }`}
                >
                  {item.value}
                </div>
                <div className="mt-2 text-[0.66rem] text-muted">{item.label}</div>
              </div>
            ))}
          </div>
        </Reveal>

        <div className="mt-12 grid gap-10 md:mt-16 md:grid-cols-12 md:gap-8">
          <Reveal className="md:col-span-5">
            <div className="border-t border-line">
              {k.features.map((feature, index) => {
                const Icon = featureIcons[index] ?? ShieldCheck
                return (
                  <div key={feature.title} className="flex gap-4 border-b border-line py-5">
                    <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl border border-line bg-paper-raised text-blue">
                      <Icon size={16} aria-hidden="true" />
                    </span>
                    <div>
                      <h3 className="font-brand text-[0.9rem] font-semibold">{feature.title}</h3>
                      <p className="mt-1 text-[0.78rem] leading-relaxed text-ink-soft">
                        {feature.body}
                      </p>
                    </div>
                  </div>
                )
              })}
            </div>
            <p className="mt-5 max-w-[30rem] text-[0.72rem] leading-relaxed text-muted">
              {k.footnote}
            </p>
          </Reveal>

          <Reveal className="md:col-span-7" delay={0.08}>
            <div className="overflow-hidden rounded-[14px] border border-line bg-paper-raised shadow-[var(--shadow-card)]">
              <div className="flex items-center gap-3 border-b border-line px-5 py-3.5">
                <ChevronLeft size={17} className="text-ink-soft" aria-hidden="true" />
                <span className="font-brand text-[0.9rem] font-semibold">{backup.deviceTitle}</span>
                <span className="ml-auto text-[0.66rem] text-muted">{backup.sample}</span>
                <MoreHorizontal size={17} className="text-muted" aria-hidden="true" />
              </div>

              <div className="p-5 md:p-6">
                <div className="flex items-center gap-3 rounded-xl border border-line bg-paper px-4 py-3">
                  <FileArchive size={17} className="text-blue" aria-hidden="true" />
                  <span className="text-[0.82rem] font-medium">{backup.choose}</span>
                  <ChevronRight size={16} className="ml-auto text-muted" aria-hidden="true" />
                </div>
                <p className="mt-2 text-[0.66rem] leading-relaxed text-muted">{backup.chooseHint}</p>

                <div className="mt-5 flex items-center gap-2">
                  <span className="mono">{backup.contentTitle}</span>
                  <span className="h-px flex-1 bg-line" aria-hidden="true" />
                </div>

                <div className="mt-1 overflow-hidden rounded-xl border border-line bg-paper">
                  {backup.rows.map((row, index) => (
                    <div
                      key={row.label}
                      className={`flex items-center justify-between px-4 py-2.5 text-[0.8rem] ${
                        index === 0 ? '' : 'border-t border-line'
                      }`}
                    >
                      <span className="text-ink-soft">{row.label}</span>
                      <span className="font-brand font-semibold">{row.value}</span>
                    </div>
                  ))}
                </div>

                <p className="mt-4 text-[0.72rem] font-medium text-ink">{backup.mergeNote}</p>
                <p className="mt-1.5 text-[0.66rem] leading-relaxed text-muted">
                  {backup.mergeHint}
                </p>

                <div className="mt-5 flex flex-col gap-2">
                  <span className="btn btn-blue w-full text-[0.82rem]">{backup.merge}</span>
                  <span className="py-2 text-center text-[0.7rem] font-medium text-danger">
                    {backup.replace}
                  </span>
                  <span className="text-center text-[0.7rem] text-muted">{backup.cancel}</span>
                </div>
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}
