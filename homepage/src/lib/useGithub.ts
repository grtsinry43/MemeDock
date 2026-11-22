import { useEffect, useState } from 'react'
import { fetchLatestRelease, fetchStarCount, readCache, writeCache, type ReleaseInfo } from './github'

const STARS_KEY = 'memedock-github-stars-v1'
const RELEASE_KEY = 'memedock-release-v2'

let releaseMemory: ReleaseInfo | null = null
let releaseInflight: Promise<ReleaseInfo | null> | null = null
const releaseListeners = new Set<(value: ReleaseInfo) => void>()

let starsMemory: number | null = null
let starsInflight: Promise<number | null> | null = null
const starsListeners = new Set<(value: number) => void>()

function requestLatestRelease() {
  if (!releaseInflight) {
    releaseInflight = fetchLatestRelease()
      .then((value) => {
        if (value) {
          releaseMemory = value
          writeCache(RELEASE_KEY, value)
          for (const listener of releaseListeners) listener(value)
        }
        return value
      })
      .catch(() => null)
      .finally(() => {
        releaseInflight = null
      })
  }
  return releaseInflight
}

function requestStarCount() {
  if (!starsInflight) {
    starsInflight = fetchStarCount()
      .then((value) => {
        if (value !== null) {
          starsMemory = value
          writeCache(STARS_KEY, value)
          for (const listener of starsListeners) listener(value)
        }
        return value
      })
      .catch(() => null)
      .finally(() => {
        starsInflight = null
      })
  }
  return starsInflight
}

export function useStarCount(): number | null {
  const [count, setCount] = useState<number | null>(() => {
    if (starsMemory !== null) return starsMemory
    const cached = readCache<number>(STARS_KEY)
    if (cached !== null) starsMemory = cached
    return cached
  })

  useEffect(() => {
    const listener = (value: number) => setCount(value)
    starsListeners.add(listener)
    if (starsMemory !== null) {
      setCount(starsMemory)
    } else {
      void requestStarCount()
    }
    return () => {
      starsListeners.delete(listener)
    }
  }, [])

  return count
}

export function useLatestRelease(): ReleaseInfo | null {
  const [release, setRelease] = useState<ReleaseInfo | null>(() => {
    if (releaseMemory) return releaseMemory
    const cached = readCache<ReleaseInfo>(RELEASE_KEY)
    if (cached) releaseMemory = cached
    return cached
  })

  useEffect(() => {
    const listener = (value: ReleaseInfo) => setRelease(value)
    releaseListeners.add(listener)
    if (releaseMemory) {
      setRelease(releaseMemory)
    } else {
      void requestLatestRelease()
    }
    return () => {
      releaseListeners.delete(listener)
    }
  }, [])

  return release
}
