import { Skeleton } from '@/atoms/Skeleton'

export function PageSkeleton({ stats = 4, rows = 6 }: { stats?: number; rows?: number }) {
  return (
    <div aria-busy="true" aria-label="Chargement">
      <Skeleton className="h-8 w-56" />
      <Skeleton className="mt-2 h-4 w-72" />
      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {Array.from({ length: stats }, (_, index) => (
          <Skeleton key={index} className="h-20 rounded-xl" />
        ))}
      </div>
      <Skeleton className="mt-8 h-56 rounded-xl" />
      <div className="mt-8 space-y-2">
        {Array.from({ length: rows }, (_, index) => (
          <Skeleton key={index} className="h-10" />
        ))}
      </div>
    </div>
  )
}
