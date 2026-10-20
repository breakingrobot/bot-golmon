import { History } from 'lucide-react'

import type { Recrue } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Card } from '@/atoms/Card'
import { formatDay, formatNumber } from '@/lib/format'
import { choiceOf, CHOICES, trend, TREND_LABELS, trialLength } from '@/lib/recrue'
import { MemberAvatar } from '@/molecules/MemberAvatar'
import { MemberName } from '@/molecules/MemberName'
import { VetoControl } from '@/pages/recrues/VetoControl'
import { VoteDialog } from '@/pages/recrues/VoteDialog'

export type RecrueCardProps = {
  guildId: string
  recrue: Recrue
  canVote: boolean
  canVeto: boolean
  canFollow: boolean
  /** Lien vers la fiche (`membre.fiche`). */
  linkTo: string | undefined
  onHistory: () => void
}

export const RecrueCard = ({ guildId, recrue, canVote, canVeto, canFollow, linkTo, onHistory }: RecrueCardProps) => {
  const status = TREND_LABELS[trend(recrue)]
  const counts = { POUR: recrue.pour, CONTRE: recrue.contre, BLANC: recrue.blanc }
  return (
    <Card className="gap-3 px-4 py-4 sm:px-5">
      <div className="flex items-start gap-3">
        <MemberAvatar member={recrue.member} className="size-10" />
        <div className="min-w-0 flex-1">
          <MemberName member={recrue.member} guildId={linkTo} avatar={false} className="text-base" />
          <p className="text-xs text-ink-muted">
            En essai depuis {trialLength(recrue.since)} · {formatNumber(recrue.weeks)} semaine
            {recrue.weeks > 1 ? 's' : ''} d'avis · depuis le {formatDay(recrue.since)}
          </p>
        </div>
        <Badge variant={status.variant}>{status.label}</Badge>
      </div>

      <dl className="grid grid-cols-3 gap-2 text-center">
        {CHOICES.map((choice) => (
          <div key={choice.value} className="rounded-lg bg-muted/50 px-2 py-1.5">
            <dt className="text-xs text-ink-muted">
              <span aria-hidden>{choice.emoji}</span> {choice.label}
            </dt>
            <dd className="text-lg font-semibold tabular-nums">{formatNumber(counts[choice.value])}</dd>
          </div>
        ))}
      </dl>

      {recrue.veto && (
        <p className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
          Véto de {recrue.veto.by.name} : {recrue.veto.motif}
        </p>
      )}
      {recrue.myVote && (
        <p className="text-sm text-ink-muted">
          Votre avis : <span className="font-medium text-ink">{choiceOf(recrue.myVote.choice).label}</span> —{' '}
          {recrue.myVote.motif}
        </p>
      )}

      {recrue.votes && recrue.votes.length > 0 && (
        <details className="text-sm">
          <summary className="cursor-pointer text-ink-muted hover:text-ink">
            Avis détaillés ({formatNumber(recrue.votes.length)})
          </summary>
          <ul className="mt-2 space-y-1.5">
            {recrue.votes.map((vote) => (
              <li key={vote.voter.id} className="flex flex-wrap items-baseline gap-x-2">
                <Badge variant={choiceOf(vote.choice).variant}>{choiceOf(vote.choice).label}</Badge>
                <span className="font-medium text-ink">{vote.voter.name}</span>
                <span className="w-full text-ink-muted sm:w-auto">{vote.motif}</span>
              </li>
            ))}
          </ul>
        </details>
      )}

      <div className="flex flex-wrap gap-2">
        {canVote && <VoteDialog guildId={guildId} recrue={recrue} />}
        {canVeto && <VetoControl guildId={guildId} recrue={recrue} />}
        {canFollow && (
          <Button size="sm" variant="ghost" onClick={onHistory}>
            <History aria-hidden /> Historique
          </Button>
        )}
      </div>
    </Card>
  )
}
