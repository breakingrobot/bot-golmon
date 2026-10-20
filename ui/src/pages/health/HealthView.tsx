import { useQuery } from '@tanstack/react-query'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { AlertTriangle, CheckCircle2, Hash, XCircle } from 'lucide-react'

import { healthQuery } from '@/api/queries-site'
import type { GuildHealth } from '@/api/types-site'
import { Badge } from '@/atoms/Badge'
import { Card } from '@/atoms/Card'
import { ToggleGroup, ToggleGroupItem } from '@/atoms/ToggleGroup'
import { formatBytes, formatDateTime, formatDuration, formatNumber, formatRelative, plural } from '@/lib/format'
import { useGuild } from '@/lib/guild-context'
import { EmptyState } from '@/molecules/EmptyState'
import { PageHeader } from '@/molecules/PageHeader'
import { PageSkeleton } from '@/molecules/PageSkeleton'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { StatCard } from '@/molecules/StatCard'
import { StatGrid } from '@/molecules/StatGrid'
import { SILENT_PERIODS } from '@/pages/health/health-search'
import type { SilentPeriod } from '@/pages/health/health-search'

const route = getRouteApi('/serveurs/$guildId/sante')

type Channel = GuildHealth['channels'][number]

const ChannelCard = ({ channel, days }: { channel: Channel; days: number }) => (
  <li className="rounded-xl border bg-card px-4 py-3 shadow-xs">
    <div className="flex flex-wrap items-center gap-2">
      <Hash className="size-4 text-ink-subtle" aria-hidden />
      <span className="font-medium text-ink">{channel.name ?? channel.channelId}</span>
      {channel.accessible ? (
        <Badge variant="success">
          <CheckCircle2 aria-hidden /> Accessible
        </Badge>
      ) : (
        <Badge variant="danger">
          <XCircle aria-hidden /> Inaccessible
        </Badge>
      )}
      <span className="text-xs text-ink-muted">{plural(channel.webhooks, 'webhook')}</span>
    </div>
    {channel.silent && (
      <p className="mt-2 flex items-start gap-2 rounded-md bg-internal-soft px-3 py-2 text-sm text-internal">
        <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />
        <span>
          Webhook silencieux :{' '}
          {channel.lastMovementAt
            ? `aucun mouvement depuis plus de ${plural(days, 'jour')}.`
            : 'aucun mouvement reçu dans ce salon.'}{' '}
          Vérifier le webhook du jeu et les droits du bot dans le salon.
        </span>
      </p>
    )}
    <dl className="mt-2 grid gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
      <div className="flex justify-between gap-3 sm:block">
        <dt className="text-ink-subtle">Dernier mouvement</dt>
        <dd className="text-ink">
          {channel.lastMovementAt ? (
            <time dateTime={channel.lastMovementAt} title={formatDateTime(channel.lastMovementAt)}>
              {formatRelative(channel.lastMovementAt)}
            </time>
          ) : (
            'Jamais'
          )}
        </dd>
      </div>
      <div className="flex justify-between gap-3 sm:block">
        <dt className="text-ink-subtle">Dernier rattrapage</dt>
        <dd className="text-ink">
          {channel.lastCatchUpAt ? (
            <>
              <time dateTime={channel.lastCatchUpAt} title={formatDateTime(channel.lastCatchUpAt)}>
                {formatRelative(channel.lastCatchUpAt)}
              </time>
              {channel.lastCatchUpOk === false && <span className="text-danger"> · échoué</span>}
            </>
          ) : (
            'Pas depuis le démarrage'
          )}
        </dd>
      </div>
    </dl>
  </li>
)

