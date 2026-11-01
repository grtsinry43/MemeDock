import { useEffect, useState } from 'react'

export function useActiveSection(ids: readonly string[]): string | null {
  const [active, setActive] = useState<string | null>(null)

  useEffect(() => {
    let frame = 0

    const measure = () => {
      frame = 0
      const target = window.scrollY + Math.min(window.innerHeight * 0.32, 220)
      let current: string | null = null
      for (const id of ids) {
        const element = document.getElementById(id)
        if (!element) continue
        if (element.offsetTop <= target) current = id
      }
      setActive(current)
    }

    const onScroll = () => {
      if (frame) return
      frame = window.requestAnimationFrame(measure)
    }

    measure()
    window.addEventListener('scroll', onScroll, { passive: true })
    window.addEventListener('resize', onScroll)
    return () => {
      if (frame) window.cancelAnimationFrame(frame)
      window.removeEventListener('scroll', onScroll)
      window.removeEventListener('resize', onScroll)
    }
  }, [ids])

  return active
}
