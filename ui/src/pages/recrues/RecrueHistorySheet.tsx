import { useQuery } from '@tanstack/react-query'

import { recrueHistoryQuery } from '@/api/queries-groupe'
import type { MemberRef } from '@/api/types'
import { Badge } from '@/atoms/Badge'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/atoms/Sheet'
import { Skeleton } from '@/atoms/Skeleton'
import { formatNumber, formatWeek } from '@/lib/format'
import { choiceOf } from '@/lib/recrue'
import { QueryError } from '@/molecules/QueryError'

export type RecrueHistorySheetProps = {
  guildId: string
  /** Membre dont l'historique est ouvert. */
  member: MemberRef | null
  onClose: () => void
}

export const RecrueHistorySheet = ({ guildId, member, onClose }: RecrueHistorySheetProps) => {
  const history = useQuery({ ...recrueHistoryQuery(guildId, member?.id ?? ''), enabled: Boolean(member) })
  return (
    <Sheet open={Boolean(member)} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="overflow-y-auto">
        <SheetHeader>
          <SheetTitle>Historique des avis{member ? ` — ${member.name}` : ''}</SheetTitle>
          <SheetDescription>Semaines d'avis, de la plus récente à la plus ancienne (8 au plus).</SheetDescription>
        </SheetHeader>
        <div className="grid gap-4 px-4 pb-8">
          {history.isPending ? (
            <div className="space-y-3">
              <Skeleton className="h-24" />
              <Skeleton className="h-24" />
            </div>
          ) : history.isError ? (
            <QueryError error={history.error} retry={() => void history.refetch()} />
          ) : history.data.length === 0 ? (
            <p className="text-sm text-ink-muted">Aucun historique d'avis.</p>
          ) : (
            history.data.map((week) => (
              <section key={week.weekStart} className="rounded-lg border px-3 py-2.5">
                <h3 className="flex flex-wrap items-center gap-2 text-sm font-medium text-ink">
                  {formatWeek(week.weekStart)}
                  <span className="text-xs font-normal text-ink-muted tabular-nums">
                    ✅ {formatNumber(week.pour)} · ❌ {formatNumber(week.contre)} · ⬜ {formatNumber(week.blanc)}
                  </span>
                </h3>
                {week.veto && (
                  <p className="mt-1 text-sm text-danger">
                    Véto de {week.veto.by.name} : {week.veto.motif}
                  </p>
                )}
                {week.votes.length > 0 && (
                  <ul className="mt-2 space-y-1 text-sm">
                    {week.votes.map((vote) => (
                      <li key={vote.voter.id} className="flex flex-wrap items-baseline gap-x-2">
                        <Badge variant={choiceOf(vote.choice).variant}>{choiceOf(vote.choice).label}</Badge>
                        <span className="font-medium">{vote.voter.name}</span>
                        <span className="text-ink-muted">{vote.motif}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            ))
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
