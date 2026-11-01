import {
  AppWindow,
  Cpu,
  Laptop,
  Monitor,
  Smartphone,
  Terminal,
  type LucideIcon,
} from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'
import { PlatformGallery } from '../components/PlatformGallery'

const platformIcons: Record<string, LucideIcon> = {
  macos: Monitor,
  ios: Smartphone,
  android: Smartphone,
  windows: AppWindow,
  linux: Terminal,
}

export function Native() {
  const { t } = useI18n()
  const n = t.native

  return (
    <section id="native" className="section" aria-labelledby="native-title">
      <div className="shell">
        <SectionHeader
          id="native-title"
          index={n.index}
          label={n.label}
          catalog={n.catalog}
          title={n.title}
          lede={n.lede}
        />

        <Reveal>
          <div className="architecture">
            <div className="arch-grid">
              {n.platforms.map((platform) => {
                const Icon = platformIcons[platform.id] ?? Laptop
                return (
                  <div key={platform.id} className="arch-card">
                    <div className="flex items-center gap-2">
                      <Icon size={15} className="text-blue" aria-hidden="true" />
                      <span className="font-brand text-[0.85rem] font-semibold">
                        {platform.name}
                      </span>
                    </div>
                    <div className="mono mt-2 text-[0.55rem]">{platform.stack}</div>
                    <p className="mt-2 text-[0.68rem] leading-relaxed text-muted">
                      {platform.note}
                    </p>
                  </div>
                )
              })}
            </div>

            <div className="arch-connectors" aria-hidden="true">
              <span className="arch-bus" />
              {[10, 30, 50, 70, 90].map((left) => (
                <span key={left} className="arch-stem" style={{ left: `${left}%` }} />
              ))}
              <span className="arch-stem arch-stem--down" style={{ left: '50%' }} />
            </div>

            <div className="arch-core">
              <div className="flex flex-wrap items-center gap-4">
                <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-paper-raised text-blue">
                  <Cpu size={18} aria-hidden="true" />
                </span>
                <div>
                  <div className="font-brand text-[1rem] font-semibold">{n.core.title}</div>
                  <div className="mt-0.5 text-[0.72rem] text-muted">{n.core.sub}</div>
                </div>
                <div className="flex flex-wrap gap-1.5 md:ml-auto">
                  {n.core.chips.map((chip) => (
                    <span
                      key={chip}
                      className="rounded-md border border-line bg-paper-raised px-2 py-1 font-mono text-[0.55rem] tracking-wide text-muted"
                    >
                      {chip}
                    </span>
                  ))}
                </div>
              </div>
            </div>

            <p className="mt-4 text-center text-[0.68rem] leading-relaxed text-muted">
              {n.caption}
            </p>
          </div>
        </Reveal>

        <Reveal delay={0.08}>
          <div className="mt-10">
            <PlatformGallery />
          </div>
        </Reveal>

        <div className="mt-12 grid gap-8 border-t border-line pt-8 md:grid-cols-3 md:gap-10">
          {n.notes.map((note, index) => (
            <Reveal key={note.title} delay={index * 0.06}>
              <div className="flex items-baseline gap-3">
                <span className="mono">{String(index + 1).padStart(2, '0')}</span>
                <h3 className="font-brand text-[0.9rem] font-semibold">{note.title}</h3>
              </div>
              <p className="mt-1 pl-7 text-[0.78rem] leading-relaxed text-muted">{note.body}</p>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  )
}
