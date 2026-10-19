// Thème clair / sombre : suit le système par défaut, choix manuel mémorisé (localStorage, clé `theme`,
// valeur brute `light` ou `dark`). public/theme-init.js applique le même calcul avant le premier rendu.

import { atom, useAtom, useAtomValue } from 'jotai'
import { atomWithStorage } from 'jotai/utils'
import { useEffect } from 'react'

import { rawLocalStorage } from '@/state/storage'

export type ThemeChoice = 'system' | 'light' | 'dark'
export type ResolvedTheme = 'light' | 'dark'

const isStoredChoice = (value: string): value is 'light' | 'dark' => value === 'light' || value === 'dark'

/** Choix de l'utilisateur ; « système » n'est pas écrit (clé retirée). */
export const themeChoiceAtom = atomWithStorage<ThemeChoice>(
  'theme',
  'system',
  rawLocalStorage<ThemeChoice>(isStoredChoice, 'system'),
  { getOnInit: true },
)

const darkQuery = () =>
  typeof window !== 'undefined' && 'matchMedia' in window ? window.matchMedia('(prefers-color-scheme: dark)') : null

/** Préférence du système, tenue à jour tant qu'un composant la lit. */
export const systemDarkAtom = atom(darkQuery()?.matches ?? false)
systemDarkAtom.onMount = (set) => {
  const query = darkQuery()
  if (!query) return undefined
  set(query.matches)
  const onChange = (event: MediaQueryListEvent) => set(event.matches)
  query.addEventListener('change', onChange)
  return () => query.removeEventListener('change', onChange)
}

export const resolvedThemeAtom = atom<ResolvedTheme>((get) => {
  const choice = get(themeChoiceAtom)
  if (choice !== 'system') return choice
  return get(systemDarkAtom) ? 'dark' : 'light'
})

export function useTheme() {
  const [choice, setChoice] = useAtom(themeChoiceAtom)
  const resolved = useAtomValue(resolvedThemeAtom)
  return { choice, resolved, setChoice }
}

/** Applique le thème résolu à <html> (classe `dark` et `color-scheme`), comme theme-init.js. */
export function ThemeSync() {
  const resolved = useAtomValue(resolvedThemeAtom)
  useEffect(() => {
    document.documentElement.classList.toggle('dark', resolved === 'dark')
    document.documentElement.style.colorScheme = resolved
  }, [resolved])
  return null
}
