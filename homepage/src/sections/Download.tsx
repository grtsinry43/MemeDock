import { useState } from 'react'
import {
  AppWindow,
  ArrowUpRight,
  Download as DownloadIcon,
  Monitor,
  Smartphone,
  Terminal,
  type LucideIcon,
} from 'lucide-react'
import { useI18n } from '../lib/i18n'
import { LATEST_RELEASE_URL, RELEASES_URL } from '../lib/github'
import { useLatestRelease } from '../lib/useGithub'
import {
  detectOs,
  formatBytes,
  preferredAsset,
  type Os,
} from '../lib/platform'
import { SectionHeader } from '../components/ui/SectionHeader'
import { Reveal } from '../components/ui/Reveal'

const platformIcons: Record<string, LucideIcon> = {
  macos: Monitor,
  windows: AppWindow,
  linux: Terminal,
  android: Smartphone,
  ios: Smartphone,
}

export function Download() {
  const { t } = useI18n()
  const d = t.download
  const [os] = useState<Os>(detectOs)
  const release = useLatestRelease()
  const version = release?.tag_name ?? d.fallbackVersion
  const detectedPlatform = d.platforms.find((platform) => platform.id === os)
  const primary = release ? preferredAsset(os, release.assets) : null
  const primaryHref = primary?.browser_download_url ?? LATEST_RELEASE_URL
  const primaryLabel = detectedPlatform
    ? `${d.download} ${detectedPlatform.name}`
    : d.choose
  const ResolvedIcon = platformIcons[os] ?? DownloadIcon

  return (
    <section id="download" className="section" aria-labelledby="download-title">
      <div className="shell">
        <SectionHeader
          id="download-title"
          index={d.index}
          label={d.label}
          catalog={d.catalog}
          title={d.title}
          lede={d.lede}
        />

        <Reveal>
          <div className="overflow-hidden rounded-[14px] border border-line bg-paper-raised shadow-[var(--shadow-card)]">
            <div className="grid md:grid-cols-12">
              <div className="relative overflow-hidden border-b border-line p-6 md:col-span-5 md:border-r md:border-b-0 md:p-8">
                <ResolvedIcon
                  size={180}
                  strokeWidth={0.7}
                  className="pointer-events-none absolute -right-6 -bottom-8 text-blue opacity-[0.06]"
                  aria-hidden="true"
                />
                <div className="relative">
                  <div className="flex items-center gap-2">
                    <span className="text-[0.72rem] font-medium text-blue-deep">{d.latest}</span>
                    <span className="mono">{version}</span>
                  </div>
                  <h3 className="display mt-5 max-w-[18rem] text-[clamp(2.2rem,3.3vw,3rem)]">
                    {detectedPlatform ? detectedPlatform.name : d.choose}
                  </h3>
                  <p className="mt-2 flex items-center gap-2 text-[0.78rem] text-muted">
                    <span className="size-1.5 rounded-full bg-blue" aria-hidden="true" />
                    {detectedPlatform ? d.recommendedFor : d.latestAssets}
                  </p>

                  <a
                    href={primaryHref}
                    target="_blank"
                    rel="noreferrer"
                    className="btn btn-primary mt-6 w-full"
                  >
                    <DownloadIcon size={16} aria-hidden="true" />
                    {primaryLabel}
                  </a>
                  {primary ? (
                    <div
                      className="mt-2 truncate font-mono text-[0.58rem] tracking-normal text-muted normal-case"
                      title={primary.name}
                    >
                      {primary.name} · {formatBytes(primary.size)}
                    </div>
                  ) : null}

                  <div className="mt-6 border-t border-line pt-4">
                    <a
                      href={RELEASES_URL}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1.5 font-brand text-[0.8rem] font-semibold text-blue-deep transition-colors hover:text-ink"
                    >
                      {d.allReleases}
                      <ArrowUpRight size={13} aria-hidden="true" />
                    </a>
                    <p className="mt-2 text-[0.65rem] leading-relaxed text-muted">{d.checksum}</p>
                  </div>
                </div>
              </div>

              <div className="p-3 md:col-span-7 md:p-5">
                <div className="overflow-hidden rounded-xl border border-line">
                  {d.platforms.map((platform, index) => {
                    const Icon = platformIcons[platform.id] ?? DownloadIcon
                    const asset = release ? preferredAsset(platform.id as Os, release.assets) : null
                    const href = asset?.browser_download_url ?? LATEST_RELEASE_URL
                    const recommended = platform.id === os
                    return (
                      <a
                        key={platform.id}
                        href={href}
                        target="_blank"
                        rel="noreferrer"
                        className={`group flex items-center gap-3 px-3.5 py-3 transition-colors hover:bg-blue-soft/50 md:px-4 ${
                          index === 0 ? '' : 'border-t border-line'
                        } ${recommended ? 'bg-blue-soft/60' : ''}`}
                      >
                        <span
                          className={`flex size-9 shrink-0 items-center justify-center rounded-xl border ${
                            recommended
                              ? 'border-blue/25 bg-paper-raised text-blue'
                              : 'border-line bg-paper-raised text-muted'
                          }`}
                        >
                          <Icon size={15} aria-hidden="true" />
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                            <span className="font-brand text-[0.85rem] font-semibold">
                              {platform.name}
                            </span>
                            <span className="font-mono text-[0.58rem] tracking-[0.08em] text-muted">
                              {platform.arch}
                            </span>
                            {recommended ? (
                              <span className="text-[0.6rem] font-medium text-blue-deep">
                                {d.recommended}
                              </span>
                            ) : null}
                          </span>
                          <span className="mt-0.5 block text-[0.66rem] leading-relaxed text-muted">
                            {platform.note}
                          </span>
                        </span>
                        <span className="hidden shrink-0 items-center gap-1.5 sm:flex">
                          {platform.formats.map((format) => (
                            <span key={format} className="font-mono text-[0.58rem] text-muted">
                              {format}
                            </span>
                          ))}
                        </span>
                        <span className="ml-1 flex shrink-0 flex-col items-end gap-0.5 text-muted">
                          {asset ? (
                            <span className="font-mono text-[0.5rem] tracking-wide">
                              {formatBytes(asset.size)}
                            </span>
                          ) : null}
                          <ArrowUpRight
                            size={13}
                            className="transition-colors group-hover:text-blue"
                            aria-hidden="true"
                          />
                        </span>
                      </a>
                    )
                  })}
                </div>
              </div>
            </div>

            <div className="flex flex-col gap-1.5 border-t border-line px-6 py-4 text-[0.66rem] leading-relaxed text-muted md:flex-row md:items-center md:justify-between md:px-8">
              <span>{d.note}</span>
              <span>{d.linuxNote}</span>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  )
}
