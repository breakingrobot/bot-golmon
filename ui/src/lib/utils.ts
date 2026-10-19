import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** Initiales d'un nom (« Léo Marchetti » → « LM »). */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  const first = parts[0]?.[0] ?? '?'
  const second = parts.length > 1 ? (parts.at(-1)?.[0] ?? '') : ''
  return (first + second).toUpperCase()
}

/** Couleur d'un rôle Discord (entier RGB) en CSS, `null` pour la couleur par défaut. */
export function roleColor(color: number | null): string | null {
  if (!color) return null
  return `#${color.toString(16).padStart(6, '0')}`
}
