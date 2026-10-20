// Recrues : libellés des avis, ancienneté d'essai, tendance des avis.

import type { Recrue, RecrueChoice } from '@/api/types'
import type { BadgeVariant } from '@/atoms/Badge'

export type ChoiceInfo = { value: RecrueChoice; label: string; emoji: string; variant: BadgeVariant }

const CHOICE_INFO: Record<RecrueChoice, ChoiceInfo> = {
  POUR: { value: 'POUR', label: 'Pour', emoji: '✅', variant: 'success' },
  CONTRE: { value: 'CONTRE', label: 'Contre', emoji: '❌', variant: 'danger' },
  BLANC: { value: 'BLANC', label: 'Blanc', emoji: '⬜', variant: 'neutral' },
}

/** Ordre d'affichage stable, comme sur Discord. */
export const CHOICES: ChoiceInfo[] = [CHOICE_INFO.POUR, CHOICE_INFO.CONTRE, CHOICE_INFO.BLANC]

export const choiceOf = (value: RecrueChoice): ChoiceInfo => CHOICE_INFO[value]

/** Motif d'un avis ou d'un véto (comme le formulaire Discord). */
export const MOTIF_MAX = 500

const DAY = 86_400_000

const dayText = (count: number) => `${count} jour${count > 1 ? 's' : ''}`

/** Durée d'essai lisible : « 3 jours », « 2 semaines », « 1 semaine et 2 jours ». */
export const trialLength = (since: string, now = Date.now()): string => {
  const days = Math.max(0, Math.floor((now - new Date(since).getTime()) / DAY))
  if (days < 1) return "moins d'un jour"
  const weeks = Math.floor(days / 7)
  const rest = days % 7
  if (weeks === 0) return dayText(rest)
  const weekText = `${weeks} semaine${weeks > 1 ? 's' : ''}`
  return rest === 0 ? weekText : `${weekText} et ${dayText(rest)}`
}

export type Trend = 'veto' | 'favorable' | 'defavorable' | 'partage' | 'aucun'

/** Tendance des avis de la semaine (le véto l'emporte toujours). */
export const trend = (recrue: Pick<Recrue, 'pour' | 'contre' | 'blanc' | 'veto'>): Trend => {
  if (recrue.veto) return 'veto'
  if (recrue.pour + recrue.contre + recrue.blanc === 0) return 'aucun'
  if (recrue.pour > recrue.contre) return 'favorable'
  if (recrue.contre > recrue.pour) return 'defavorable'
  return 'partage'
}

export const TREND_LABELS: Record<Trend, { label: string; variant: BadgeVariant }> = {
  veto: { label: 'Sous véto', variant: 'danger' },
  favorable: { label: 'Plutôt favorable', variant: 'success' },
  defavorable: { label: 'Plutôt défavorable', variant: 'warning' },
  partage: { label: 'Avis partagés', variant: 'neutral' },
  aucun: { label: 'Aucun avis', variant: 'outline' },
}
