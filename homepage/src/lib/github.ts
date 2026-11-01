export type ReleaseAsset = {
  name: string
  browser_download_url: string
  size: number
}

export type ReleaseInfo = {
  tag_name: string
  html_url: string
  published_at: string
  assets: ReleaseAsset[]
}

const REPO = 'grtsinry43/MemeDock'
const CACHE_TTL = 10 * 60 * 1000

type CacheEntry<T> = { value: T; at: number }

export function readCache<T>(key: string): T | null {
  try {
    const raw = window.sessionStorage.getItem(key)
    if (!raw) return null
    const entry = JSON.parse(raw) as CacheEntry<T>
    if (!entry || typeof entry.at !== 'number') return null
    if (Date.now() - entry.at > CACHE_TTL) return null
    return entry.value
  } catch {
    return null
  }
}

export function writeCache<T>(key: string, value: T) {
  try {
    const entry: CacheEntry<T> = { value, at: Date.now() }
    window.sessionStorage.setItem(key, JSON.stringify(entry))
  } catch {
    // Caching is a convenience; the network call still works without it.
  }
}

export async function fetchLatestRelease(signal?: AbortSignal): Promise<ReleaseInfo | null> {
  const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    signal,
    headers: {
      Accept: 'application/vnd.github+json',
      'X-GitHub-Api-Version': '2022-11-28',
    },
  })
  if (!response.ok) return null
  const data = (await response.json()) as ReleaseInfo
  if (!data || typeof data.tag_name !== 'string' || !Array.isArray(data.assets)) return null
  return {
    tag_name: data.tag_name,
    html_url: data.html_url,
    published_at: data.published_at,
    assets: data.assets
      .filter(
        (asset): asset is ReleaseAsset =>
          Boolean(asset) &&
          typeof asset.name === 'string' &&
          typeof asset.browser_download_url === 'string' &&
          typeof asset.size === 'number',
      )
      .map((asset) => ({
        name: asset.name,
        browser_download_url: asset.browser_download_url,
        size: asset.size,
      })),
  }
}

export async function fetchStarCount(signal?: AbortSignal): Promise<number | null> {
  const response = await fetch(`https://api.github.com/repos/${REPO}`, {
    signal,
    headers: {
      Accept: 'application/vnd.github+json',
      'X-GitHub-Api-Version': '2022-11-28',
    },
  })
  if (!response.ok) return null
  const data = (await response.json()) as { stargazers_count?: unknown }
  return typeof data.stargazers_count === 'number' ? data.stargazers_count : null
}

export const REPO_URL = `https://github.com/${REPO}`
export const AUTHOR_URL = 'https://www.grtsinry43.com'
export const BLOG_URL = 'https://blog.grtsinry43.com/posts/memedock-app'
export const RELEASES_URL = `${REPO_URL}/releases`
export const LATEST_RELEASE_URL = `${RELEASES_URL}/latest`
export const ISSUES_URL = `${REPO_URL}/issues`
export const SERVER_SOURCE_URL = `${REPO_URL}/tree/main/apps/server`
export const LICENSE_URL = `${REPO_URL}/blob/main/LICENSE`
