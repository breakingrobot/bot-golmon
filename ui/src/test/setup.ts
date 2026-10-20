// Préparation commune des tests : assertions DOM, nettoyage, API de navigateur absentes de happy-dom.

import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

afterEach(() => {
  cleanup()
  localStorage.clear()
  sessionStorage.clear()
})

// Radix (Select, Popover) mesure et capture le pointeur : happy-dom n'implémente pas tout.
const element = Element.prototype as Element & {
  hasPointerCapture?: (id: number) => boolean
  releasePointerCapture?: (id: number) => void
  scrollIntoView?: () => void
}
element.hasPointerCapture ??= () => false
element.releasePointerCapture ??= () => {
  // Rien à relâcher.
}
element.scrollIntoView ??= () => {
  // Pas de défilement en test.
}
