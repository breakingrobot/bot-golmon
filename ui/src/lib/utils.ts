import { clsx } from 'clsx'
import type { ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs))

/** Initiales d'un nom (« Léo Marchetti » → « LM »). */
export const initials = (name: string): string => {
  const parts = name.trim().split(/\s+/u).filter(Boolean)
  const first = parts[0]?.[0] ?? '?'
  const second = parts.length > 1 ? (parts.at(-1)?.[0] ?? '') : ''
  return (first + second).toUpperCase()
}

/** Couleur d'un rôle Discord (entier RGB) en CSS, `null` pour la couleur par défaut. */
export const roleColor = (color: number | null): string | null => {
  if (!color) return null
  return `#${color.toString(16).padStart(6, '0')}`
}
