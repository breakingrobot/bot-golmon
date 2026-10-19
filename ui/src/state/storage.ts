// Stockage local en texte brut pour `atomWithStorage` (pas de JSON : `public/theme-init.js` lit la
// même clé avant le premier rendu). Toute lecture ou écriture est protégée : navigation privée,
// cookies bloqués ou quota plein laissent la valeur en mémoire pour la session.

/** Stockage synchrone attendu par `atomWithStorage` (type non exporté par Jotai). */
export type SyncStorage<Value> = {
  getItem: (key: string, initialValue: Value) => Value
  setItem: (key: string, value: Value) => void
  removeItem: (key: string) => void
  subscribe: (key: string, callback: (value: Value) => void, initialValue: Value) => () => void
}

/**
 * `isValid` filtre les valeurs relues (clé modifiée à la main, ancienne version) ;
 * écrire `empty` (valeur par défaut) retire la clé.
 */
export function rawLocalStorage<Value extends string | null>(
  isValid: (value: string) => value is Exclude<Value, null>,
  empty: Value,
): SyncStorage<Value> {
  const read = (key: string, initialValue: Value): Value => {
    try {
      const value = localStorage.getItem(key)
      return value !== null && isValid(value) ? value : initialValue
    } catch {
      return initialValue
    }
  }
  return {
    getItem: read,
    setItem: (key, value) => {
      try {
        if (value === null || value === empty) localStorage.removeItem(key)
        else localStorage.setItem(key, value)
      } catch {
        // Valeur gardée en mémoire seulement.
      }
    },
    removeItem: (key) => {
      try {
        localStorage.removeItem(key)
      } catch {
        // Rien à retirer.
      }
    },
    // Changement fait dans un autre onglet.
    subscribe: (key, callback, initialValue) => {
      const onStorage = (event: StorageEvent) => {
        if (event.storageArea === localStorage && event.key === key) callback(read(key, initialValue))
      }
      window.addEventListener('storage', onStorage)
      return () => window.removeEventListener('storage', onStorage)
    },
  }
}
