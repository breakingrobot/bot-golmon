// Présences : libellés des réponses, état d'une présence, jours de la semaine des plannings.

import type { PresenceAnswer, PresenceSummary } from '@/api/types'
import type { BadgeVariant } from '@/atoms/Badge'

export const ANSWER_LABELS: Record<PresenceAnswer, string> = {
  PRESENT: 'Présent',
  ABSENT: 'Absent',
  RETARD: 'Retard',
}

export type PresenceState = 'upcoming' | 'open' | 'closed'

/** À venir (ouverte, pas encore passée), ouverte (heure passée, clôture imminente) ou close. */
export const presenceState = (
  presence: Pick<PresenceSummary, 'date' | 'closedAt'>,
  now = Date.now(),
): PresenceState => {
  if (presence.closedAt) return 'closed'
  return new Date(presence.date).getTime() > now ? 'upcoming' : 'open'
}

export const STATE_LABELS: Record<PresenceState, { label: string; variant: BadgeVariant }> = {
  upcoming: { label: 'À venir', variant: 'brand' },
  open: { label: 'Ouverte', variant: 'warning' },
  closed: { label: 'Close', variant: 'neutral' },
}

/** Lundi = 1 … dimanche = 7, comme `/gestion presence planifier`. */
export const WEEKDAYS = ['lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi', 'dimanche'] as const

/** Heure saisie `HH:MM` (ou `21h00`), normalisée en `HH:MM` ; `null` si invalide. */
export const normalizeTime = (text: string): string | null => {
  const match = /^(?<hours>\d{1,2})[:hH](?<minutes>\d{2})$/u.exec(text.trim())
  if (!match) return null
  const hours = Number(match.groups?.['hours'])
  const minutes = Number(match.groups?.['minutes'])
  if (hours > 23 || minutes > 59) return null
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`
}

/** Taux de réponse d'une présence : réponses sur réponses + sans réponse. */
export const answeredShare = (presence: Pick<PresenceSummary, 'present' | 'absent' | 'retard' | 'noAnswer'>) => {
  const answered = presence.present + presence.absent + presence.retard
  const total = answered + presence.noAnswer
  return total > 0 ? Math.round((answered * 100) / total) : null
}
