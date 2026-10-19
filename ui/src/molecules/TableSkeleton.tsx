import { Card } from '@/atoms/Card'
import { Skeleton } from '@/atoms/Skeleton'

export function TableSkeleton({ rows = 6 }: { rows?: number }) {
  return (
    <Card className="gap-2 px-5" aria-busy="true" aria-label="Chargement">
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton key={index} className="h-8" />
      ))}
    </Card>
  )
}
