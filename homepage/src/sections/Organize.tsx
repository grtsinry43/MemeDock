import { useI18n } from '../lib/i18n'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'
import { Specimen } from '../components/ui/Specimen'
import { StickerSorter } from '../demos/StickerSorter'
import organizeShot from '../../../assets/screenshots/batch-organize.webp'

export function Organize() {
  const { t } = useI18n()
  const o = t.organize

  return (
    <section id="organize" className="section" aria-labelledby="organize-title">
      <div className="shell">
        <SectionHeader
          id="organize-title"
          index={o.index}
          label={o.label}
          catalog={o.catalog}
          title={o.title}
          lede={o.lede}
        />

        <Reveal>
          <StickerSorter />
          <p className="mt-3 text-[0.72rem] leading-relaxed text-muted">{o.demoCaption}</p>
        </Reveal>

        <div className="mt-14 grid gap-10 md:mt-20 md:grid-cols-12 md:gap-8">
          <Reveal className="md:col-span-7">
            <Specimen
              src={organizeShot}
              alt={o.alt}
              width={2000}
              height={1400}
              figure={o.figure}
              label={o.figureLabel}
              caption={o.figureCaption}
            />
          </Reveal>

          <Reveal className="md:col-span-4 md:col-start-9" delay={0.08}>
            <div className="border-t border-line">
              {o.features.map((feature, index) => (
                <div key={feature.title} className="border-b border-line py-4">
                  <div className="flex items-baseline gap-3">
                    <span className="mono">{String(index + 1).padStart(2, '0')}</span>
                    <h3 className="font-brand text-[0.9rem] font-semibold">{feature.title}</h3>
                  </div>
                  <p className="mt-1 pl-7 text-[0.78rem] leading-relaxed text-muted">
                    {feature.body}
                  </p>
                </div>
              ))}
            </div>
          </Reveal>
        </div>
      </div>
    </section>
  )
}
