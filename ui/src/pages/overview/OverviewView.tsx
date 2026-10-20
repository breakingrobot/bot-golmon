import {
  ArrowLeftRight,
  CalendarCheck,
  CalendarOff,
  Coins,
  LayoutDashboard,
  ScrollText,
  Ticket,
  UserPlus,
  Vault,
} from 'lucide-react'

import type { GuildOverview } from '@/api/types-site'
import { Badge } from '@/atoms/Badge'
import type { BadgeVariant } from '@/atoms/Badge'
import { formatDateTime, formatMoney, formatNumber, formatRelative, plural } from '@/lib/format'
import { canSeePage, hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { EmptyState } from '@/molecules/EmptyState'
import { ObjectiveBar } from '@/molecules/ObjectiveBar'
import { PageHeader } from '@/molecules/PageHeader'
import { AddCotisationDialog } from '@/organisms/AddCotisationDialog'
import { OverviewTile } from '@/pages/overview/OverviewTile'

const STATUS: Record<string, { label: string; variant: BadgeVariant }> = {
  PRESENT: { label: 'Présent', variant: 'success' },
  RETARD: { label: 'En retard', variant: 'warning' },
  ABSENT: { label: 'Absent', variant: 'danger' },
  NON_REPONDU: { label: 'Pas encore répondu', variant: 'neutral' },
}

export type OverviewViewProps = { overview: GuildOverview }

export const OverviewView = ({ overview }: OverviewViewProps) => {
  const { guild } = useGuild()
  const { lowStock, objective, presence, absences, recrues, transactions, tickets, journal } = overview
  const empty = Object.values(overview).every((tile) => tile === null)
  return (
    <>
      <PageHeader
        title="Accueil"
        description={guild.name}
        actions={
          hasPerm(guild, 'cotisation.ajouter') && (
            <AddCotisationDialog
              guildId={guild.id}
              canManage={hasPerm(guild, 'cotisation.gerer')}
              triggerLabel="Déclarer une cotisation"
            />
          )
        }
      />
      {empty ? (
        <EmptyState title="Rien à suivre ici pour l'instant" icon={<LayoutDashboard />}>
          <p>Les pages ouvertes à ce compte sont dans le menu du serveur.</p>
        </EmptyState>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {objective && (
            <OverviewTile
              guildId={guild.id}
              title="Objectif de la semaine"
              icon={Coins}
              to={canSeePage(guild, 'cotisations') ? '/serveurs/$guildId/cotisations' : null}
              value={
                objective.progress
                  ? objective.progress.exempt
                    ? 'Exempté'
                    : objective.progress.reached
                      ? 'Atteint'
                      : formatMoney(Math.max(0, objective.progress.target - objective.progress.amount))
                  : 'Aucun objectif'
              }
              tone={objective.progress?.reached ? 'staff' : objective.progress?.exempt ? 'brand' : 'default'}
              hint={
                objective.progress && !objective.progress.reached && !objective.progress.exempt
                  ? 'restant à verser cette semaine'
                  : objective.reached !== null && objective.tracked !== null
                    ? `${formatNumber(objective.reached)} membre(s) sur ${formatNumber(objective.tracked)} l'ont atteint`
                    : null
              }
            >
              {objective.progress && <ObjectiveBar progress={objective.progress} />}
              {objective.progress &&
                !objective.progress.reached &&
                objective.reached !== null &&
                objective.tracked !== null && (
                  <p className="mt-1 text-xs text-ink-muted">
                    {formatNumber(objective.reached)} membre(s) sur {formatNumber(objective.tracked)} l'ont atteint
                  </p>
                )}
            </OverviewTile>
          )}
          {presence && (
            <OverviewTile
              guildId={guild.id}
              title="Prochaine présence"
              icon={CalendarCheck}
              to="/serveurs/$guildId/presences"
              value={presence.next ? presence.next.title : 'Aucune prévue'}
              hint={presence.next ? formatDateTime(presence.next.date) : null}
            >
              {presence.next && (
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant={STATUS[presence.next.myStatus]?.variant ?? 'neutral'}>
                    {STATUS[presence.next.myStatus]?.label ?? presence.next.myStatus}
                  </Badge>
                  <span className="text-xs text-ink-muted">{plural(presence.next.responses, 'réponse')}</span>
                </div>
              )}
            </OverviewTile>
          )}
          {absences && (
            <OverviewTile
              guildId={guild.id}
              title="Absents aujourd'hui"
              icon={CalendarOff}
              to="/serveurs/$guildId/absences"
              value={formatNumber(absences.count)}
              hint={absences.count === 0 ? 'Tout le monde est là.' : null}
            >
              {absences.members.length > 0 && (
                <p className="line-clamp-2 text-ink-muted">
                  {absences.members.map((member) => member.name).join(', ')}
                  {absences.count > absences.members.length && '…'}
                </p>
              )}
            </OverviewTile>
          )}
          {lowStock && (
            <OverviewTile
              guildId={guild.id}
              title="Stock bas"
              icon={Vault}
              to={canSeePage(guild, 'coffres') ? '/serveurs/$guildId/coffres' : null}
              value={formatNumber(lowStock.count)}
              tone={lowStock.count > 0 ? 'danger' : 'staff'}
              hint={
                lowStock.count === 0
                  ? 'Aucun objet sous son seuil.'
                  : `${plural(lowStock.count, 'objet')} sous le seuil`
              }
            >
              {lowStock.items.length > 0 && (
                <ul className="space-y-0.5">
                  {lowStock.items.map((item) => (
                    <li key={`${item.propertyId}:${item.item}`} className="flex justify-between gap-3">
                      <span className="truncate">
                        {item.label}
                        <span className="text-ink-subtle">
                          {' '}
                          · {item.propertyName ?? `Propriété ${item.propertyId}`}
                        </span>
                      </span>
                      <span className="shrink-0 text-danger tabular-nums">
                        {formatNumber(item.quantity)} / {formatNumber(item.min)}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </OverviewTile>
          )}
          {recrues && (
            <OverviewTile
              guildId={guild.id}
              title="Recrues en cours"
              icon={UserPlus}
              to="/serveurs/$guildId/recrues"
              value={formatNumber(recrues.count)}
              tone={recrues.missingVotes ? 'internal' : 'default'}
              hint={
                recrues.missingVotes === null
                  ? 'cette semaine'
                  : recrues.missingVotes === 0
                    ? 'Tous les avis sont donnés.'
                    : `${plural(recrues.missingVotes, 'avis', 'avis')} à donner`
              }
            />
          )}
          {transactions && (
            <OverviewTile
              guildId={guild.id}
              title="Transactions en cours"
              icon={ArrowLeftRight}
              to={canSeePage(guild, 'transactions') ? '/serveurs/$guildId/transactions' : null}
              value={formatNumber(transactions.count)}
              hint={`${formatMoney(transactions.amount, true)} engagés`}
            />
          )}
          {tickets && (
            <OverviewTile
              guildId={guild.id}
              title="Tickets ouverts"
              icon={Ticket}
              to="/tickets"
              value={formatNumber(tickets.open)}
              tone={tickets.open > 0 ? 'user' : 'default'}
            />
          )}
          {journal && (
            <OverviewTile
              guildId={guild.id}
              title="Dernière activité"
              icon={ScrollText}
              to="/serveurs/$guildId/journal"
              value={journal.last ? formatRelative(journal.last.at) : 'Journal vide'}
              hint={journal.last ? `${journal.last.label} · ${journal.last.actor.name}` : null}
            />
          )}
        </div>
      )}
    </>
  )
}
