import { useState } from 'react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { Check, Sparkles } from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { cn } from '../lib/cn'
import { Mascot } from '../components/brand/Mascot'

const EASE = [0.16, 1, 0.3, 1] as const
const SPRING = { type: 'spring', stiffness: 420, damping: 28 } as const

export function FormatLab() {
  const { t } = useI18n()
  const lab = t.share.lab
  const [presetId, setPresetId] = useState(lab.presets[0].id)
  const [firstFrame, setFirstFrame] = useState(false)

  const preset = lab.presets.find((item) => item.id === presetId) ?? lab.presets[0]
  const transparent = preset.id === 'original' || preset.id === 'png'
  const container = firstFrame ? (preset.id === 'jpeg' ? 'JPEG' : 'PNG') : preset.container
  const stageKey = `${presetId}-${firstFrame ? 'still' : 'motion'}`

  const readoutValues: Record<string, string> = {
    container,
    edge: preset.edge,
    background: preset.background,
    animation: firstFrame ? lab.firstFrameValue : preset.animation,
  }

  return (
    <div className="lab">
      <div className="lab-grid">
        <div className="lab-controls">
          <span className="eyebrow flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
            {lab.eyebrow}
          </span>
          <h3 className="mt-2 font-brand text-lg font-semibold tracking-[-0.02em]">{lab.title}</h3>

          <div className="mt-4 flex flex-col gap-0.5" role="radiogroup" aria-label={lab.presetLabel}>
            {lab.presets.map((item) => {
              const active = presetId === item.id
              return (
                <button
                  key={item.id}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  className="format-preset"
                  onClick={() => setPresetId(item.id)}
                >
                  <span className="format-radio" aria-hidden="true">
                    {active ? (
                      <m.span
                        layoutId="format-dot"
                        className="format-dot"
                        transition={SPRING}
                      />
                    ) : null}
                  </span>
                  <span className="min-w-0">
                    <span className="flex items-center gap-1.5 font-brand text-[0.84rem] font-semibold">
                      {item.label}
                      {active ? (
                        <Check size={12} className="text-blue" aria-hidden="true" />
                      ) : null}
                    </span>
                    <span className="mt-0.5 block text-[0.68rem] leading-relaxed text-muted">
                      {item.detail}
                    </span>
                  </span>
                </button>
              )
            })}
          </div>

          <div className="mt-5 flex items-start justify-between gap-3 border-t border-line pt-4">
            <div>
              <div className="flex items-center gap-1.5 text-[0.8rem] font-medium">
                <Sparkles size={13} className="text-blue" aria-hidden="true" />
                {lab.firstFrame}
              </div>
              <p className="mt-1 text-[0.65rem] leading-relaxed text-muted">{lab.firstFrameHint}</p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={firstFrame}
              aria-label={lab.firstFrame}
              className="switch"
              onClick={() => setFirstFrame((value) => !value)}
            />
          </div>
        </div>

        <div className="lab-stage-wrap">
          <div
            className={cn('format-stage', transparent ? 'checker' : 'format-stage-white')}
            aria-label={lab.stageLabel}
          >
            <div className="absolute top-3 left-3 z-10 flex items-center gap-2">
              <span className="text-[0.68rem] text-muted">{lab.stageLabel}</span>
              <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
            </div>
            <span className="absolute top-3 right-3 z-10 font-mono text-[0.62rem] text-muted">{preset.container}</span>
            <span className="absolute bottom-3 left-3 z-10 text-[0.68rem] text-muted">{preset.edge}</span>
            {firstFrame ? (
              <span className="absolute right-3 bottom-3 z-10 text-[0.68rem] font-medium text-blue-deep">
                {lab.firstFrameBadge}
              </span>
            ) : null}

            <div className="relative" style={{ transform: `scale(${preset.scale})` }}>
              <m.div
                key={stageKey}
                initial={{ scale: 1.06, rotate: -2.5, opacity: 0.4 }}
                animate={{ scale: 1, rotate: 0, opacity: 1 }}
                transition={SPRING}
              >
                <Mascot size={210} paused={firstFrame} label={lab.mascotLabel} />
              </m.div>
            </div>

            <AnimatePresence mode="wait">
              <m.div
                key={`bubble-${stageKey}`}
                initial={{ opacity: 0, y: 8, scale: 0.96 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -4, scale: 0.97 }}
                transition={{ duration: 0.22, ease: EASE }}
                className="format-bubble"
              >
                {preset.bubble}
              </m.div>
            </AnimatePresence>
          </div>

          <div className="format-readout" role="status" aria-live="polite">
            {lab.readout.map((cell) => (
              <ReadoutCell key={cell.key} label={cell.label} value={readoutValues[cell.key]} />
            ))}
          </div>

          <p className="mt-3 text-[0.72rem] leading-relaxed text-muted">{lab.note}</p>
        </div>
      </div>
    </div>
  )
}

function ReadoutCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="format-readout-cell">
      <div className="text-[0.66rem] text-muted">{label}</div>
      <div className="relative mt-1 h-[1.15rem] overflow-hidden">
        <AnimatePresence mode="wait" initial={false}>
          <m.div
            key={value}
            initial={{ y: 12, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: -12, opacity: 0 }}
            transition={{ duration: 0.18, ease: EASE }}
            className="font-brand text-[0.82rem] font-semibold text-ink"
          >
            {value}
          </m.div>
        </AnimatePresence>
      </div>
    </div>
  )
}
