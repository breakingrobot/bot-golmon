import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowLeft, Car, Phone } from 'lucide-react'

import { memberProfileQuery } from '@/api/queries'
import type { MemberProfile } from '@/api/types'
import { WeeklyCotisationsChart } from '@/components/charts/charts'
import { MovementsTable } from '@/components/coffre-movements'
import { AddCotisationDialog } from '@/components/cotisation-dialog'
import { EntriesTable } from '@/components/cotisation-entries'
import { MemberAvatar } from '@/components/member'
import { ObjectiveBar, ObjectiveStatus } from '@/components/objective'
import { attendanceTone, PageSkeleton, QueryError, Section, ShareBar, StatCard, StatGrid } from '@/components/page'
import { Badge, type BadgeVariant } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  formatDate,
  formatDateTime,
  formatDay,
  formatMoney,
  formatNumber,
  formatPercent,
  formatWeek,
} from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { roleColor } from '@/lib/utils'

export const Route = createFileRoute('/serveurs/$guildId/membres/$userId')({ component: ProfilePage })

const PRESENCE_STATUS: Record<
  NonNullable<MemberProfile['presence']>['recent'][number]['status'],
  { label: string; variant: BadgeVariant }
> = {
  PRESENT: { label: 'Présent', variant: 'success' },
  RETARD: { label: 'En retard', variant: 'warning' },
  ABSENT: { label: 'Absent', variant: 'danger' },
  NON_REPONDU: { label: 'Sans réponse', variant: 'neutral' },
}

