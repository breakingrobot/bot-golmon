import type { TicketStatus } from '@/api/types'
import { Badge } from '@/atoms/Badge'

export const STATUS: Record<TicketStatus, { label: string; variant: 'success' | 'warning' | 'neutral' }> = {
  OPEN: { label: 'Ouvert', variant: 'success' },
  SUSPENDED: { label: 'Suspendu', variant: 'warning' },
  CLOSED: { label: 'Fermé', variant: 'neutral' },
}

export function TicketStatusBadge({ status }: { status: TicketStatus }) {
  const { label, variant } = STATUS[status]
  return <Badge variant={variant}>{label}</Badge>
}
