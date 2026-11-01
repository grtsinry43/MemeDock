import { useId } from 'react'
import { cn } from '../../lib/cn'

type MascotProps = {
  size?: number
  paused?: boolean
  className?: string
  label?: string
}

export function Mascot({ size = 220, paused = false, className, label }: MascotProps) {
  const uid = useId().replace(/:/g, '')
  const flapId = `mascot-flap-${uid}`

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="20 20 68 68"
      className={cn('mascot', className)}
      data-paused={paused}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <defs>
        <linearGradient id={flapId} x1="81" y1="65" x2="65" y2="81" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#E9F1FF" />
          <stop offset="1" stopColor="#B9D0FA" />
        </linearGradient>
      </defs>
      <g transform="rotate(-5 54 54)">
        <path
          d="M40,27 H68 A13,13 0 0 1 81,40 V65 L65,81 H40 A13,13 0 0 1 27,68 V40 A13,13 0 0 1 40,27 Z"
          fill="#0B3A82"
          fillOpacity=".14"
          transform="translate(0 2.5)"
        />
        <path
          d="M40,27 H68 A13,13 0 0 1 81,40 V65 L65,81 H40 A13,13 0 0 1 27,68 V40 A13,13 0 0 1 40,27 Z"
          fill="#FFFFFF"
          stroke="rgba(22,24,28,.12)"
          strokeWidth=".8"
        />
        <circle className="mascot-eye" cx="45" cy="47" r="3.6" fill="#1F6FEB" />
        <circle className="mascot-eye" cx="58" cy="47" r="3.6" fill="#1F6FEB" />
        <path
          d="M42.5,56 Q51.5,65 60.5,56"
          fill="none"
          stroke="#1F6FEB"
          strokeWidth="4.4"
          strokeLinecap="round"
        />
        <path
          d="M81,65 H71 Q65,65 65,71 V81 Z"
          fill="#0B3A82"
          fillOpacity=".16"
          transform="translate(-1.6 -1.6)"
        />
        <path d="M81,65 H71 Q65,65 65,71 V81 Z" fill={`url(#${flapId})`} />
      </g>
    </svg>
  )
}
