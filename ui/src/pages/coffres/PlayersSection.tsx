import { useQuery } from '@tanstack/react-query'

import { type CoffreFilters, coffrePlayersQuery } from '@/api/queries'
import type { CoffreMeta } from '@/api/types'
import { Card } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatNumber } from '@/lib/format'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberName } from '@/molecules/MemberName'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { LinkCharacterDialog } from '@/organisms/LinkCharacterDialog'
import { UnlinkCharacterButton } from '@/organisms/UnlinkCharacterButton'

export function PlayersSection({
  guildId,
  filters,
  meta,
  canManage,
}: {
  guildId: string
  filters: CoffreFilters
  meta: CoffreMeta | undefined
  canManage: boolean
}) {
  const players = useQuery(coffrePlayersQuery(guildId, filters))
  return (
    <Section title="Par joueur" description="Activité de chaque personnage sur la sélection, et membre Discord relié.">
      {players.isPending ? (
        <TableSkeleton rows={4} />
      ) : players.isError ? (
        <QueryError error={players.error} retry={() => void players.refetch()} />
      ) : players.data.length === 0 ? (
        <EmptyState title="Aucun mouvement sur la sélection" />
      ) : (
        <Card className="py-0">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>Personnage</TableHead>
                <TableHead>Membre</TableHead>
                <TableHead className="text-right">Déposé</TableHead>
                <TableHead className="text-right">Retiré</TableHead>
                <TableHead className="text-right">Mouvements</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {players.data.map((player) => {
                const member =
                  player.member ?? meta?.links.find((link) => link.gameId === player.gameId)?.member ?? null
                return (
                  <TableRow key={player.gameId ?? player.name}>
                    <TableCell>
                      <span className="font-medium">{player.name}</span>
                      {player.gameId && <span className="ml-2 font-mono text-xs text-ink-subtle">{player.gameId}</span>}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1">
                        {member ? (
                          <MemberName member={member} guildId={guildId} />
                        ) : (
                          <span className="text-sm text-ink-subtle">Non relié</span>
                        )}
                        {canManage && player.gameId && (
                          <>
                            <LinkCharacterDialog
                              guildId={guildId}
                              gameId={player.gameId}
                              name={player.name}
                              current={member}
                            />
                            {member && (
                              <UnlinkCharacterButton
                                guildId={guildId}
                                gameId={player.gameId}
                                name={player.name}
                                member={member}
                              />
                            )}
                          </>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-right text-staff tabular-nums">
                      {formatNumber(player.deposited)}
                    </TableCell>
                    <TableCell className="text-right text-danger tabular-nums">
                      {formatNumber(player.withdrawn)}
                    </TableCell>
                    <TableCell className="text-right text-ink-muted tabular-nums">
                      {formatNumber(player.count)}
                    </TableCell>
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>
        </Card>
      )}
    </Section>
  )
}
