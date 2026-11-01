import {
  ArrowRight,
  Check,
  Clipboard,
  FolderOpen,
  MousePointer2,
  Send,
  Share2,
} from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'
import { Specimen } from '../components/ui/Specimen'
import telegramShot from '../../../assets/screenshots/telegram-import.webp'

const channelIcons = [MousePointer2, Clipboard, FolderOpen, Share2, Send]

export function Collect() {
  const { t } = useI18n()
  const c = t.collect
  const telegram = c.telegram

  return (
    <section id="collect" className="section" aria-labelledby="collect-title">
      <div className="shell">
        <SectionHeader
          id="collect-title"
          index={c.index}
          label={c.label}
          catalog={c.catalog}
          title={c.title}
          lede={c.lede}
        />

        <div className="grid gap-10 md:grid-cols-12 md:gap-8">
          <Reveal className="md:col-span-5">
            <div className="border-t border-line">
              {c.channels.map((channel, index) => {
                const Icon = channelIcons[index] ?? MousePointer2
                return (
                  <div
                    key={channel.title}
                    className="group flex gap-4 border-b border-line py-4 transition-colors hover:border-line-strong"
                  >
                    <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl border border-line bg-paper-raised text-muted transition-colors group-hover:border-blue/30 group-hover:text-blue">
                      <Icon size={16} aria-hidden="true" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-baseline gap-2">
                        <h3 className="font-brand text-[0.9rem] font-semibold">{channel.title}</h3>
                        <span className="mono ml-auto">{String(index + 1).padStart(2, '0')}</span>
                      </div>
                      <p className="mt-0.5 text-[0.78rem] leading-relaxed text-muted">
                        {channel.body}
                      </p>
                    </div>
                  </div>
                )
              })}
            </div>
          </Reveal>

          <Reveal className="md:col-span-7" delay={0.1}>
            <div className="rounded-[14px] border border-line bg-paper-raised p-5 shadow-[var(--shadow-soft)] md:p-7">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div>
                  <span className="inline-flex items-center gap-1.5 text-[0.7rem] font-semibold text-muted">
                    <Send size={12} aria-hidden="true" />
                    {telegram.badge}
                  </span>
                  <h3 className="display mt-3 whitespace-pre-line text-[clamp(1.7rem,2.8vw,2.4rem)]">
                    {telegram.title}
                  </h3>
                  <p className="mt-2 max-w-[30rem] text-[0.84rem] leading-relaxed text-muted">
                    {telegram.body}
                  </p>
                </div>
                <div className="flex items-center gap-1.5 rounded-full border border-line bg-paper px-3 py-1.5">
                  {telegram.conversion.map((item, index) => (
                    <span key={item} className="flex items-center gap-1.5">
                      {index > 0 ? (
                        <ArrowRight size={11} className="text-muted" aria-hidden="true" />
                      ) : null}
                      <span
                        className={
                          index === telegram.conversion.length - 1
                            ? 'font-mono text-[0.58rem] tracking-[0.08em] text-blue-deep uppercase'
                            : 'font-mono text-[0.58rem] tracking-[0.08em] text-muted uppercase'
                        }
                      >
                        {item}
                      </span>
                    </span>
                  ))}
                </div>
              </div>

              <p className="mt-2 text-[0.68rem] text-muted">{telegram.conversionNote}</p>

              <div className="mt-5">
                <Specimen
                  src={telegramShot}
                  alt={telegram.alt}
                  width={2000}
                  height={1400}
                  figure={telegram.figure}
                  label={telegram.figureLabel}
                  caption={telegram.figureCaption}
                  annotations={[
                    { n: 1, x: 66, y: 35 },
                    { n: 2, x: 73, y: 54 },
                  ]}
                />
                <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1.5">
                  {telegram.annotations.map((annotation, index) => (
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
              </div>

              <div className="mt-5 flex items-start gap-2 border-t border-line pt-4 text-[0.72rem] leading-relaxed text-muted">
                <Check size={14} className="mt-0.5 shrink-0 text-blue" aria-hidden="true" />
                {telegram.duplicate}
              </div>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}
