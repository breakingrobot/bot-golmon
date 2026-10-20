import { useQuery } from '@tanstack/react-query'
import { PackageOpen } from 'lucide-react'

import { coffreStockQuery } from '@/api/queries'
import { Skeleton } from '@/atoms/Skeleton'
import { EmptyState } from '@/molecules/EmptyState'
import { QueryError } from '@/molecules/QueryError'
import { Section } from '@/molecules/Section'
import { StockCard } from '@/pages/coffres/StockCard'

export type StockSectionProps = {
  guildId: string
  propriete: string | undefined
  canManage: boolean
  /** Objets connus du serveur, pour l'inventaire. */
  items: readonly { item: string; label: string }[] | undefined
}

export const StockSection = ({ guildId, propriete, canManage, items }: StockSectionProps) => {
  const stock = useQuery(coffreStockQuery(guildId, propriete))
  return (
    <Section
      title="Stock"
      description="Dépôts moins retraits lus depuis le début du suivi, plus les corrections d’inventaire. Une quantité négative signale un stock présent avant le suivi : un inventaire la corrige."
    >
      {stock.isPending ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-44 rounded-xl" />
          <Skeleton className="h-44 rounded-xl" />
        </div>
      ) : stock.isError ? (
        <QueryError error={stock.error} retry={() => void stock.refetch()} />
      ) : stock.data.properties.length === 0 ? (
        <EmptyState title="Aucun coffre suivi" icon={<PackageOpen />}>
          Les dépôts et retraits apparaissent dès que le bot lit les messages des salons de coffres.
        </EmptyState>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {stock.data.properties.map((property) => (
            <StockCard
              key={property.id}
              property={property}
              guildId={guildId}
              canManage={canManage}
              items={items ?? []}
            />
          ))}
        </div>
      )}
    </Section>
  )
}
