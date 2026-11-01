import { useI18n } from '../lib/i18n'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'
import { Specimen } from '../components/ui/Specimen'
import { FormatLab } from '../demos/FormatLab'
import outputShot from '../../../assets/screenshots/output-backup-settings.webp'

export function Share() {
  const { t } = useI18n()
  const s = t.share

  return (
    <section id="share" className="section" aria-labelledby="share-title">
      <div className="shell">
        <SectionHeader
          id="share-title"
          index={s.index}
          label={s.label}
          catalog={s.catalog}
          title={s.title}
          lede={s.lede}
        />

        <div className="grid gap-10 md:grid-cols-12 md:gap-8">
          <Reveal className="flex flex-col md:col-span-4">
            <div className="border-t border-line">
              {s.notes.map((note, index) => (
                <div key={note.title} className="border-b border-line py-4">
                  <div className="flex items-baseline gap-3">
                    <span className="mono">{String(index + 1).padStart(2, '0')}</span>
                    <h3 className="font-brand text-[0.9rem] font-semibold">{note.title}</h3>
                  </div>
                  <p className="mt-1 pl-7 text-[0.78rem] leading-relaxed text-muted">{note.body}</p>
                </div>
              ))}
            </div>
            <div className="mt-8 hidden md:block">
              <Specimen
                src={outputShot}
                alt={s.alt}
                width={2000}
                height={1400}
                figure={s.figure}
                label={s.figureLabel}
                caption={s.figureCaption}
              />
            </div>
          </Reveal>

          <Reveal className="md:col-span-8" delay={0.08}>
            <FormatLab />
          </Reveal>
        </div>

        <div className="mt-8 md:hidden">
          <Specimen
            src={outputShot}
            alt={s.alt}
            width={2000}
            height={1400}
            figure={s.figure}
            label={s.figureLabel}
            caption={s.figureCaption}
          />
        </div>
      </div>
    </section>
  )
}
