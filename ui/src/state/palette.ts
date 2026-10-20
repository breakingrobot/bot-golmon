// Palette de commandes (Ctrl+K / ⌘K, bouton de recherche de l'en-tête et de la barre du bas) et
// dialogue « Déclarer une cotisation » qu'elle peut ouvrir depuis n'importe quelle page du serveur.

import { atom } from 'jotai'

export const paletteOpenAtom = atom(false)

export const quickCotisationOpenAtom = atom(false)
