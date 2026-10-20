import { useQuery } from '@tanstack/react-query'

import { coffrePlayersQuery } from '@/api/queries'
import type { CoffreFilters } from '@/api/queries'
import type { CoffreMeta, CoffrePlayer } from '@/api/types'
import { Card } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import type { CsvColumn } from '@/lib/csv'
import { formatNumber } from '@/lib/format'
import { CsvExportButton } from '@/molecules/CsvExportButton'
import { EmptyState } from '@/molecules/EmptyState'
import { MemberName } from '@/molecules/MemberName'
import { MobileCard, MobileCards } from '@/molecules/MobileCards'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { LinkCharacterDialog } from '@/organisms/LinkCharacterDialog'
import { UnlinkCharacterButton } from '@/organisms/UnlinkCharacterButton'

const PLAYERS_CSV: CsvColumn<CoffrePlayer & { linked: string | null }>[] = [
  { header: 'Personnage', value: (player) => player.name },
  { header: 'Identifiant jeu', value: (player) => player.gameId },
  { header: 'Membre', value: (player) => player.linked },
  { header: 'Déposé', value: (player) => player.deposited },
  { header: 'Retiré', value: (player) => player.withdrawn },
  { header: 'Mouvements', value: (player) => player.count },
]

export type PlayersSectionProps = {
  guildId: string
  filters: CoffreFilters
  meta: CoffreMeta | undefined
  canManage: boolean
}

export const PlayersSection = ({ guildId, filters, meta, canManage }: PlayersSectionProps) => {
  const players = useQuery(coffrePlayersQuery(guildId, filters))
  const memberOf = (player: CoffrePlayer) =>
    player.member ?? meta?.links.find((link) => link.gameId === player.gameId)?.member ?? null
  return (
    <Section
      title="Par joueur"
      description="Activité de chaque personnage sur la sélection, et membre Discord relié."
      actions={
        <CsvExportButton
          filename="coffres-joueurs"
          columns={PLAYERS_CSV}
          rows={(players.data ?? []).map((player) => ({
            ...player,
            linked: memberOf(player)?.name ?? null,
          }))}
          disabled={!players.data?.length}
        />
      }
    >
      {players.isPending ? (
        <TableSkeleton rows={4} />
      ) : players.isError ? (
        <QueryError error={players.error} retry={() => void players.refetch()} />
      ) : players.data.length === 0 ? (
        <EmptyState title="Aucun mouvement sur la sélection" />
      ) : (
        <>
          <MobileCards label="Personnages">
            {players.data.map((player) => {
              const member = memberOf(player)
              return (
                <MobileCard
                  key={player.gameId ?? player.name}
                  title={player.name}
                  aside={
                    canManage &&
                    player.gameId && (
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
                    )
                  }
                  fields={[
                    { label: 'Membre', value: member ? member.name : 'Non relié' },
                    { label: 'Mouvements', value: formatNumber(player.count) },
                    {
                      label: 'Déposé',
                      value: <span className="text-staff">{formatNumber(player.deposited)}</span>,
                    },
                    {
                      label: 'Retiré',
                      value: <span className="text-danger">{formatNumber(player.withdrawn)}</span>,
                    },
                  ]}
                />
              )
            })}
          </MobileCards>
          <Card className="hidden py-0 sm:flex">
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
                  const member = memberOf(player)
                  return (
                    <TableRow key={player.gameId ?? player.name}>
                      <TableCell>
                        <span className="font-medium">{player.name}</span>
                        {player.gameId && (
                          <span className="ml-2 font-mono text-xs text-ink-subtle">{player.gameId}</span>
                        )}
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
        </>
      )}
    </Section>
  )
}
