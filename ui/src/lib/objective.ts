import type { Objective } from '@/api/types'

export const COUNTS_LABEL: Record<Objective['counts'], string> = {
  total: 'propre + sale',
  propre: 'argent propre seulement',
  sale: 'argent sale seulement',
}
