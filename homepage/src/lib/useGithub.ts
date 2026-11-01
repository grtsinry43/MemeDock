import { useEffect, useState } from 'react'
import { fetchLatestRelease, fetchStarCount, readCache, writeCache, type ReleaseInfo } from './github'

const STARS_KEY = 'memedock-github-stars-v1'
const RELEASE_KEY = 'memedock-release-v1'

export function useStarCount(): number | null {
  const [count, setCount] = useState<number | null>(() => readCache<number>(STARS_KEY))

  useEffect(() => {
    if (count !== null) return
    const controller = new AbortController()
    fetchStarCount(controller.signal)
      .then((value) => {
        if (value === null) return
        setCount(value)
        writeCache(STARS_KEY, value)
      })
      .catch(() => {
        // A missing star count is not worth surfacing.
      })
    return () => controller.abort()
  }, [count])

  return count
}

export function useLatestRelease(): ReleaseInfo | null {
  const [release, setRelease] = useState<ReleaseInfo | null>(() =>
    readCache<ReleaseInfo>(RELEASE_KEY),
  )

  useEffect(() => {
    if (release) return
    const controller = new AbortController()
    fetchLatestRelease(controller.signal)
      .then((value) => {
        if (!value) return
        setRelease(value)
        writeCache(RELEASE_KEY, value)
      })
      .catch(() => {
        // The download section keeps its release-page fallback.
      })
    return () => controller.abort()
  }, [release])

  return release
}
