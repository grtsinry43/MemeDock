import { useReducedMotion } from 'motion/react'
import * as m from 'motion/react-m'
import type { ReactNode } from 'react'
import { cn } from '../../lib/cn'

type RevealProps = {
  children: ReactNode
  className?: string
  delay?: number
  as?: 'div' | 'section' | 'figure'
}

const EASE = [0.16, 1, 0.3, 1] as const

export function Reveal({ children, className, delay = 0, as = 'div' }: RevealProps) {
  const reduce = useReducedMotion()
  const Component = m[as]

  return (
    <Component
      className={cn(className)}
      initial={{ opacity: 0, y: reduce ? 0 : 18 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, margin: '0px 0px -12% 0px' }}
      transition={{ duration: 0.7, delay, ease: EASE }}
    >
      {children}
    </Component>
  )
}
