import { useQuery } from '@tanstack/react-query'
import { Lock } from 'lucide-react'

import { presenceQuery, useClosePresence } from '@/api/queries-groupe'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/atoms/AlertDialog'
import { Badge } from '@/atoms/Badge'
import { Button } from '@/atoms/Button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/atoms/Sheet'
import { Skeleton } from '@/atoms/Skeleton'
import { formatDateTime, formatNumber, formatRelative } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { ANSWER_LABELS, presenceState, STATE_LABELS } from '@/lib/presence'
import { MemberName } from '@/molecules/MemberName'
import { QueryError } from '@/molecules/QueryError'
import { AnswerCounts } from '@/pages/presences/AnswerCounts'
import { PresenceGroup } from '@/pages/presences/PresenceGroup'

export type PresenceSheetProps = { guildId: string; presenceId: string | undefined; onClose: () => void }

export const PresenceSheet = ({ guildId, presenceId, onClose }: PresenceSheetProps) => {
  const { guild } = useGuild()
  const detail = useQuery({ ...presenceQuery(guildId, presenceId ?? ''), enabled: Boolean(presenceId) })
  const close = useClosePresence(guildId)
  const canClose = hasPerm(guild, 'presence.creer')
  const linkTo = hasPerm(guild, 'membre.fiche') ? guildId : undefined
  const { data } = detail
  const presence = data?.presence
  const state = presence ? STATE_LABELS[presenceState(presence)] : null

  return (
    <Sheet open={Boolean(presenceId)} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="overflow-y-auto">
        <SheetHeader>
          <SheetTitle>{presence?.title ?? 'Présence'}</SheetTitle>
          <SheetDescription>{presence ? formatDateTime(presence.date) : 'Chargement…'}</SheetDescription>
        </SheetHeader>
        <div className="grid gap-6 px-4 pb-8">
          {detail.isPending ? (
            <div className="space-y-3">
              <Skeleton className="h-6 w-40" />
              <Skeleton className="h-24" />
              <Skeleton className="h-24" />
            </div>
          ) : detail.isError ? (
            <QueryError error={detail.error} retry={() => void detail.refetch()} />
          ) : data && presence && state ? (
            <>
              <div className="flex flex-wrap items-center gap-3">
                <Badge variant={state.variant}>{state.label}</Badge>
                <AnswerCounts presence={presence} />
              </div>
              {presence.description && <p className="text-sm whitespace-pre-line text-ink">{presence.description}</p>}
              <p className="flex flex-wrap items-center gap-1 text-xs text-ink-subtle">
                Créée par <MemberName member={presence.createdBy} avatar={false} linked={false} className="text-xs" />·
                effectif du rôle : {formatNumber(data.roster)}
                {presence.closedAt && ` · close ${formatRelative(presence.closedAt)}`}
              </p>
              {canClose && !presence.closedAt && (
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <Button variant="outline" size="sm" className="w-fit">
                      <Lock aria-hidden /> Clôturer
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Clôturer « {presence.title} » ?</AlertDialogTitle>
                      <AlertDialogDescription>
                        Les boutons du message Discord sont désactivés et la présence compte dans les statistiques.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Annuler</AlertDialogCancel>
                      <AlertDialogAction disabled={close.isPending} onClick={() => close.mutate(presence.id)}>
                        Clôturer
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              )}
              <PresenceGroup title="Présents" tone="text-staff" people={data.present} guildId={linkTo} />
              <PresenceGroup title="Retards" tone="text-internal" people={data.retard} guildId={linkTo} />
              <PresenceGroup title="Absents" tone="text-danger" people={data.absent} guildId={linkTo} />
              <PresenceGroup title="Sans réponse" tone="text-ink-muted" people={data.noAnswer} guildId={linkTo} />
              <section>
                <h3 className="text-xs font-medium tracking-wide text-ink-subtle uppercase">Changements d'avis</h3>
                {data.changes.length === 0 ? (
                  <p className="mt-1 text-sm text-ink-subtle">Aucun changement d'avis.</p>
                ) : (
                  <ul className="mt-2 space-y-1.5">
                    {data.changes.map((change) => (
                      <li
                        key={`${change.member.id}-${change.at}`}
                        className="flex flex-wrap items-center gap-x-2 text-sm"
                      >
                        <MemberName member={change.member} avatar={false} linked={false} className="text-sm" />
                        <span className="text-ink-muted">
                          {change.from ? ANSWER_LABELS[change.from] : 'Sans réponse'} → {ANSWER_LABELS[change.to]}
                        </span>
                        <time dateTime={change.at} className="text-xs text-ink-subtle">
                          {formatRelative(change.at)}
                        </time>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            </>
          ) : null}
        </div>
      </SheetContent>
    </Sheet>
  )
}
