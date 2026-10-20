import { useQuery } from '@tanstack/react-query'
import { Car } from 'lucide-react'

import { vehiclesQuery } from '@/api/queries-groupe'
import { Card } from '@/atoms/Card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/atoms/Table'
import { formatNumber } from '@/lib/format'
import { hasPerm } from '@/lib/guild'
import { useGuild } from '@/lib/guild-context'
import { cn } from '@/lib/utils'
import { CopyButton } from '@/molecules/CopyButton'
import { EmptyState } from '@/molecules/EmptyState'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { TableSkeleton } from '@/molecules/TableSkeleton'
import { AddVehicleDialog } from '@/pages/annuaire/AddVehicleDialog'
import { DeleteVehicle } from '@/pages/annuaire/DeleteVehicle'
import { VehicleOwner } from '@/pages/annuaire/VehicleOwner'

export type VehiclesSectionProps = { guildId: string; q: string }

export const VehiclesSection = ({ guildId, q }: VehiclesSectionProps) => {
  const { guild } = useGuild()
  const query = useQuery(vehiclesQuery(guildId, q))
  const linkTo = hasPerm(guild, 'membre.fiche') ? guildId : undefined

  return (
    <Section
      title="Véhicules"
      description={
        query.data ? `${formatNumber(query.data.items.length)} véhicule(s) enregistré(s)` : 'Registre du groupe'
      }
      actions={query.data && <AddVehicleDialog guildId={guildId} roles={query.data.roles} />}
    >
      {query.isPending ? (
        <TableSkeleton />
      ) : query.isError ? (
        <QueryError error={query.error} retry={() => void query.refetch()} />
      ) : query.data.items.length === 0 ? (
        <EmptyState title="Aucun véhicule" icon={<Car />}>
          {q ? 'Aucun véhicule ne correspond à la recherche.' : 'Aucun véhicule enregistré.'}
        </EmptyState>
      ) : (
        <div className={cn('transition-opacity', query.isPlaceholderData && 'opacity-60')}>
          <ul className="grid gap-2 sm:hidden" aria-label="Véhicules">
            {query.data.items.map((vehicle) => (
              <li key={vehicle.plate} className="rounded-xl border bg-card px-4 py-3 shadow-xs">
                <div className="flex items-center gap-2">
                  <span className="font-mono font-semibold text-ink">{vehicle.plate}</span>
                  <CopyButton text={vehicle.plate} label="Copier la plaque" />
                  <span className="ml-auto">
                    <DeleteVehicle guildId={guildId} plate={vehicle.plate} />
                  </span>
                </div>
                <p className="text-sm text-ink-muted">{vehicle.model ?? 'Modèle inconnu'}</p>
                <div className="mt-1 text-sm">
                  <VehicleOwner vehicle={vehicle} guildId={linkTo} />
                </div>
              </li>
            ))}
          </ul>
          <Card className="hidden py-0 sm:block">
            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead>Plaque</TableHead>
                  <TableHead>Modèle</TableHead>
                  <TableHead>Propriétaire</TableHead>
                  <TableHead className="w-12">
                    <span className="sr-only">Actions</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {query.data.items.map((vehicle) => (
                  <TableRow key={vehicle.plate}>
                    <TableCell>
                      <span className="inline-flex items-center gap-1 font-mono font-medium">
                        {vehicle.plate}
                        <CopyButton text={vehicle.plate} label="Copier la plaque" />
                      </span>
                    </TableCell>
                    <TableCell className="text-ink-muted">{vehicle.model ?? '—'}</TableCell>
                    <TableCell>
                      <VehicleOwner vehicle={vehicle} guildId={linkTo} />
                    </TableCell>
                    <TableCell>
                      <DeleteVehicle guildId={guildId} plate={vehicle.plate} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Card>
        </div>
      )}
    </Section>
  )
}
