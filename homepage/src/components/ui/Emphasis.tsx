import { useEffect, useRef, useState } from 'react'
import { useInView, useReducedMotion } from 'motion/react'
import type { ReactNode } from 'react'

/**
 * A hand-drawn scribble circle that draws itself around a word when it scrolls
 * into view. The path is intentionally irregular and unclosed (like a real pen
 * stroke), and its length is measured at runtime so the dash animation is exact.
 */
export function Emphasis({ children, className }: { children: ReactNode; className?: string }) {
  const wrapRef = useRef<HTMLSpanElement>(null)
  const pathRef = useRef<SVGPathElement>(null)
  const inView = useInView(wrapRef, { once: true, amount: 0.3 })
  const reduce = useReducedMotion()
  const [len, setLen] = useState(0)

  useEffect(() => {
    const path = pathRef.current
    if (path) setLen(Math.ceil(path.getTotalLength()))
  }, [])

  const offset = reduce || inView ? 0 : len

  return (
    <span
      ref={wrapRef}
      className={`relative inline-block whitespace-nowrap px-2 py-1 ${className ?? ''}`}
    >
      <span className="relative z-10">{children}</span>
      <svg
        className="pointer-events-none absolute inset-0 block h-full w-full overflow-visible"
        viewBox="0 0 120 44"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <path
          ref={pathRef}
          d="M60 5C22 4 6 12 6 22s20 18 56 17 52-9 52-18S96 4 64 5"
          fill="none"
          stroke="var(--blue)"
          strokeWidth={2.5}
          strokeLinecap="round"
          strokeDasharray={len}
          style={{
            strokeDashoffset: offset,
            transition: reduce
              ? 'none'
              : 'stroke-dashoffset 0.9s cubic-bezier(0.16, 1, 0.3, 1) 0.15s',
          }}
        />
      </svg>
    </span>
  )
}

/** Parses [[marked]] spans into <Emphasis> nodes. Plain text passes through. */
export function renderMarked(text: string): ReactNode {
  const parts = text.split(/(\[\[.*?\]\])/g)
  return parts.map((part, index) => {
    if (part.startsWith('[[') && part.endsWith(']]')) {
      return <Emphasis key={index}>{part.slice(2, -2)}</Emphasis>
    }
    return part
  })
}