export const HealthView = () => {
  const { guild } = useGuild()
  const search = route.useSearch()
  const navigate = useNavigate({ from: '/serveurs/$guildId/sante' })
  const query = useQuery(healthQuery(guild.id, search.jours))

  if (query.isPending) return <PageSkeleton stats={4} />
  if (query.isError) return <QueryError error={query.error} retry={() => void query.refetch()} />
  const health = query.data
  const connected = health.shards.filter((shard) => shard.connected)
  const latencies = connected.flatMap((shard) => (shard.latencyMs === null ? [] : [shard.latencyMs]))
  const latency = latencies.length > 0 ? Math.round(latencies.reduce((a, b) => a + b, 0) / latencies.length) : null
  const silent = health.channels.filter((channel) => channel.silent).length

  return (
    <>
      <PageHeader
        title="Santé"
        description={`${guild.name} · état du bot, des webhooks des coffres et dernières erreurs`}
      />
      <StatGrid>
        <StatCard
          label="Gateway Discord"
          value={health.shards.length === 0 ? 'Démarrage' : `${connected.length} / ${health.shards.length}`}
          tone={health.shards.length > 0 && connected.length === health.shards.length ? 'staff' : 'danger'}
          hint={latency === null ? 'shard(s) connecté(s)' : `shard(s) connecté(s) · ${formatNumber(latency)} ms`}
        />
        <StatCard
          label="En ligne depuis"
          value={formatDuration(health.uptimeSeconds)}
          hint={`démarré le ${formatDateTime(health.startedAt)}`}
        />
        <StatCard
          label="Base de données"
          value={health.databaseBytes === null ? '—' : formatBytes(health.databaseBytes)}
        />
        <StatCard label="Version" value={health.version} hint={health.commit ? `commit ${health.commit}` : null} />
      </StatGrid>

      <Section
        title="Webhooks des coffres"
        description="Salons suivis : le bot les voit-il encore, quand le jeu y a-t-il publié pour la dernière fois."
        actions={
          <ToggleGroup
            type="single"
            value={String(health.silentDays)}
            onValueChange={(value) =>
              value &&
              void navigate({
                search: value === '7' ? {} : { jours: value as SilentPeriod },
                replace: true,
              })
            }
            aria-label="Alerte après combien de jours sans mouvement"
          >
            {SILENT_PERIODS.map((period) => (
              <ToggleGroupItem key={period} value={period}>
                {period} j
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        }
      >
        {!health.messageContent && (
          <p className="mb-3 flex items-start gap-2 rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />
            Le Message Content Intent est désactivé : le bot ne lit pas les encadrés du jeu.
          </p>
        )}
        {health.channels.length === 0 ? (
          <EmptyState title="Aucun salon de coffre suivi">
            <p>
              <code className="rounded bg-muted px-1 font-mono text-xs">/config coffre salon:#…</code> sur Discord.
            </p>
          </EmptyState>
        ) : (
          <>
            {silent > 0 && (
              <p role="status" className="mb-3 text-sm font-medium text-internal">
                {plural(silent, 'salon silencieux', 'salons silencieux')}
              </p>
            )}
            <ul className="grid gap-3 lg:grid-cols-2">
              {health.channels.map((channel) => (
                <ChannelCard key={channel.channelId} channel={channel} days={health.silentDays} />
              ))}
            </ul>
          </>
        )}
      </Section>

      <Section
        title="Dernières erreurs"
        description={
          health.global
            ? 'Avertissements et erreurs du journal du bot, tous serveurs (100 au plus, depuis le démarrage).'
            : 'Avertissements et erreurs liés à ce serveur (depuis le démarrage). Les autres ne sont visibles que pour qui administre tous les serveurs du bot.'
        }
      >
        {health.errors.length === 0 ? (
          <EmptyState title="Aucune erreur récente" icon={<CheckCircle2 />} />
        ) : (
          <Card className="py-0">
            <ol className="divide-y">
              {health.errors.map((error) => (
                <li key={`${error.at}-${error.target}-${error.message}`} className="px-4 py-3">
                  <div className="flex flex-wrap items-center gap-2 text-xs">
                    <Badge variant={error.level === 'ERROR' ? 'danger' : 'warning'}>
                      {error.level === 'ERROR' ? 'Erreur' : 'Avertissement'}
                    </Badge>
                    <time dateTime={error.at} className="text-ink-muted tabular-nums">
                      {formatDateTime(error.at)}
                    </time>
                    <span className="truncate font-mono text-ink-subtle">{error.target}</span>
                  </div>
                  <p className="mt-1 text-sm break-words text-ink">{error.message}</p>
                </li>
              ))}
            </ol>
          </Card>
        )}
      </Section>
    </>
  )
}
