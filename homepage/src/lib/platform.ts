import type { ReleaseAsset } from './github'

export type Os = 'macos' | 'windows' | 'linux' | 'android' | 'ios' | 'unknown'

export function detectOs(): Os {
  const ua = navigator.userAgent
  const platform = (
    navigator as Navigator & { userAgentData?: { platform?: string } }
  ).userAgentData?.platform
    ? (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData
        ?.platform
    : navigator.platform

  if (/android/i.test(ua)) return 'android'
  if (/iphone|ipad|ipod/i.test(ua)) return 'ios'
  if (/mac/i.test(platform ?? '') && navigator.maxTouchPoints > 1) return 'ios'
  if (/win/i.test(platform ?? '') || /windows/i.test(ua)) return 'windows'
  if (/mac/i.test(platform ?? '') || /mac os x/i.test(ua)) return 'macos'
  if (/linux/i.test(platform ?? '') || /linux/i.test(ua)) return 'linux'
  return 'unknown'
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return ''
  const mb = bytes / (1024 * 1024)
  if (mb >= 100) return `${mb.toFixed(0)} MB`
  if (mb >= 10) return `${mb.toFixed(1)} MB`
  return `${mb.toFixed(2)} MB`
}

export function assetsForOs(os: Os, assets: ReleaseAsset[]): ReleaseAsset[] {
  const names: Record<Exclude<Os, 'unknown'>, RegExp> = {
    macos: /macos-arm64/i,
    windows: /windows-x64/i,
    linux: /linux-x86_64/i,
    android: /android-arm64/i,
    ios: /ios-arm64/i,
  }
  if (os === 'unknown') return []
  return assets.filter((asset) => names[os].test(asset.name))
}

export function preferredAsset(os: Os, assets: ReleaseAsset[]): ReleaseAsset | null {
  const matching = assetsForOs(os, assets)
  if (matching.length === 0) return null
  const find = (pattern: RegExp) => matching.find((asset) => pattern.test(asset.name))

  switch (os) {
    case 'macos':
      return find(/\.dmg$/i) ?? find(/\.zip$/i) ?? matching[0]
    case 'windows':
      return find(/setup\.exe$/i) ?? find(/\.zip$/i) ?? matching[0]
    case 'linux':
      return find(/\.appimage$/i) ?? find(/\.deb$/i) ?? matching[0]
    case 'android':
      return find(/\.apk$/i) ?? matching[0]
    case 'ios':
      return find(/\.ipa$/i) ?? matching[0]
    default:
      return null
  }
}
