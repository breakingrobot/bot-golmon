import type { AbsenceEntry } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import type { BadgeVariant } from '@/atoms/Badge'
import { Card } from '@/atoms/Card'
import { absenceState, effectiveEnd } from '@/lib/absence-timeline'
import type { AbsenceState } from '@/lib/absence-timeline'
import { formatDay } from '@/lib/format'
import { MemberName } from '@/molecules/MemberName'
import { EndAbsenceButton } from '@/pages/absences/EndAbsenceButton'

export const STATE_BADGES: Record<AbsenceState, { label: string; variant: BadgeVariant }> = {
  current: { label: 'En cours', variant: 'warning' },
  upcoming: { label: 'À venir', variant: 'brand' },
  past: { label: 'Terminée', variant: 'neutral' },
}

export type AbsenceListProps = {
  absences: AbsenceEntry[]
  guildId: string
  /** Fiche des membres (`membre.fiche`). */
  linkTo: string | undefined
  /** Membre connecté. */
  userId: string
  /** `absence.autrui` : terminer l'absence d'un autre membre. */
  canEndOthers: boolean
  now: string
}

export const AbsenceList = ({ absences, guildId, linkTo, userId, canEndOthers, now }: AbsenceListProps) => (
  <Card className="gap-0 py-0">
    <ul className="divide-y">
      {absences.map((absence) => {
        const state = absenceState(absence, now)
        const own = absence.member.id === userId
        const cancelled = absence.endedAt !== null && absence.endedAt <= absence.from
        return (
          <li key={absence.id} className="flex flex-wrap items-center gap-x-3 gap-y-1.5 px-4 py-3 sm:px-5">
            <MemberName
              member={absence.member}
              guildId={linkTo}
              className="min-w-0 basis-full sm:flex-1 sm:basis-auto"
            />
            <Badge variant={cancelled ? 'neutral' : STATE_BADGES[state].variant}>
              {cancelled ? 'Annulée' : STATE_BADGES[state].label}
            </Badge>
            <span className="text-sm text-ink-muted tabular-nums">
              {formatDay(absence.from)} → {formatDay(state === 'past' ? effectiveEnd(absence) : absence.to)}
            </span>
            {state !== 'past' && (own || canEndOthers) && (
              <EndAbsenceButton guildId={guildId} absence={absence} upcoming={state === 'upcoming'} own={own} />
            )}
            <span className="w-full text-sm text-ink">{absence.motif}</span>
            {absence.createdBy.id !== absence.member.id && (
              <span className="w-full text-xs text-ink-subtle">Déclarée par {absence.createdBy.name}</span>
            )}
          </li>
        )
      })}
    </ul>
  </Card>
)
