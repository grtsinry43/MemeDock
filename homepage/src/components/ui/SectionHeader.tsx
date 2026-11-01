import type { ReactNode } from 'react'
import { cn } from '../../lib/cn'
import { renderMarked } from './Emphasis'

type SectionHeaderProps = {
  id?: string
  index: string
  label: string
  catalog: string
  title: string
  lede: string
  className?: string
  children?: ReactNode
}

export function SectionHeader({
  id,
  index: _index,
  label,
  catalog: _catalog,
  title,
  lede,
  className,
  children,
}: SectionHeaderProps) {
  return (
    <div className={cn('mb-10 md:mb-14', className)}>
      <div className="flex items-center gap-2 border-t border-line pt-4">
        <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
        <span className="eyebrow">{label}</span>
      </div>
      <div className="section-head mt-8">
        <div className="md:col-span-7">
          <h2
            id={id}
            className="display whitespace-pre-line text-[clamp(2rem,4vw,3.4rem)]"
          >
            {renderMarked(title)}
          </h2>
        </div>
        <div className="md:col-span-4 md:col-start-9">
          <p className="lede max-w-[35rem]">{lede}</p>
          {children}
        </div>
      </div>
    </div>
  )
}
