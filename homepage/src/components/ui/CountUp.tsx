import { useEffect, useRef, useState } from 'react'
import { useInView, useReducedMotion } from 'motion/react'

/** Counts a number up to its value when it scrolls into view. */
export function CountUp({ value, duration = 1.1 }: { value: string; duration?: number }) {
  const ref = useRef<HTMLSpanElement>(null)
  const inView = useInView(ref, { once: true, amount: 0.5 })
  const reduce = useReducedMotion()
  const target = Number(value)
  const [display, setDisplay] = useState(0)

  useEffect(() => {
    if (!inView) return
    if (reduce || target === 0) {
      setDisplay(target)
      return
    }
    let raf = 0
    let start: number | undefined
    const step = (ts: number) => {
      if (start === undefined) start = ts
      const progress = Math.min((ts - start) / (duration * 1000), 1)
      const eased = 1 - Math.pow(1 - progress, 3)
      setDisplay(Math.round(eased * target))
      if (progress < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  }, [inView, reduce, target, duration])

  return <span ref={ref}>{display}</span>
}
