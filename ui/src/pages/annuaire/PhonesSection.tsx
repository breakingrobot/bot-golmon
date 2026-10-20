import { useQuery } from '@tanstack/react-query'
import { Phone } from 'lucide-react'

import { phonesQuery } from '@/api/queries-groupe'
import { Card } from '@/atoms/Card'
import { formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { cn } from '@/lib/utils'
import { CopyButton } from '@/molecules/CopyButton'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberName } from '@/molecules/MemberName'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { DeletePhone } from '@/pages/annuaire/DeletePhone'
import { PhoneDialog } from '@/pages/annuaire/PhoneDialog'

export type PhonesSectionProps = { guildId: string; q: string }

export const PhonesSection = ({ guildId, q }: PhonesSectionProps) => {
  const { guild, userId } = useGuild()
  const query = useQuery(phonesQuery(guildId, q))
  // Son propre numéro, même hors de la recherche en cours.
  const mine = useQuery(phonesQuery(guildId, ''))
  const current = mine.data?.find((entry) => entry.member.id === userId)?.number ?? null
  const canDeleteOthers = hasPerm(guild, 'numero.supprimer_autrui')
  const linkTo = hasPerm(guild, 'membre.fiche') ? guildId : undefined

  return (
    <Section
      title="Numéros"
      description={query.data ? `${formatNumber(query.data.length)} numéro(s)` : 'Annuaire des membres'}
      actions={
        guild.inGroup && (
          <PhoneDialog
            guildId={guildId}
            userId={userId}
            current={current}
            canManage={hasPerm(guild, 'numero.ajouter_autrui')}
          />
        )
      }
    >
      {query.isPending ? (
        <TableSkeleton />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : query.data.length === 0 ? (
        <EmptyState title="Aucun numéro" icon={<Phone />}>
          {q ? 'Aucun numéro ne correspond à la recherche.' : 'Aucun numéro enregistré.'}
        </EmptyState>
      ) : (
        <Card className={cn('gap-0 py-0', query.isPlaceholderData && 'opacity-60')}>
          <ul className="divide-y">
            {query.data.map((entry) => {
              const own = entry.member.id === userId
              return (
                <li key={entry.member.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 px-4 py-2.5 sm:px-5">
                  <MemberName member={entry.member} guildId={linkTo} className="min-w-0 flex-1" />
                  <span className="inline-flex items-center gap-1 font-mono text-sm text-ink">
                    {entry.number}
                    <CopyButton text={entry.number} label="Copier le numéro" />
                  </span>
                  {(own || canDeleteOthers) && <DeletePhone guildId={guildId} entry={entry} own={own} />}
                </li>
              )
            })}
          </ul>
        </Card>
      )}
    </Section>
  )
}
