import { useScroll, useSpring } from 'motion/react'
import * as m from 'motion/react-m'

/** A thin reading-progress bar fixed to the top of the viewport. */
export function ScrollProgress() {
  const { scrollYProgress } = useScroll()
  const scaleX = useSpring(scrollYProgress, { stiffness: 140, damping: 30, restDelta: 0.001 })

  return (
    <m.div
      aria-hidden="true"
      className="fixed top-0 right-0 left-0 z-[60] h-[2px] origin-left bg-blue"
      style={{ scaleX }}
    />
  )
}
