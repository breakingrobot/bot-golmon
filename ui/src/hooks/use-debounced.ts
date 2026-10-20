import { useEffect, useState } from 'react'

export const useDebounced = <T>(value: T, delay = 250): T => {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = globalThis.setTimeout(() => setDebounced(value), delay)
    return () => globalThis.clearTimeout(timer)
  }, [value, delay])
  return debounced
}
