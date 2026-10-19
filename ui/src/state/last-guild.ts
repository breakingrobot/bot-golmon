// Dernier serveur ouvert, mémorisé (localStorage `dernier-serveur`) : placé en tête de l'accueil.

import { atomWithStorage } from 'jotai/utils'

import { rawLocalStorage } from '@/state/storage'

const isSnowflake = (value: string): value is string => /^\d{1,20}$/u.test(value)

export const lastGuildIdAtom = atomWithStorage<string | null>(
  'dernier-serveur',
  null,
  rawLocalStorage<string | null>(isSnowflake, null),
  { getOnInit: true },
)