function ProfilePage() {
  const { guild, userId: me } = useGuild()
  const { userId } = Route.useParams()
  const query = useQuery(memberProfileQuery(guild.id, userId))
  const canList = hasPerm(guild, 'membre.fiche')

  return (
    <>
      {canList && (
        <Button asChild variant="link" className="mb-2 h-auto px-0">
          <Link to="/serveurs/$guildId/membres" params={{ guildId: guild.id }}>
            <ArrowLeft aria-hidden /> Membres
          </Link>
        </Button>
      )}
      {query.isPending ? (
        <PageSkeleton />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : (
        <Profile profile={query.data} own={userId === me} />
      )}
    </>
  )
}

function Profile({ profile, own }: { profile: MemberProfile; own: boolean }) {
  const { guild } = useGuild()
  const canManage = hasPerm(guild, 'cotisation.gerer')
  const canAdd = own ? hasPerm(guild, 'cotisation.ajouter') : canManage
  const { member } = profile

  return (
    <>
      <header className="mb-8 flex flex-wrap items-start gap-4">
        <MemberAvatar member={member} className="size-16 text-lg" />
        <div className="min-w-0 flex-1">
          <h1 className="text-2xl font-semibold tracking-tight text-ink">
            {member.name}
            {own && <span className="ml-2 align-middle text-sm font-normal text-ink-muted">(vous)</span>}
          </h1>
          <p className="mt-1 text-sm text-ink-muted">
            {profile.joinedAt ? `Sur le serveur depuis le ${formatDate(profile.joinedAt)}` : guild.name}
          </p>
          {profile.roles.length > 0 && (
            <ul className="mt-3 flex flex-wrap gap-1.5" aria-label="Rôles">
              {profile.roles.map((role) => (
                <li key={role.id}>
                  <Badge variant="outline" className="gap-1.5">
                    <span
                      className="size-2 rounded-full bg-ink-subtle"
                      style={roleColor(role.color) ? { background: roleColor(role.color) ?? undefined } : undefined}
                      aria-hidden
                    />
                    {role.name}
                  </Badge>
                </li>
              ))}
            </ul>
          )}
        </div>
        {canAdd && profile.cotisations && (
          <AddCotisationDialog guildId={guild.id} canManage={canManage} fixedMember={own ? null : member} />
        )}
      </header>

      {profile.presence && (
        <Section title="Présences">
          <StatGrid>
            <StatCard
              label="Taux"
              value={profile.presence.rate === null ? '—' : formatPercent(profile.presence.rate)}
              tone={profile.presence.rate === null ? 'default' : attendanceTone(profile.presence.rate)}
              hint="présent + retard"
            />
            <StatCard label="Présent" value={formatNumber(profile.presence.present)} tone="staff" />
            <StatCard label="Retard" value={formatNumber(profile.presence.retard)} tone="internal" />
            <StatCard label="Absent" value={formatNumber(profile.presence.absent)} tone="danger" />
          </StatGrid>
          {profile.presence.recent.length > 0 && (
            <Card className="mt-4 gap-0 py-0">
              <ul className="divide-y">
                {profile.presence.recent.map((presence) => {
                  const status = PRESENCE_STATUS[presence.status]
                  return (
                    <li key={presence.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 px-5 py-2.5">
                      <span className="min-w-0 flex-1 font-medium text-ink">{presence.title}</span>
                      <time dateTime={presence.date} className="text-sm text-ink-muted tabular-nums">
                        {formatDateTime(presence.date)}
                      </time>
                      <Badge variant={status.variant}>{status.label}</Badge>
                    </li>
                  )
                })}
              </ul>
            </Card>
          )}
        </Section>
      )}

      {profile.absences.length > 0 && (
        <Section title="Absences">
          <Card className="gap-0 py-0">
            <ul className="divide-y">
              {profile.absences.map((absence) => (
                <li
                  key={`${absence.from}-${absence.to}`}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 px-5 py-2.5"
                >
                  <span className="text-sm text-ink tabular-nums">
                    {formatDay(absence.from)} → {formatDay(absence.to)}
                  </span>
                  {absence.current && <Badge variant="warning">En cours</Badge>}
                  <span className="w-full text-sm text-ink-muted sm:w-auto sm:flex-1">{absence.motif}</span>
                </li>
              ))}
            </ul>
          </Card>
        </Section>
      )}

      {profile.cotisations && (
        <Section title="Cotisations" description="Depuis le début ; graphique sur les 12 dernières semaines.">
          <StatGrid>
            <StatCard
              label="Total"
              value={formatMoney(profile.cotisations.totals.propre + profile.cotisations.totals.sale)}
              tone="staff"
            />
            <StatCard label="Propre" value={formatMoney(profile.cotisations.totals.propre)} />
            <StatCard label="Sale" value={formatMoney(profile.cotisations.totals.sale)} tone="internal" />
            <StatCard label="Cotisations" value={formatNumber(profile.cotisations.totals.count)} />
          </StatGrid>
          {profile.cotisations.objective && (
            <Card className="mt-4">
              <CardContent className="flex flex-wrap items-center gap-4">
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-ink">Objectif de la semaine</p>
                  <ObjectiveBar progress={profile.cotisations.objective} className="mt-2 max-w-md" />
                </div>
                <ObjectiveStatus progress={profile.cotisations.objective} />
              </CardContent>
            </Card>
          )}
          <Card className="mt-4 pb-3">
            <CardContent>
              <WeeklyCotisationsChart weekly={profile.cotisations.weekly} height={200} />
            </CardContent>
          </Card>
          {profile.cotisations.recent.length > 0 && (
            <>
              <h3 className="mt-6 mb-2 text-sm font-medium text-ink">Dernières cotisations</h3>
              <Card className="py-0">
                <EntriesTable entries={profile.cotisations.recent} showMember={false} />
              </Card>
            </>
          )}
        </Section>
      )}

      {profile.coffres && (
        <Section title="Coffres">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm text-ink-muted">Personnages :</span>
            {profile.coffres.characters.map((character) => (
              <Badge key={character.gameId} variant="outline">
                {character.name} <span className="font-mono text-ink-subtle">{character.gameId}</span>
              </Badge>
            ))}
          </div>
          <StatGrid className="mt-4 sm:grid-cols-2 lg:grid-cols-2">
            <StatCard label="Déposé" value={formatNumber(profile.coffres.deposited)} tone="staff" />
            <StatCard label="Retiré" value={formatNumber(profile.coffres.withdrawn)} tone="danger" />
          </StatGrid>
          {profile.coffres.recent.length > 0 && (
            <Card className="mt-4 py-0">
              <MovementsTable items={profile.coffres.recent} guildId={guild.id} />
            </Card>
          )}
        </Section>
      )}

      {profile.recrue && (
        <Section title="Recrutement" description="Avis par semaine, de la plus récente à la plus ancienne.">
          <Card className="gap-0 py-0">
            <ul className="divide-y">
              {profile.recrue.weeks.map((week) => (
                <li key={week.weekStart} className="px-5 py-3">
                  <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
                    <span className="min-w-0 flex-1 font-medium text-ink">{formatWeek(week.weekStart)}</span>
                    <span className="text-sm text-staff tabular-nums">Pour {week.pour}</span>
                    <span className="text-sm text-danger tabular-nums">Contre {week.contre}</span>
                    <span className="text-sm text-ink-muted tabular-nums">Blanc {week.blanc}</span>
                  </div>
                  {week.veto && (
                    <p className="mt-2 rounded-md bg-danger-soft px-2 py-1 text-xs text-danger">
                      Véto de {week.veto.by.name} · {week.veto.motif}
                    </p>
                  )}
                  <ShareBar
                    value={week.pour}
                    max={Math.max(1, week.pour + week.contre + week.blanc)}
                    label={`Part d'avis favorables, ${formatWeek(week.weekStart)}`}
                    tone="staff"
                    className="mt-2 w-full max-w-xs"
                  />
                </li>
              ))}
            </ul>
          </Card>
        </Section>
      )}

      {(profile.phone || profile.vehicles.length > 0) && (
        <Section title="Téléphone et véhicules">
          <Card>
            <CardContent className="grid gap-4 sm:grid-cols-2">
              {profile.phone && (
                <p className="flex items-center gap-2 text-sm">
                  <Phone className="size-4 text-ink-subtle" aria-hidden />
                  <span className="font-mono text-ink">{profile.phone}</span>
                </p>
              )}
              {profile.vehicles.length > 0 && (
                <ul className="grid gap-1.5 text-sm">
                  {profile.vehicles.map((vehicle) => (
                    <li key={vehicle.plate} className="flex items-center gap-2">
                      <Car className="size-4 text-ink-subtle" aria-hidden />
                      <span className="rounded border px-1.5 font-mono text-xs text-ink">{vehicle.plate}</span>
                      <span className="text-ink-muted">{vehicle.model ?? 'Modèle non précisé'}</span>
                    </li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>
        </Section>
      )}
    </>
  )
}
