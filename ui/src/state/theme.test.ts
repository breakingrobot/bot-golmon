import { createStore } from 'jotai'
import { describe, expect, it, vi } from 'vitest'

// `getOnInit` lit le stockage à la création des atomes (chargement du module) : un module neuf par test.
const loadTheme = () => {
  vi.resetModules()
  return import('@/state/theme')
}

// `public/theme-init.js` relit la même clé avant le premier rendu : valeur brute, pas de JSON.
describe('thème (Jotai)', () => {
  it('relit le choix mémorisé en texte brut', async () => {
    localStorage.setItem('theme', 'dark')
    const { resolvedThemeAtom, themeChoiceAtom } = await loadTheme()
    const store = createStore()

    expect(store.get(themeChoiceAtom)).toBe('dark')
    expect(store.get(resolvedThemeAtom)).toBe('dark')
  })

  it('ignore une valeur inconnue', async () => {
    localStorage.setItem('theme', '"dark"')
    const { themeChoiceAtom } = await loadTheme()

    expect(createStore().get(themeChoiceAtom)).toBe('system')
  })

  it('écrit « light » ou « dark », et retire la clé pour « système »', async () => {
    const { themeChoiceAtom } = await loadTheme()
    const store = createStore()

    store.set(themeChoiceAtom, 'light')
    expect(localStorage.getItem('theme')).toBe('light')
    store.set(themeChoiceAtom, 'system')
    expect(localStorage.getItem('theme')).toBeNull()
  })

  it('suit le système quand aucun choix n’est fait', async () => {
    const { resolvedThemeAtom, systemDarkAtom } = await loadTheme()
    const store = createStore()

    store.set(systemDarkAtom, true)
    expect(store.get(resolvedThemeAtom)).toBe('dark')
    store.set(systemDarkAtom, false)
    expect(store.get(resolvedThemeAtom)).toBe('light')
  })
})
