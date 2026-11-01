import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from 'react'
import { cn } from '../../lib/cn'

export type ButtonVariant = 'primary' | 'ghost' | 'soft' | 'blue'

const variantClasses: Record<ButtonVariant, string> = {
  primary: 'btn-primary',
  ghost: 'btn-ghost',
  soft: 'btn-soft',
  blue: 'btn-blue',
}

type ButtonLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  variant?: ButtonVariant
  external?: boolean
  children: ReactNode
}

export function ButtonLink({
  variant = 'primary',
  external = false,
  className,
  children,
  ...props
}: ButtonLinkProps) {
  return (
    <a
      className={cn('btn', variantClasses[variant], className)}
      {...(external ? { target: '_blank', rel: 'noreferrer' } : {})}
      {...props}
    >
      {children}
    </a>
  )
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant
  children: ReactNode
}

export function Button({ variant = 'primary', className, children, ...props }: ButtonProps) {
  return (
    <button className={cn('btn', variantClasses[variant], className)} {...props}>
      {children}
    </button>
  )
}
