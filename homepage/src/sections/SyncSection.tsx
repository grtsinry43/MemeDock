import {
  ArrowDown,
  ArrowUpRight,
  KeyRound,
  Laptop,
  Lock,
  Monitor,
  RefreshCw,
  Server,
  Smartphone,
} from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { SERVER_SOURCE_URL } from '../lib/github'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'

const pointIcons = [Server, KeyRound, RefreshCw]
const deviceIcons = [Laptop, Smartphone, Monitor]

export function SyncSection() {
  const { t } = useI18n()
  const s = t.sync
  const d = s.diagram

  return (
    <section id="sync" className="section" aria-labelledby="sync-title">
      <div className="shell">
        <SectionHeader
          id="sync-title"
          index={s.index}
          label={s.label}
          catalog={s.catalog}
          title={s.title}
          lede={s.lede}
        />

        <div className="grid gap-10 md:grid-cols-12 md:gap-8">
          <Reveal className="md:col-span-5">
            <div className="border-t border-line">
              {s.points.map((point, index) => {
                const Icon = pointIcons[index] ?? Server
                return (
                  <div key={point.title} className="flex gap-4 border-b border-line py-5">
                    <span className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl border border-line bg-paper-raised text-blue">
                      <Icon size={16} aria-hidden="true" />
                    </span>
                    <div>
                      <h3 className="font-brand text-[0.9rem] font-semibold">{point.title}</h3>
                      <p className="mt-1 text-[0.78rem] leading-relaxed text-muted">{point.body}</p>
                    </div>
                  </div>
                )
              })}
            </div>
            <p className="mt-5 max-w-[30rem] text-[0.72rem] leading-relaxed text-muted">{s.note}</p>
            <a
              href={SERVER_SOURCE_URL}
              target="_blank"
              rel="noreferrer"
              className="mt-5 inline-flex items-center gap-1.5 font-brand text-[0.82rem] font-semibold text-blue-deep transition-colors hover:text-ink"
            >
              {s.link}
              <ArrowUpRight size={14} aria-hidden="true" />
            </a>
          </Reveal>

          <Reveal className="md:col-span-7" delay={0.08}>
            <div className="sync-map">
              <div className="flex items-center gap-2">
                <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
                <span className="text-[0.68rem] text-muted">{d.badge}</span>
              </div>

              <div className="mt-6 grid grid-cols-3 gap-2.5">
                {d.devices.map((device, index) => {
                  const Icon = deviceIcons[index] ?? Laptop
                  return (
                    <div key={device} className="sync-device">
                      <Icon size={18} className="mb-1 text-muted" aria-hidden="true" />
                      <span>{device}</span>
                    </div>
                  )
                })}
              </div>

              <div className="sync-wires hidden md:block" aria-hidden="true">
                <span className="sync-bus" />
                <span className="sync-stem" style={{ left: '16.66%' }} />
                <span className="sync-stem" style={{ left: '50%' }} />
                <span className="sync-stem" style={{ left: '83.33%' }} />
                <span className="sync-stem sync-stem--down" style={{ left: '50%' }} />
              </div>

              <div className="flex justify-center py-3 md:hidden" aria-hidden="true">
                <ArrowDown size={16} className="text-muted" />
              </div>

              <div className="sync-server">
                <div className="flex items-start gap-3">
                  <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-paper-raised text-blue">
                    <Server size={18} aria-hidden="true" />
                  </span>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="font-brand text-[0.95rem] font-semibold">{d.server}</span>
                      <Lock size={12} className="text-blue" aria-hidden="true" />
                    </div>
                    <div className="mono mt-0.5">{d.serverSub}</div>
                  </div>
                </div>
              </div>

              <p className="mt-4 text-center text-[0.68rem] text-muted">{d.caption}</p>
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}
