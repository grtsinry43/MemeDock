import { cn } from '../../lib/cn'

export function LogoMark({ size = 30, className }: { size?: number; className?: string }) {
  return (
    <img
      src={`${import.meta.env.BASE_URL}favicon.svg`}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      className={cn('shrink-0', className)}
    />
  )
}

export function Logo({
  size = 30,
  className,
  wordClassName,
}: {
  size?: number
  className?: string
  wordClassName?: string
}) {
  return (
    <span className={cn('inline-flex items-center gap-2.5', className)}>
      <LogoMark size={size} />
      <span className={cn('wordmark text-[1.05rem] leading-none', wordClassName)}>MemeDock</span>
    </span>
  )
}
