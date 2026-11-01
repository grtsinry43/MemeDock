import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { useI18n } from '../../lib/i18n'
import { cn } from '../../lib/cn'

export type SpecimenAnnotation = {
  n: number
  x: number
  y: number
}

type SpecimenProps = {
  src: string
  alt: string
  width: number
  height: number
  figure: string
  label: string
  caption: string
  annotations?: SpecimenAnnotation[]
  priority?: boolean
  className?: string
}

export function Specimen({
  src,
  alt,
  width,
  height,
  figure,
  label,
  caption,
  annotations = [],
  priority = false,
  className,
}: SpecimenProps) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const dialogRef = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog) return
    if (open && !dialog.open) dialog.showModal()
    if (!open && dialog.open) dialog.close()
  }, [open])

  return (
    <>
      <figure className={cn('specimen', className)}>
        <div className="specimen-frame">
          <img
            src={src}
            alt={alt}
            width={width}
            height={height}
            loading={priority ? 'eager' : 'lazy'}
            fetchPriority={priority ? 'high' : 'auto'}
            decoding="async"
          />
          <button
            type="button"
            className="specimen-zoom"
            onClick={() => setOpen(true)}
            aria-label={`${t.common.zoom}: ${alttrim(alt)}`}
          />
          {annotations.map((annotation) => (
            <span
              key={annotation.n}
              className="specimen-marker pointer-events-none hidden md:inline-flex"
              style={{ left: `${annotation.x}%`, top: `${annotation.y}%` }}
              aria-hidden="true"
            >
              {annotation.n}
            </span>
          ))}
        </div>
        <figcaption className="specimen-caption">
          {figure ? <span className="mono">{figure}</span> : null}
          {label ? <span className="mono">{label}</span> : null}
          <span className="ml-auto text-right text-[0.7rem] text-muted">{caption}</span>
        </figcaption>
      </figure>

      <dialog
        ref={dialogRef}
        className="lightbox"
        aria-label={alt}
        onClose={() => setOpen(false)}
        onClick={(event) => {
          if (event.target === event.currentTarget) setOpen(false)
        }}
      >
        <button
          type="button"
          className="icon-btn absolute top-4 right-4 z-10 text-white hover:bg-white/10"
          onClick={() => setOpen(false)}
          aria-label={t.common.close}
        >
          <X size={20} aria-hidden="true" />
        </button>
        <figure className="flex max-h-full flex-col items-center gap-4">
          <img src={src} alt={alt} width={width} height={height} decoding="async" />
          <figcaption className="mono !text-white/60">
            {figure} · {label}
          </figcaption>
        </figure>
      </dialog>
    </>
  )
}

function alttrim(value: string) {
  return value.length > 90 ? `${value.slice(0, 87)}…` : value
}
