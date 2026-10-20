// Recherche tolérante : sans accents, ligatures ni majuscules (« elodie » trouve « Élodie »).

/** Texte ramené à sa forme de comparaison. */
export const fold = (text: string): string =>
  text
    .normalize('NFD')
    .replaceAll(/\p{Diacritic}/gu, '')
    .toLowerCase()
    .replaceAll('œ', 'oe')
    .replaceAll('æ', 'ae')
    .trim()

/** Mots de la recherche, déjà ramenés. */
export const queryWords = (query: string): string[] => fold(query).split(/\s+/u).filter(Boolean)

/** Chaque mot de la recherche figure dans l'un des textes (recherche vide : tout correspond). */
export const matchesQuery = (query: string, ...texts: (string | null | undefined)[]): boolean => {
  const words = queryWords(query)
  if (words.length === 0) return true
  const haystack = fold(texts.filter(Boolean).join(' '))
  return words.every((word) => haystack.includes(word))
}
