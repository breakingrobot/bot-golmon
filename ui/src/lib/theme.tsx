// Thème clair / sombre : suit le système par défaut, choix manuel mémorisé (localStorage).
// public/theme-init.js applique le même calcul avant le premier rendu.

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'

export type ThemeChoice = 'system' | 'light' | 'dark'

const STORAGE_KEY = 'theme'
const media = () => window.matchMedia('(prefers-color-scheme: dark)')

function readChoice(): ThemeChoice {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    if (value === 'light' || value === 'dark') return value
  } catch {
    // Stockage indisponible (navigation privée, cookies bloqués) : thème du système.
  }
  return 'system'
}

function writeChoice(choice: ThemeChoice) {
  try {
    if (choice === 'system') localStorage.removeItem(STORAGE_KEY)
    else localStorage.setItem(STORAGE_KEY, choice)
  } catch {
    // Choix gardé pour la session seulement.
  }
}

type ThemeContextValue = { choice: ThemeChoice; resolved: 'light' | 'dark'; setChoice: (choice: ThemeChoice) => void }

const ThemeContext = createContext<ThemeContextValue | null>(null)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [choice, setChoiceState] = useState<ThemeChoice>(readChoice)
  const [systemDark, setSystemDark] = useState(() => media().matches)

  useEffect(() => {
    const query = media()
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches)
    query.addEventListener('change', onChange)
    return () => query.removeEventListener('change', onChange)
  }, [])

  const resolved = choice === 'system' ? (systemDark ? 'dark' : 'light') : choice

  useEffect(() => {
    document.documentElement.classList.toggle('dark', resolved === 'dark')
  }, [resolved])

  const setChoice = useCallback((next: ThemeChoice) => {
    writeChoice(next)
    setChoiceState(next)
  }, [])

  const value = useMemo(() => ({ choice, resolved, setChoice }), [choice, resolved, setChoice])
  return <ThemeContext value={value}>{children}</ThemeContext>
}

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext)
  if (!context) throw new Error('useTheme doit être utilisé dans ThemeProvider')
  return context
}
